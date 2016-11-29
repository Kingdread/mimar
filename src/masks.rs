//! Various bitmasks used in the binary format of MIMA commands and MIMA
//! microcommands.

macro_rules! bits {
    ($start:expr, ) => {};

    ($start:expr, $name:ident, $($names:ident,)*) => {
        pub const $name: u32 = 1 << $start;
        bits!($start - 1, $($names,)*);
    }
}

bits! {
    27,
    ACCU_READ,
    ACCU_WRITE,
    X_READ,
    Y_READ,
    Z_WRITE,
    ONE_WRITE,
    IAR_READ,
    IAR_WRITE,
    IR_READ,
    IR_WRITE,
    SDR_READ,
    SDR_WRITE,
    SAR_READ,
    ALU_C2,
    ALU_C1,
    ALU_C0,
    MEM_READ,
    MEM_WRITE,
}

pub const MEM_ACCESS: u32 = MEM_READ | MEM_WRITE;

pub const ALU_CONTROL: u32 = ALU_C2 | ALU_C1 | ALU_C0;
pub const ALU_SHIFT: u32 = 12;

pub const MICRO_NEXT: u32 = 0xFF;
pub const MICRO_DATA: u32 = 0xFFFFF00;

pub const OPCODE_SHIFT: u32 = 20;
pub const OPCODE: u32 = 0xF << OPCODE_SHIFT;
pub const EXTENDED_SHIFT: u32 = 16;
pub const EXTENDED: u32 = 0xFF << EXTENDED_SHIFT;

pub const DATA_MASK: u32 = 0xFFFFFF;
pub const ADDRESS_MASK: u32 = 0xFFFFF;
