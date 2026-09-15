//! CPU register context.

/// An x86_64 general-purpose register view.
///
/// Stage 1 only needs the registers used by the Linux syscall ABI plus the
/// instruction pointer. The [`unicorn_engine`] module maps this struct onto
/// Unicorn's register file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuContext {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

impl Default for CpuContext {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuContext {
    /// A context with all registers zeroed.
    pub fn new() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rsp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0,
            rflags: 0,
        }
    }

    /// Snapshot the syscall ABI register set in Linux x86_64 order.
    pub fn syscall_regs(&self) -> [u64; 6] {
        [self.rdi, self.rsi, self.rdx, self.r10, self.r8, self.r9]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syscall_regs_order() {
        let ctx = CpuContext {
            rdi: 1,
            rsi: 2,
            rdx: 3,
            r10: 4,
            r8: 5,
            r9: 6,
            ..CpuContext::new()
        };
        assert_eq!(ctx.syscall_regs(), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn default_is_zeroed() {
        let ctx = CpuContext::new();
        assert_eq!(ctx.rax, 0);
        assert_eq!(ctx.rip, 0);
    }
}
