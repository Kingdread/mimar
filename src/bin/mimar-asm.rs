//! MIMA assembler.
//!
//! The MIMA assembler takes input in an assembly-like language and outputs
//! memory maps suitable for use with [mimar-sim](../mimar_sim/index.html).
//!
//! # Assembly format
//!
//! The general format is one command per line, where a command looks like `LDC
//! 0`, where but the argument can be omitted (e.g. `NOT`).
//!
//! Lines may be prefixed with a label, which will define a global variable with
//! the command's location as value, e.g. `LOOP: LDV 0`. Instead of constants as
//! arguments, you can specify labels, like `JMP LOOP`.
//!
//! You can specify where to start blocks with the `*= address`. The
//! preprocessor can also define constants with `NAME = value`. Note though that
//! this value is replaced at assemble-time, much like `#define`s in C. The
//! value is not placed in the storage and can't be accessed from within the
//! program.
//!
//! To initialize a cell to a value, use the special `DS` instruction. This will
//! just fill the cell with the given constant.
//!
//! # Example
//!
//! The syntax is best shown with an example:
//!
//! ```text
//! I:     DS 5
//!        *= $100
//! START: LDC 1
//!        ADD I
//!        HALT
//! ```
//!
//! This program defines a variable I with initial value 5 (or more exact, it
//! defines the label I to point at a memory cell which is initialized with
//! value 5). Then, at address 0x100, it places the code to load the constant 1,
//! add the value of I and halt the machine.
//!
//! You can test the program like this (assuming you have the firmware saved as
//! `firmware`, see [`mimar-fwc`](../mimar_fwc/index.html)):
//!
//! ```text
//! mimar-asm firmware example
//! mimar-sim firmware out.mima -s START
//! ```
//!
//! # Output format
//!
//! The output is a single memory cell per line, in the format `address value`,
//! where address and value are the hex-encoded address and value. If the
//! address had a label associated with it, it is placed as a comment after the
//! line.
extern crate mimar;
extern crate rustc_serialize;
extern crate docopt;
#[macro_use]
extern crate lazy_static;
extern crate regex;

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::process;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter, Write as FmtWrite};
use std::error;

use docopt::Docopt;
use regex::Regex;

use mimar::firmware::Firmware;
use mimar::util;
use mimar::masks;

/// Argument to a command.
///
/// Can either be a constant or a global (which might not yet be defined).
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
enum Argument {
    /// Constant defined in the source
    Constant(i32),
    /// Placeholder for a global variable
    Global(String),
    /// No argument
    None,
}

/// Assembler error
#[derive(Debug)]
enum Error {
    /// Invalid input line
    InvalidLine(usize, String),
    /// No label with the given name found
    NoLabel(String),
    /// Invalid number literal
    InvalidLiteral(usize, String),
    /// Invalid command
    InvalidCommand(String),
    /// Underlying IO error
    IoError(io::Error),
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        try!(write!(fmt, "{}: ", error::Error::description(self)));
        match *self {
            Error::InvalidLine(n, ref l) =>
                write!(fmt, "line {}: {}", n, l),
            Error::NoLabel(ref l) =>
                write!(fmt, "{}", l),
            Error::InvalidLiteral(n, ref l) =>
                write!(fmt, "line {}: {}", n, l),
            Error::InvalidCommand(ref l) =>
                write!(fmt, "{}", l),
            Error::IoError(ref e) =>
                write!(fmt, "{}", e),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::InvalidLine(..) => "invalid line",
            Error::NoLabel(..) => "unknown label",
            Error::InvalidLiteral(..) => "invalid literal",
            Error::InvalidCommand(..) => "invalid command",
            Error::IoError(_) => "IO error",
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IoError(e)
    }
}

fn parse_num(input: &str, line_no: usize, line: &str) -> Result<i32, Error> {
    util::parse_num(input).ok_or_else(
        || Error::InvalidLiteral(line_no, line.into()))
}

