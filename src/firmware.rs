//! Representation of the MIMA firmware in memory.

use std::collections::HashMap;
use std::io::{self, Write, BufRead};

use super::util;

/// Type of a microinstruction.
pub type Microinstruction = u32;

/// A single instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// The numeric opcode, as found in the compiled assembly.
    pub opcode: u8,
    /// The "human readable" mnemonic, e.g. `LDC`.
    pub mnemonic: String,
    /// The start of the instruction in the compiled firmware memory.
    pub start: u8,
}

/// The firmware which the MIMA runs.
#[derive(Debug, Clone, Default)]
pub struct Firmware {
    /// All implemented instructions.
    pub instructions: Vec<Instruction>,
    /// The code for each instruction.
    pub code: HashMap<u8, Microinstruction>,
}

impl Firmware {
    /// Create a new empty Firmware.
    pub fn new() -> Firmware {
        Default::default()
    }

    /// Insert the given instruction.
    ///
    /// This overrides any older instruction with the same opcode.
    pub fn insert_instruction(&mut self, instr: Instruction) {
        self.instructions.retain(|i| i.opcode != instr.opcode);
        self.instructions.push(instr);
    }

    /// Find the instruction with the given opcode.
    pub fn find_instruction(&self, opcode: u8) -> Option<&Instruction> {
        for instr in &self.instructions {
            if instr.opcode == opcode {
                return Some(instr);
            }
        }
        None
    }

    /// Load memory from a slice.
    ///
    /// It is assumed that the given slice starts at 0x00.
    pub fn load_memory(&mut self, memory: &[Microinstruction]) {
        for (i, val) in memory.iter().enumerate() {
            self.set_memory(i as u8, *val);
        }
    }

    /// Load the given memory address.
    pub fn get_memory(&self, location: u8) -> Microinstruction {
        *self.code.get(&location).unwrap_or(&0)
    }

    /// Set the given memory address to the given value.
    pub fn set_memory(&mut self, location: u8, value: u32) {
        self.code.remove(&location);
        if value != 0 {
            self.code.insert(location, value);
        }
    }

    /// Output the firmware to the given writer.
    pub fn save<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for inst in &self.instructions {
            try!(writeln!(out, "I:{} {:#04x} {:#04x}", inst.mnemonic, inst.opcode, inst.start));
        }
        try!(writeln!(out, ""));
        for i in 0..256 {
            let i = i as u8;
            try!(writeln!(out, "M:{:#04x} {:#09x}", i, self.get_memory(i)));
        }
        Ok(())
    }

    /// Load the firmware from the given reader.
    pub fn load<B: BufRead>(reader: &mut B) -> io::Result<Firmware> {
        let mut firmware = Firmware::new();
        for line in reader.lines() {
            let line = try!(line);
            if line.starts_with("I:") {
                let mut split = line[2..].split(" ");
                let mnemo = split.next().unwrap();
                let opcode = util::parse_num(split.next().unwrap()).unwrap();
                let start = util::parse_num(split.next().unwrap()).unwrap();
                firmware.insert_instruction(Instruction {
                    opcode: opcode as u8,
                    mnemonic: mnemo.into(),
                    start: start as u8,
                });
            } else if line.starts_with("M:") {
                let mut split = line[2..].split(" ");
                let adr = util::parse_num(split.next().unwrap()).unwrap();
                let val = util::parse_num(split.next().unwrap()).unwrap();
                firmware.set_memory(adr as u8, val as u32);
            }
        }
        Ok(firmware)
    }
}
