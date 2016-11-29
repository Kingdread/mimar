//! Pluggable logging interface.
//!
//! mimar supports customizing the logging by providing objects which implement
//! the `Logger` trait. This is useful if you want to control what (and how
//! much) output is generated.

use super::Mima;
use super::firmware::Instruction;

/// Trait for objects that can log MIMA actions.
///
/// The default action for any method is to do nothing, generating no output.
#[allow(unused_variables)]
pub trait Logger {
    fn log_instruction(&self, mima: &Mima, iar: u32, instr: &Instruction, param: u32) {}
}

/// Object which does not generate any logging.
pub struct NoLogging;

impl Logger for NoLogging {}
