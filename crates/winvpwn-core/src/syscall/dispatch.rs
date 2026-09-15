//! Syscall dispatch.
//!
//! Each syscall is a [`SyscallHandler`]. `Dispatch` owns the handlers and
//! routes guest syscalls (Linux x86_64 numbers) to them. Stage 1 implements
//! `write`, `exit` and `exit_group`; every other number returns `-ENOSYS`
//! and is recorded by the caller.

use std::sync::Arc;

use crate::syscall::errno::{err, linux::ENOSYS};
use crate::trace::recorder::TraceSink;
use crate::vkernel::{Context, ExitReason};

/// Register layout the dispatcher exposes to handlers.
#[derive(Debug, Clone, Copy)]
pub struct SyscallRegs {
    pub syscall_nr: i64,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub arg4: u64,
    pub arg5: u64,
    pub rip: u64,
}

impl SyscallRegs {
    /// Build the register view from the guest's Linux x86_64 ABI registers.
    ///
    /// `args` is `[rdi, rsi, rdx, r10, r8, r9]`.
    pub fn from_regs(syscall_nr: i64, args: [u64; 6], rip: u64) -> Self {
        SyscallRegs {
            syscall_nr,
            arg0: args[0],
            arg1: args[1],
            arg2: args[2],
            arg3: args[3],
            arg4: args[4],
            arg5: args[5],
            rip,
        }
    }
}

/// Result of dispatching one syscall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyscallOutcome {
    /// The syscall succeeded or failed; `ret` is the guest return value.
    Return { ret: i64 },
    /// The process asked to terminate; `reason` describes why.
    Exit(ExitReason),
}

/// A handler for a single syscall number.
pub trait SyscallHandler: Send + Sync {
    fn number(&self) -> i64;
    fn name(&self) -> &'static str;
    /// Execute the syscall against `ctx`. Implementations may read guest
    /// memory through `ctx.mem` and return a guest-visible result.
    fn handle(
        &self,
        ctx: &mut Context<'_>,
        regs: SyscallRegs,
    ) -> Result<SyscallOutcome, DispatchError>;
}

/// Errors surfaced by the dispatcher itself (not guest errnos).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchError {
    /// A handler was asked for a number it does not own.
    Mismatch,
    /// Internal engine error while reading guest state.
    Engine(String),
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchError::Mismatch => write!(f, "E201: syscall handler mismatch"),
            DispatchError::Engine(msg) => write!(f, "E202: engine error: {}", msg),
        }
    }
}

impl std::error::Error for DispatchError {}

/// Routes syscall numbers to their handlers.
#[derive(Default)]
pub struct Dispatch {
    handlers: Vec<Arc<dyn SyscallHandler>>,
    trace: Option<TraceSink>,
}

impl Dispatch {
    /// Create an empty dispatcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler. Later registrations win on collision.
    pub fn register(&mut self, handler: Arc<dyn SyscallHandler>) {
        self.handlers.retain(|h| h.number() != handler.number());
        self.handlers.push(handler);
    }

    /// Attach a trace sink that receives every dispatched syscall.
    pub fn set_trace(&mut self, sink: TraceSink) {
        self.trace = Some(sink);
    }

    fn find(&self, nr: i64) -> Option<&Arc<dyn SyscallHandler>> {
        self.handlers.iter().find(|h| h.number() == nr)
    }

    /// Dispatch a syscall. Unknown numbers return `-ENOSYS`.
    pub fn dispatch(
        &self,
        ctx: &mut Context<'_>,
        regs: SyscallRegs,
    ) -> Result<SyscallOutcome, DispatchError> {
        let handler = self.find(regs.syscall_nr);
        let outcome = match handler {
            Some(h) => h.handle(ctx, regs),
            None => Ok(SyscallOutcome::Return { ret: err(ENOSYS) }),
        }?;
        if let Some(sink) = &self.trace {
            let name = handler.map(|h| h.name()).unwrap_or("<unimplemented>");
            let args = [
                regs.arg0, regs.arg1, regs.arg2, regs.arg3, regs.arg4, regs.arg5,
            ];
            sink.record(name, regs.syscall_nr, args, regs.rip, &outcome);
        }
        Ok(outcome)
    }

    /// Look up a handler by number (tests, diagnostics).
    pub fn get(&self, nr: i64) -> Option<Arc<dyn SyscallHandler>> {
        self.find(nr).cloned()
    }

    /// The set of implemented syscall numbers.
    pub fn implemented(&self) -> Vec<i64> {
        let mut v: Vec<i64> = self.handlers.iter().map(|h| h.number()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_returns_enosys() {
        let dispatch = Dispatch::new();
        let mut ctx = Context::new();
        let regs = SyscallRegs::from_regs(999, [0, 0, 0, 0, 0, 0], 0x400000);
        assert_eq!(
            dispatch.dispatch(&mut ctx, regs).unwrap(),
            SyscallOutcome::Return { ret: -ENOSYS }
        );
    }

    #[test]
    fn implemented_lists_sorted() {
        let mut dispatch = Dispatch::new();
        let write = std::sync::Arc::new(crate::syscall::dispatch_impl::WriteHandler);
        let exit = std::sync::Arc::new(crate::syscall::dispatch_impl::ExitHandler);
        dispatch.register(write);
        dispatch.register(exit);
        assert_eq!(dispatch.implemented(), vec![1, 60]);
    }
}
