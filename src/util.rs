//! Utility functions that are useful in many cases.

/// Parse a number.
///
/// The number is assumed to be decimal. If a 0x or $ prefix is found, the
/// number is parsed as hexadecimal instead.
///
/// ```rust
/// use mimar::util::parse_num;
/// assert_eq!(parse_num("123"), Some(123));
/// assert_eq!(parse_num("0x10"), Some(16));
/// assert_eq!(parse_num("$10"), Some(16));
/// assert_eq!(parse_num("-0xF"), Some(-15));
/// assert_eq!(parse_num("foo"), None);
/// ```
pub fn parse_num(text: &str) -> Option<i32> {
    let mut result = 0;
    let mut base = 10;
    let mut stripped = text;
    let sign = if stripped.starts_with("-") {
        stripped = &stripped[1..];
        -1
    } else {
        1
    };
    if stripped.starts_with("0x") {
        stripped = &stripped[2..];
        base = 16;
    } else if stripped.starts_with("$") {
        stripped = &stripped[1..];
        base = 16;
    };
    for chr in stripped.chars() {
        result *= base;
        if let Some(d) = chr.to_digit(16) {
            result += d as i32
        } else {
            return None;
        }
    }
    Some(sign * result)
}

/// Bit-rotate num to the right.
///
/// The rightmost bit is appended left. The num is assumed to have width bits.
/// The bits left of the width'th bit are ignored.
///
/// ```rust
/// use mimar::util::rar;
/// assert_eq!(rar(2, 2), 1);
/// assert_eq!(rar(1, 2), 2);
/// ```
pub fn rar(num: u32, width: u32) -> u32 {
    let last = num & 1;
    (num >> 1) | (last << (width - 1))
}
