//! The MIMA firmware compiler is responsible for taking a firmware description
//! in register transfer notation (described below) and outputs a compiled
//! firmware (also described below).
//!
//! # What the firmware does
//!
//! Each MIMA instruction (e.g. `LDC`) is actually a little microprogram,
//! executed by the control unit. The firmware contains those microprograms (and
//! defines their opcodes).
//!
//! # Input format
//!
//! The syntax to define a command is `define MNEMONIC OPCODE`. Mnemonic should
//! be the human-readable mnemnonic and opcode should be the numeric code,
//! either decimal (`10`) or hexadecimal (`0x10` or `$10`).
//!
//! Following the define-line should be the single cycles, so each line defines
//! which registers are reading and which are writing. The syntax elements are:
//!
//! * `reg1 -> reg2`: copy contents from first register to second
//! * `R = 1` or `W = 1`: set the memory read/write bit
//! * `ALU 101`: set the ALU operation
//!
//! Multiple operations can be done in a single cycle by separating them with
//! `;`, but note that you can only have a single source register (as there is
//! only one bus).
//!
//! The ALU operations are:
//!
//! * `add` or `001`: add X and Y
//! * `rar` or `010`: rotate X to the right
//! * `and` or `011`: binary-and X and Y
//! * `or` or `100`: binary-or X and Y
//! * `xor` or `101`: binary-xor X and Y
//! * `not` or `110`: build the ones-complement of X
//! * `eql` or `111`: compare X and Y. If they are equal, the result is -1,
//!    otherwise 0.
//!
//! But the easiest way is to give a small example:
//!
//! ```text
//! define ADD 0x3
//! IR -> SAR; R = 1
//! Accu -> X; R = 1
//! R = 1
//! SDR -> Y
//! ALU 001
//! Z -> Accu
//! ```
//!
//! # The default (stock) firmware
//!
//! You can generate the default firmware with all default MIMA commands with
//! the command `mimar-fwc --default`. The default firmware is output in
//! human-readable format and needs to be compiled before usage. This
//! facilitates using the default firmware to learn and to slightly modify the
//! MIMA.
//!
//! The default firmware is compatible with most other MIMA simulators and
//! defines the following commands:
//!
//! | opcode | mnemonic |
//! |--------|----------|
//! | `0x0`  | LDC      |
//! | `0x1`  | LDV      |
//! | `0x2`  | STV      |
//! | `0x3`  | ADD      |
//! | `0x4`  | AND      |
//! | `0x5`  | OR       |
//! | `0x6`  | XOR      |
//! | `0x7`  | EQL      |
//! | `0x8`  | JMP      |
//! | `0x9`  | JMN      |
//! | `0xA`  | LDIV     |
//! | `0xB`  | STIV     |
//! | `0xF0` | HALT     |
//! | `0xF1` | NOT      |
//! | `0xF2` | RAR      |
//!
//! # Peculiarities
//!
//! * The fetch phase is hard-coded and does not need to be defined in the
//!   input. It's always located at `0x00` in the firmware.
//! * The decode phase is also hard-coded and can be triggered with the
//!   instruction at `0xFF`.
//! * The "HALT" command is hardcoded in the MIMA. It's empty in the firmware.
//!   If the MIMA encounters a command with the mnemonic "HALT", it will halt.
//! * In similar vein, the "JMN" is hardcoded, because it requires conditional
//!   execution.
//!
//! # Output format
//!
//! A single cycle is encoded as 28 bit:
//!
//! ```text
//! Bit: 27 26 25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 09 08 07 06 05 04 03 02 01 00
//!      Ar Aw  X  Y  Z  E Pr Pw Ir Iw Dr Dw  S C2 C1 C0  R  W  0  0 |  adr of next cycle  |
//!
//! Ar: Accu read
//! Aw: Accu write
//! X: X read
//! Y: Y read
//! Z: Z write
//! E: E write
//! Pr: IAR read
//! Pw: IAR write
//! Ir: IR read
//! Iw: IR write
//! Dr: SDR read
//! Dw: SDR write
//! S: SAR read
//! C2, C1, C0: ALU control bits
//! R: memory read (needs 3 cycles)
//! W: memory write (needs 3 cycles)
//! 0 0: reserved
//! ```
//!
//! The output has two types of lines:
//!
//! Lines in the format `I:MNEMO OPCODE START` build a map of all instructions.
//! `START` is the first microinstruction in the firmware memory.
//!
//! Lines in the format `M:ADDRESS VALUE` define the memory of the firmware,
//! containing all the microinstructions in the format defined above.
extern crate mimar;
extern crate regex;
#[macro_use]
extern crate lazy_static;
extern crate rustc_serialize;
extern crate docopt;