/// Assemble input from the given reader, writing to the given writer.
fn assemble<B: BufRead>(fw: &Firmware, input: B) -> Result<String, Error> {
    lazy_static! {
        static ref SETLOC: Regex = Regex::new(r"^\*\s*=\s*([$x0-9a-fA-F]+)$").unwrap();
        static ref CONSTANT: Regex = Regex::new(r"([A-Za-z]\w*)\s*=\s*([$x0-9a-fA-F]+)$").unwrap();
        static ref LABEL: Regex = Regex::new(r"([A-Za-z]\w*):$").unwrap();
        static ref COMMAND: Regex = Regex::new(
            r"(?:(?P<label>[A-Za-z]\w*):)?\s*(?P<command>[A-Za-z]+)(?:\s+(?P<arg>[-$A-Za-z0-9]+))?$").unwrap();
    }
    let mut result: HashMap<u32, (String, Argument)> = HashMap::new();
    let mut globals: HashMap<String, i32> = HashMap::new();
    let mut next = 0;
    for (line_no, input_line) in input.lines().enumerate() {
        let input_line = try!(input_line);
        let comment_start = input_line.find(';').unwrap_or(input_line.len());
        let line = &input_line[..comment_start];
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(cap) = SETLOC.captures(&line) {
            next = try!(parse_num(&cap[1], line_no, line));

        } else if let Some(cap) = CONSTANT.captures(&line) {
            let value = try!(parse_num(&cap[2], line_no, line));
            globals.insert(cap[1].into(), value);

        } else if let Some(cap) = LABEL.captures(&line) {
            globals.insert(cap[1].into(), next as i32);

        } else if let Some(cap) = COMMAND.captures(&line) {
            if let Some(name) = cap.name("label") {
                globals.insert(name.into(), next as i32);
            }
            let arg = cap.name("arg").map(
                |v| util::parse_num(v).map(Argument::Constant)
                    .unwrap_or(Argument::Global(v.into())))
                .unwrap_or(Argument::None);
            let cmd = cap.name("command").unwrap();
            result.insert(next as u32, (cmd.into(), arg));
            next += 1;

        } else {
            return Err(Error::InvalidLine(line_no, input_line.clone()));
        }
    }
    let reverse_labels: HashMap<u32, &str> = globals.iter()
        .map(|(k, v)| (*v as u32, k as &str)).collect();
    let mut memory = result.into_iter().collect::<Vec<_>>();
    memory.sort_by(|a, b| a.0.cmp(&b.0));
    let mut output = String::new();
    for (address, command) in memory {
        let mut instr: u32 = 0;

        // Special case DS
        if command.0 == "DS" {
            if let Argument::Constant(i) = command.1 {
                instr = i as u32 & masks::DATA_MASK;
            } else {
                return Err(Error::InvalidCommand(command.0));
            }

        } else {
            match fw.find_instruction_by_mnemonic(&command.0) {
                Some(i) => instr |= i.opcode as u32,
                None => return Err(Error::InvalidCommand(command.0)),
            }
            if instr > 0xF {
                instr <<= masks::EXTENDED_SHIFT;
            } else {
                instr <<= masks::OPCODE_SHIFT;
            }
            match command.1 {
                Argument::Constant(i) => instr |= i as u32 & masks::ADDRESS_MASK,
                Argument::Global(n) => {
                    if let Some(l) = globals.get(&n) {
                        println!("GLOBAL {}", n);
                        instr |= *l as u32 & masks::ADDRESS_MASK;
                    } else {
                        return Err(Error::NoLabel(n));
                    }
                },
                Argument::None => (),
            }
        }

        write!(output, "{:#07x} {:#08x}", address, instr).unwrap();
        if let Some(lbl) = reverse_labels.get(&address) {
            write!(output, " ;{}", lbl).unwrap();
        }
        writeln!(output, "").unwrap();
    }
    Ok(output)
}

/// Take the file path and return a `BufReader`.
///
/// If the file cannot be opened, print the error and exit.
fn input_file(path: &str) -> BufReader<File> {
    let file = File::open(path).unwrap_or_else(|e| {
        println!("Can't open {}: {}", path, e);
        process::exit(1);
    });
    BufReader::new(file)
}

const USAGE: &'static str = "
MIMA assembler.

Usage:
  mimar-asm [-o <output>] <firmware> <input>
  mimar-asm --help

Options:
  -h --help      Show this help.
  -o <output>    Specify the output file [default: out.mima].
";

#[derive(Debug, RustcDecodable)]
struct Args {
    arg_firmware: String,
    arg_input: String,
    flag_o: String,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let firmware = Firmware::load(input_file(&args.arg_firmware))
        .unwrap_or_else(|e| {
            println!("Can't load firmware: {}", e);
            process::exit(1);
        });

    let mut output = File::create(&args.flag_o)
        .unwrap_or_else(|e| {
            println!("Can't write {}: {}", args.flag_o, e);
            process::exit(1);
        });

    let asm = assemble(&firmware, input_file(&args.arg_input))
        .unwrap_or_else(|e| {
            println!("Assembler error: {}", e);
            process::exit(1);
        });

    output.write_all(asm.as_bytes()).unwrap_or_else(|e| {
        println!("Can't write output: {}", e);
        process::exit(1);
    });
}
