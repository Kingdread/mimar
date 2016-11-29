//! Library that contains structs for emulating MIMA

use std::collections::HashMap;
use std::io::{self, BufRead};
use std::error::Error;
use std::fmt::{self, Formatter, Display};

pub mod masks;
pub mod util;
pub mod firmware;
pub mod registers;

use self::firmware::Firmware;
use self::registers::Register;

/// State of the MIMA after a cycle completed
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum MimaState {
    /// The MIMA is well and running.
    Running,
    /// The MIMA errored.
    Error(MimaError),
    /// The MIMA has been halted.
    Halted,
}

/// Error that might happen during a MIMA cycle
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum MimaError {
    /// The bus is already busy
    BusBusy,
    /// The bus is empty
    BusEmpty,
    /// An invalid opcode was encountered
    InvalidOpcode,
}

/// Error that may arise when loading MIMA memory.
#[derive(Debug)]
pub enum MimaLoadError {
    /// An invalid line was encountered.
    InvalidLine,
    /// Underlying IO error.
    IOError(io::Error),
}

impl Display for MimaLoadError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        write!(fmt, "{}", self.description())
    }
}

impl From<io::Error> for MimaLoadError {
    fn from(err: io::Error) -> MimaLoadError {
        MimaLoadError::IOError(err)
    }
}

impl Error for MimaLoadError {
    fn description(&self) -> &'static str {
        match *self {
            MimaLoadError::InvalidLine => "invalid input line",
            MimaLoadError::IOError(_) => "underlying IO error",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            MimaLoadError::IOError(ref err) => Some(err),
            _ => None,
        }
    }
}

/// A Mima with registers, memory and other state.
#[derive(Clone, Debug)]
pub struct Mima {
    /// The main memory (RAM). Saved sparse, i.e. only cells with a different
    /// value than 0.
    pub memory: HashMap<u32, u32>,
    /// The currently loaded firmware
    pub firmware: Firmware,
    /// The number of cycles the MIMA did.
    pub cycle_count: u64,
    /// The values of the registers
    pub registers: HashMap<Register, u32>,
    /// The next instruction which will be executed.
    pub next_instruction: u8,
    /// Mapping of labels to their address.
    pub labels: HashMap<String, u32>,
    /// Setting of the R/W memory bits
    rw_bits: u32,
    /// Countdown until the memory access is complete
    memory_timer: u8,
}

impl Default for Mima {
    fn default() -> Mima {
        Mima::new()
    }
}

macro_rules! mtry {
    ($expr:expr) => {
        match $expr {
            Some(value) => value,
            None => return Err(MimaLoadError::InvalidLine),
        }
    }
}

macro_rules! bus_write {
    ($bus:ident, $val:expr) => {
        if let Some(_) = $bus {
            return MimaState::Error(MimaError::BusBusy);
        } else {
            $bus = Some($val);
        }
    }
}

macro_rules! bus_read {
    ($bus:ident) => {
        if let Some(val) = $bus {
            val
        } else {
            return MimaState::Error(MimaError::BusEmpty);
        }
    }
}

impl Mima {
    /// Create a new MIMA.
    ///
    /// The memory and firmware are initially set to empty (all zeroes), just
    /// like all registers.
    pub fn new() -> Mima {
        let mut registers = HashMap::new();
        for register in Register::all() {
            registers.insert(*register, 0);
        }
        registers.insert(Register::One, 1);
        Mima {
            memory: HashMap::new(),
            firmware: Firmware::new(),
            cycle_count: 0,
            registers: registers,
            next_instruction: 0,
            labels: HashMap::new(),
            rw_bits: 0,
            memory_timer: 0,
        }
    }

    /// Set the given register to the given value.
    ///
    /// The value is automatically truncated.
    pub fn set_register(&mut self, reg: Register, value: u32) {
        self.registers.insert(reg, value & reg.value_bits());
    }

    /// Get the memory at the given location
    pub fn get_memory(&mut self, location: u32) -> u32 {
        *self.memory.get(&location).unwrap_or(&0)
    }

    /// Set the given memory address to the given value.
    pub fn set_memory(&mut self, location: u32, value: u32) {
        self.memory.remove(&location);
        if value != 0 {
            self.memory.insert(location, value);
        }
    }