use std::io::{self, BufRead, Write, BufReader};
use std::fs::File;
use std::error::Error;
use std::fmt::{self, Display};
use std::process;

use regex::Regex;
use docopt::Docopt;

use mimar::{masks, util};
use mimar::firmware::{Firmware, Microinstruction, Instruction};
use mimar::registers::{Register, UnknownRegister};

macro_rules! log {
    ($str:expr, $($args:expr),*) => {
        writeln!(io::stderr(), $str, $($args),*).unwrap();
    }
}

/// Possible errors that might happen when parsing a register-transfer line.
#[derive(Debug, PartialEq, Eq)]
enum RTError {
    /// Bus already busy because another register is writing data.
    BusBusy,
    /// Register name not known.
    UnknownRegister,
    /// Register is write only.
    RegisterReadViolation,
    /// Register is read only.
    RegisterWriteViolation,
    /// Unknown ALU operation
    InvalidALUInstruction,
    /// General syntax error
    SyntaxError,
}

impl Display for RTError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.description())
    }
}

impl Error for RTError {
    fn description(&self) -> &'static str {
        match *self {
            RTError::BusBusy => "the bus is already being used",
            RTError::UnknownRegister => "unknown register",
            RTError::RegisterReadViolation =>
                "attempting to read a write-only register",
            RTError::RegisterWriteViolation =>
                "attempting to write a read-only register",
            RTError::InvalidALUInstruction =>
                "invalid ALU instruction",
            RTError::SyntaxError => "syntax error",
        }
    }
}

impl From<UnknownRegister> for RTError {
    fn from(_: UnknownRegister) -> Self {
        RTError::UnknownRegister
    }
}

/// Parse a single line of register-transfer-notation.
fn parse_register_transfer(line: &str) -> Result<Microinstruction, RTError> {
    lazy_static! {
        static ref TRANSFER: Regex = Regex::new("(\\w+)\\s*->\\s*(\\w+)").unwrap();
        static ref RW_BIT: Regex = Regex::new("([RrWw])\\s*=\\s*([10])").unwrap();
        static ref ALU: Regex = Regex::new("ALU ([A-Za-z01]+)").unwrap();
    }
    let parts = line.split(';');
    let mut source: Option<Register> = None;
    let mut targets: Vec<Register> = Vec::new();
    let mut alu = 0;
    let mut r_bit = 0;
    let mut w_bit = 0;
    for part in parts {
        let part = part.trim();
        // handle register parts like IAR -> IR
        if let Some(caps) = TRANSFER.captures(part) {
            let src_register = try!(caps[1].parse::<Register>());
            if source.is_some() && source.unwrap() != src_register {
                return Err(RTError::BusBusy);
            }
            source = Some(src_register);
            let target = try!(caps[2].parse::<Register>());
            targets.push(target);
        // handle parts like R=1
        } else if let Some(caps) = RW_BIT.captures(part) {
            match &caps[1] {
                "r" | "R" => r_bit = caps[2].parse().unwrap(),
                "w" | "W" => w_bit = caps[2].parse().unwrap(),
                _ => unreachable!(),
            }
        // handle parts like ALU add (or ALU 011)
        } else if let Some(caps) = ALU.captures(part) {
            let cmd = caps[1].to_lowercase();
            alu = match &cmd as &str {
                "noop" | "000" => 0,
                "add" | "001" => masks::ALU_C0,
                "rar" | "rotate" | "010" => masks::ALU_C1,
                "and" | "011" => masks::ALU_C1 | masks::ALU_C0,
                "or" | "100" => masks::ALU_C2,
                "xor" | "101" => masks::ALU_C2 | masks::ALU_C0,
                "not" | "complement" | "110" => masks::ALU_C2 | masks::ALU_C1,
                "eql" | "equal" | "cmp" | "compare" | "111" =>
                    masks::ALU_C2 | masks::ALU_C1 | masks::ALU_C0,
                _ => return Err(RTError::InvalidALUInstruction),
            }
        } else {
            return Err(RTError::SyntaxError);
        }
    }

    // build the actual instruction word
    let mut instr: Microinstruction = 0;
    if let Some(source) = source {
        if let (_, Some(write_bit)) = source.control_bits() {
            instr |= write_bit;
        } else {
            return Err(RTError::RegisterReadViolation);
        }
        for target in &targets {
            if let (Some(read_bit), _) = target.control_bits() {
                instr |= read_bit;
            } else {
                return Err(RTError::RegisterWriteViolation);
            }
        }
    }
    if r_bit == 1 {
        instr |= masks::MEM_READ;
    }
    if w_bit == 1 {
        instr |= masks::MEM_WRITE;
    }
    Ok(instr | alu)
}

