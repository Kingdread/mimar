//! Information about the various registers in the MIMA.

use std::str::FromStr;

use super::masks;

/// Enum containing all available registers.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Register {
    /// Accumulator
    Accu,
    /// Constant 1
    One,
    /// instruction address register
    IAR,
    /// instruction register
    IR,
    /// first ALU input register
    X,
    /// second ALU input register
    Y,
    /// ALU result register
    Z,
    /// storage address register
    SAR,
    /// storage data register
    SDR,
}

impl Register {
    /// Return an array of all registers.
    pub fn all() -> &'static [Register] {
        use self::Register::*;
        static REGISTERS: [Register; 9] = [Accu, One, IAR, IR, X, Y, Z, SAR, SDR];
        &REGISTERS
    }

    /// Returns the bits that are responsible for controlling this register.
    ///
    /// The return format is `(read_bit, write_bit)`. If one of those is not
    /// available (e.g. for a read-only register), `None` is returned at this
    /// position.
    pub fn control_bits(&self) -> (Option<u32>, Option<u32>) {
        match *self {
            Register::Accu => (Some(masks::ACCU_READ), Some(masks::ACCU_WRITE)),
            Register::One => (None, Some(masks::ONE_WRITE)),
            Register::IAR => (Some(masks::IAR_READ), Some(masks::IAR_WRITE)),
            Register::IR => (Some(masks::IR_READ), Some(masks::IR_WRITE)),
            Register::X => (Some(masks::X_READ), None),
            Register::Y => (Some(masks::Y_READ), None),
            Register::Z => (None, Some(masks::Z_WRITE)),
            Register::SAR => (Some(masks::SAR_READ), None),
            Register::SDR => (Some(masks::SDR_READ), Some(masks::SDR_WRITE)),
        }
    }

    /// Return true if the register is readable.
    pub fn is_readable(&self) -> bool {
        self.control_bits().0.is_some()
    }

    /// Return true if the register is writeable.
    pub fn is_writeable(&self) -> bool {
        self.control_bits().1.is_some()
    }

    /// Return the register width, i.e. the number of bits it can hold.
    pub fn width(&self) -> u8 {
        match *self {
            Register::Accu => 24,
            Register::One => 24,
            Register::IAR => 20,
            Register::IR => 24,
            Register::X => 24,
            Register::Y => 24,
            Register::Z => 24,
            Register::SAR => 20,
            Register::SDR => 24,
        }
    }

    /// Return the bitmask for values of this register.
    pub fn value_bits(&self) -> u32 {
        2u32.pow(self.width() as u32) - 1
    }
}

/// Error for unknown registers, used for `std::str::FromStr`.
pub struct UnknownRegister;

impl FromStr for Register {
    type Err = UnknownRegister;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        match &lower as &str {
            "accu" | "akku" => Ok(Register::Accu),
            "one" | "eins" => Ok(Register::One),
            "iar" => Ok(Register::IAR),
            "ir" => Ok(Register::IR),
            "x" => Ok(Register::X),
            "y" => Ok(Register::Y),
            "z" => Ok(Register::Z),
            "sar" => Ok(Register::SAR),
            "sdr" => Ok(Register::SDR),
            _ => Err(UnknownRegister),
        }
    }
}