    /// Advance the MIMA by a cycle and update the internal state.
    pub fn cycle(&mut self) -> MimaState {
        self.cycle_count += 1;
        // The decoding phase is hard-coded
        if self.next_instruction == 0xFF {
            let ir = self.registers[&Register::IR];
            println!("  IR: {:#x}", ir);
            let mut opcode = (ir & masks::OPCODE) >> masks::OPCODE_SHIFT;
            if opcode == 0xF {
                opcode = (ir & masks::EXTENDED) >> masks::EXTENDED_SHIFT;
            }
            println!("OpCode {:#x} Param: {:#x}", opcode, ir & masks::ADDRESS_MASK);
            let instruction = match self.firmware.find_instruction(opcode as u8) {
                Some(i) => i.clone(),
                None => return MimaState::Error(MimaError::InvalidOpcode),
            };
            println!("{:?}", instruction);
            self.next_instruction = instruction.start;
            // Hard-coded HALT instruction
            if instruction.mnemonic == "HALT" {
                return MimaState::Halted;
            // Hard-coded JMN instruction
            } else if instruction.mnemonic == "JMN" {
                if self.registers[&Register::Accu] > 0x7FFFFF {
                    self.set_register(Register::IAR, ir);
                }
                self.next_instruction = 0x00;
            }
            return MimaState::Running;
        }

        let instr = self.firmware.get_memory(self.next_instruction);
        self.next_instruction = (instr & masks::MICRO_NEXT) as u8;
        let mut bus: Option<u32> = None;

        if self.rw_bits & masks::MEM_READ > 0 && self.memory_timer == 0 {
            let address = self.registers[&Register::SAR];
            let data = self.get_memory(address);
            self.set_register(Register::SDR, data);
            println!("Memory read, value: {:#x}", data);
        } else if self.rw_bits & masks::MEM_WRITE > 0 && self.memory_timer == 0 {
            let address = self.registers[&Register::SAR];
            let data = self.registers[&Register::SDR];
            self.set_memory(address, data);
            println!("Memory written");
        }

        if self.rw_bits & masks::MEM_ACCESS == instr & masks::MEM_ACCESS  && self.memory_timer > 0 {
            self.memory_timer -= 1;
        } else {
            self.memory_timer = 2;
        }

        self.rw_bits = instr & masks::MEM_ACCESS;

        for (&register, &value) in &self.registers {
            if let (_, Some(pin)) = register.control_bits() {
                if instr & pin > 0 {
                    bus_write!(bus, value);
                }
            }
        }

        for (&register, value) in self.registers.iter_mut() {
            if let (Some(pin), _) = register.control_bits() {
                if instr & pin > 0 {
                    let data = bus_read!(bus);
                    *value = data & register.value_bits();
                }
            }
        }

        let alu_cmd = (instr & masks::ALU_CONTROL) >> masks::ALU_SHIFT;
        let (reg_x, reg_y) = (self.registers[&Register::X], self.registers[&Register::Y]);
        match alu_cmd {
            0 => (),
            1 => self.set_register(Register::Z, reg_x + reg_y),
            2 => self.set_register(Register::Z, util::rar(reg_x, 24)),
            3 => self.set_register(Register::Z, reg_x & reg_y),
            4 => self.set_register(Register::Z, reg_x | reg_y),
            5 => self.set_register(Register::Z, reg_x ^ reg_y),
            6 => self.set_register(Register::Z, !reg_x),
            7 => self.set_register(Register::Z,
                                   if reg_x == reg_y {0xFFFFFF} else { 0 }),
            _ => unreachable!(),
        }

        MimaState::Running
    }

    /// Let the program continue at the given address.
    pub fn jump(&mut self, address: u32) {
        self.set_register(Register::IAR, address);
    }

    /// Load memory and labels from the given reader.
    ///
    /// The memory and labels will be cleared before.
    pub fn load<B: BufRead>(&mut self, reader: &mut B) -> Result<(), MimaLoadError> {
        self.memory.clear();
        self.labels.clear();
        for line in reader.lines() {
            let line = try!(line);
            let mut splitted = line.split(";");
            let cell = splitted.next().unwrap();
            let comment = splitted.next();
            let mut splitted = cell.split(" ");
            let address = mtry!(splitted.next().map(str::trim).and_then(util::parse_num)) as u32;
            let value = mtry!(splitted.next().map(str::trim).and_then(util::parse_num)) as u32;
            if value != 0 {
                self.memory.insert(address, value);
            }
            if let Some(labels) = comment {
                for label in labels.split(" ") {
                    let label = label.trim();
                    if label.is_empty() {
                        continue;
                    }
                    self.labels.insert(label.into(), address);
                }
            }
        }
        Ok(())
    }
}