/// Get the initial fetch phase.
fn fetch_phase() -> Vec<Microinstruction> {
    const FETCH_PHASE: &'static str = r#"
        IAR -> SAR; IAR -> X; R = 1
        One -> Y; R = 1
        ALU add; R = 1
        Z -> IAR
        SDR -> IR
    "#;
    FETCH_PHASE.split("\n")
        // clean the string
        .map(str::trim)
        .filter(|s| !s.is_empty())
        // parse it
        .map(|s| parse_register_transfer(s).unwrap())
        .enumerate()
        // add the addresses
        .map(|(i, v)| v | if i == 4 { 0xFF } else { i as u32 + 1 })
        .collect()
}

/// Read the data from the given reader and return the compiled firmware.
fn compile_firmware<R: BufRead>(reader: &mut R) -> Option<Firmware> {
    lazy_static! {
        static ref DEFINE: Regex = Regex::new("define ([A-Z]+) ((?:(?:0x)|$)?[A-Za-z0-9]+)").unwrap();
    }
    let mut firmware = Firmware::new();
    let mut memory = fetch_phase();
    for line in reader.lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(cap) = DEFINE.captures(line) {
            // finish last instruction
            if memory.len() > 5 {
                if let Some(n) = memory.last_mut() {
                    // wrap back to fetch phase
                    *n &= masks::MICRO_DATA;
                }
            }

            let opcode = util::parse_num(&cap[2]).unwrap() as u8;
            if firmware.find_instruction(opcode).is_some() {
                log!("Opcode {:#x} duplicated", opcode);
                return None;
            }
            let pos = memory.len() as u8;
            let mnemonic = &cap[1];
            log!("Defining {} with Opcode {:#x} (pos {:#x})", mnemonic, opcode, pos);
            firmware.insert_instruction(Instruction {
                opcode: opcode,
                mnemonic: mnemonic.into(),
                start: pos,
            });
        } else {
            match parse_register_transfer(&line) {
                Ok(instr) => {
                    let next = (memory.len() + 1) as u8;
                    memory.push(instr | next as u32);
                },
                Err(e) => {
                    log!("{}: {}", e, line);
                    return None;
                },
            }
        }
    }
    // finish last instruction
    if memory.len() > 5 {
        if let Some(n) = memory.last_mut() {
            // wrap back to fetch phase
            *n &= masks::MICRO_DATA;
        }
    }
    firmware.load_memory(&memory);
    Some(firmware)
}

static DEFAULT_FW: &'static [u8] = include_bytes!("../default-fw.txt");

const USAGE: &'static str = "
MIMA firmware compiler.

Takes firmware in register transfer notation and outputs the compiled firmware.

Usage:
  mimar-fwc [<input>] [-o <output>]
  mimar-fwc --default [-o <output>]
  mimar-fwc --help

Options:
  -h --help    Show this screen.
  -o <output>  Set the output file.
  --default    Output the default firmware.
";

#[derive(Debug, RustcDecodable)]
struct Args {
    arg_input: Option<String>,
    flag_o: Option<String>,
    flag_default: bool,
}

fn arg_to_writer(arg: Option<&String>) -> Box<Write> {
    match arg {
        None => Box::new(io::stdout()),
        Some(filename) => {
            Box::new(File::create(filename).unwrap_or_else(|e| {
                log!("Can't open output file {}: {}", filename, e);
                process::exit(1);
            }))
        },
    }
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    if args.flag_default {
        let mut out = arg_to_writer(args.flag_o.as_ref());
        out.write_all(DEFAULT_FW).unwrap();
        return;
    }

    let stdin = io::stdin();
    let firmware = match args.arg_input {
        None => compile_firmware(&mut stdin.lock()),
        Some(ref filename) => {
            let file = File::open(filename).unwrap_or_else(|e| {
                log!("Can't open input file {}: {}", filename, e);
                process::exit(1);
            });
            let mut buffered_file = BufReader::new(file);
            compile_firmware(&mut buffered_file)
        }
    };

    firmware.unwrap_or_else(|| process::exit(1))
        .save(&mut arg_to_writer(args.flag_o.as_ref()))
        .unwrap_or_else(|e| {
            log!("Can't save firmware: {}", e);
            process::exit(1);
        });
}
