//! Stage-1 syscall handlers.

use std::sync::Arc;

use crate::syscall::dispatch::{SyscallHandler, SyscallOutcome, SyscallRegs};
use crate::syscall::errno::{err, linux::EBADF};
use crate::vkernel::process::ProcessState;
use crate::vkernel::{Context, ExitReason};

/// `write(fd, buf, count)` — capture output into the virtual process output
/// buffer instead of writing to a host descriptor.
pub struct WriteHandler;

impl SyscallHandler for WriteHandler {
    fn number(&self) -> i64 {
        1
    }
    fn name(&self) -> &'static str {
        "write"
    }

    fn handle(
        &self,
        ctx: &mut Context<'_>,
        regs: SyscallRegs,
    ) -> Result<SyscallOutcome, super::DispatchError> {
        let fd = regs.arg0 as i64;
        let ptr = regs.arg1;
        let count = regs.arg2 as usize;

        if fd != 1 && fd != 2 {
            return Ok(SyscallOutcome::Return { ret: err(EBADF) });
        }
        let data = ctx
            .mem
            .read(ptr, count)
            .map_err(|_| super::DispatchError::Engine("guest memory read failed".into()))?;
        ctx.io.write(fd, &data);
        Ok(SyscallOutcome::Return {
            ret: data.len() as i64,
        })
    }
}

/// `exit(status)` — terminate the simulated process.
pub struct ExitHandler;

impl SyscallHandler for ExitHandler {
    fn number(&self) -> i64 {
        60
    }
    fn name(&self) -> &'static str {
        "exit"
    }

    fn handle(
        &self,
        ctx: &mut Context<'_>,
        regs: SyscallRegs,
    ) -> Result<SyscallOutcome, super::DispatchError> {
        let code = (regs.arg0 & 0xff) as u8;
        ctx.process = ProcessState::Exited { code };
        Ok(SyscallOutcome::Exit(ExitReason::Exit { code }))
    }
}

/// `exit_group(status)` — terminate all threads (one thread in stage 1).
pub struct ExitGroupHandler;

impl SyscallHandler for ExitGroupHandler {
    fn number(&self) -> i64 {
        231
    }
    fn name(&self) -> &'static str {
        "exit_group"
    }

    fn handle(
        &self,
        ctx: &mut Context<'_>,
        regs: SyscallRegs,
    ) -> Result<SyscallOutcome, super::DispatchError> {
        let code = (regs.arg0 & 0xff) as u8;
        ctx.process = ProcessState::Exited { code };
        Ok(SyscallOutcome::Exit(ExitReason::Exit { code }))
    }
}

/// Register all stage-1 handlers on `dispatch`.
pub fn register_all(dispatch: &mut crate::syscall::Dispatch) {
    let handlers: Vec<Arc<dyn SyscallHandler>> = vec![
        Arc::new(WriteHandler),
        Arc::new(ExitHandler),
        Arc::new(ExitGroupHandler),
    ];
    for h in handlers {
        dispatch.register(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::Dispatch;

    #[test]
    fn write_captures_output() {
        let mut ctx = Context::new();
        let addr = 0x400000u64;
        ctx.mem.map_anon(addr, 16, 3).unwrap();
        ctx.mem.write(addr, b"hello").unwrap();

        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(WriteHandler));
        let regs = SyscallRegs::from_regs(1, [1, addr, 5, 0, 0, 0], 0x400100);
        match dispatch.dispatch(&mut ctx, regs).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, 5),
            other => panic!("unexpected: {other:?}"),
        }
        assert_eq!(ctx.io.stdout(), b"hello");
    }

    #[test]
    fn write_rejects_bad_fd() {
        let mut ctx = Context::new();
        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(WriteHandler));
        let regs = SyscallRegs::from_regs(1, [42, 0, 0, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, regs).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, err(EBADF)),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn exit_sets_state() {
        let mut ctx = Context::new();
        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(ExitHandler));
        let regs = SyscallRegs::from_regs(60, [7, 0, 0, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, regs).unwrap() {
            SyscallOutcome::Exit(ExitReason::Exit { code }) => assert_eq!(code, 7),
            other => panic!("unexpected: {other:?}"),
        }
        assert!(matches!(ctx.process, ProcessState::Exited { code: 7 }));
    }

    #[test]
    fn register_all_installs_three() {
        let mut dispatch = Dispatch::new();
        register_all(&mut dispatch);
        assert_eq!(dispatch.implemented(), vec![1, 60, 231]);
    }
}
