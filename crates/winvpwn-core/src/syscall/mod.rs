//! Linux syscall translation.

pub mod dispatch;
pub mod dispatch_impl;
pub mod errno;

pub use dispatch::{Dispatch, DispatchError, SyscallHandler, SyscallOutcome, SyscallRegs};
