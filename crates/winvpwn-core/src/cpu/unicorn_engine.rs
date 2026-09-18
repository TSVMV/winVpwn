//! Unicorn-backed CPU emulation loop.
//!
//! [`Vcpu`] owns a Unicorn x86-64 engine, maps the loaded ELF segments and a
//! guest stack, installs a `syscall` interception hook, and runs the image
//! until it exits or falls off the mapped image.
//!
//! Guest `write` output is captured in the virtual process output buffer —
//! it never reaches a host descriptor in stage 1.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use unicorn_engine::unicorn_const::{Prot, RegisterX86, X86Insn};
use unicorn_engine::Unicorn;

use crate::elf::LoadedElf;
use crate::memory::mmap::{MemoryMap, MmapError};
use crate::syscall::dispatch::{Dispatch, SyscallOutcome, SyscallRegs};
use crate::vkernel::kernel::KernelState;
use crate::vkernel::mem::UnicornGuest;
use crate::vkernel::{Context, ExitReason};

/// Page size of the x86-64 guest.
pub const PAGE_SIZE: u64 = 0x1000;

/// Default guest stack base (just below the classic `0x7ffffffff000` top).
pub const STACK_BASE: u64 = 0x7fff_fffd_e000;
/// Default guest stack size.
pub const STACK_SIZE: u64 = 0x20000;

/// Errors produced by the emulator layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcpuError {
    /// Engine could not be created.
    Init(String),
    /// A guest region could not be mapped.
    Map { base: u64, reason: String },
    /// A register or memory operation failed.
    RegOp(String),
    /// The guest could not start executing.
    Exec(String),
    /// The loaded image is not mapped yet.
    NotLoaded,
}

impl VcpuError {
    /// Stable error code.
    pub fn code(&self) -> &'static str {
        match self {
            VcpuError::Init(_) => "E501",
            VcpuError::Map { .. } => "E502",
            VcpuError::RegOp(_) => "E503",
            VcpuError::Exec(_) => "E504",
            VcpuError::NotLoaded => "E505",
        }
    }
}

impl fmt::Display for VcpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VcpuError::Init(msg) => {
                write!(f, "{}: failed to initialize emulator: {}", self.code(), msg)
            }
            VcpuError::Map { base, reason } => {
                write!(
                    f,
                    "{}: cannot map guest region at 0x{base:x}: {}",
                    self.code(),
                    reason
                )
            }
            VcpuError::RegOp(msg) => write!(f, "{}: register/memory error: {}", self.code(), msg),
            VcpuError::Exec(msg) => write!(f, "{}: execution error: {}", self.code(), msg),
            VcpuError::NotLoaded => write!(f, "{}: image not loaded", self.code()),
        }
    }
}

impl std::error::Error for VcpuError {}

impl From<MmapError> for VcpuError {
    fn from(e: MmapError) -> Self {
        VcpuError::Map {
            base: e.base,
            reason: "guest address space overlap".into(),
        }
    }
}

pub type VcpuResult<T> = Result<T, VcpuError>;

/// Mutable state shared between the emulator callbacks and the owner.
struct SharedState {
    kernel: KernelState,
    exit_reason: Option<ExitReason>,
    engine_error: Option<String>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            kernel: KernelState::new(),
            exit_reason: None,
            engine_error: None,
        }
    }
}

/// A Unicorn-backed x86-64 virtual CPU.
pub struct Vcpu {
    uc: Option<Unicorn<'static, ()>>,
    shared: Rc<RefCell<SharedState>>,
    map: MemoryMap,
    /// Guest regions mapped by this VCPU (base, size, prot).
    pub regions: Vec<(u64, u64, u32)>,
}

impl Vcpu {
    /// Create an engine with `dispatch` installed as the syscall handler.
    pub fn new(dispatch: Arc<Dispatch>) -> VcpuResult<Self> {
        Self::new_with_kernel(dispatch, KernelState::new())
    }

    /// Create an engine with a prepared kernel (stdin, VFS maps, cwd).
    pub fn new_with_kernel(dispatch: Arc<Dispatch>, kernel: KernelState) -> VcpuResult<Self> {
        let uc: Unicorn<'static, ()> = Unicorn::new(
            unicorn_engine::unicorn_const::Arch::X86,
            unicorn_engine::unicorn_const::Mode::MODE_64,
        )
        .map_err(|e| VcpuError::Init(e.to_string()))?;

        let mut shared = SharedState::new();
        shared.kernel = kernel;
        let mut vcpu = Vcpu {
            uc: Some(uc),
            shared: Rc::new(RefCell::new(shared)),
            map: MemoryMap::new(),
            regions: Vec::new(),
        };
        vcpu.install_syscall_hook(dispatch)?;
        Ok(vcpu)
    }

    /// Install the `syscall` interception hook.
    fn install_syscall_hook(&mut self, dispatch: Arc<Dispatch>) -> VcpuResult<()> {
        let uc = self.uc.as_mut().expect("engine present");
        let shared = Rc::clone(&self.shared);

        uc.add_insn_sys_hook(X86Insn::SYSCALL, 1, 0, move |uc| {
            let rax = uc.reg_read(RegisterX86::RAX).unwrap_or(0) as i64;
            let rdi = uc.reg_read(RegisterX86::RDI).unwrap_or(0);
            let rsi = uc.reg_read(RegisterX86::RSI).unwrap_or(0);
            let rdx = uc.reg_read(RegisterX86::RDX).unwrap_or(0);
            let r10 = uc.reg_read(RegisterX86::R10).unwrap_or(0);
            let r8 = uc.reg_read(RegisterX86::R8).unwrap_or(0);
            let r9 = uc.reg_read(RegisterX86::R9).unwrap_or(0);
            let rip = uc.reg_read(RegisterX86::RIP).unwrap_or(0);

            let regs = SyscallRegs::from_regs(rax, [rdi, rsi, rdx, r10, r8, r9], rip);
            let kernel = {
                let mut s = shared.borrow_mut();
                std::mem::replace(&mut s.kernel, KernelState::new())
            };
            let outcome = {
                let mut ctx = Context {
                    mem: Box::new(UnicornGuest::new(uc)),
                    kernel,
                };
                let outcome = dispatch.dispatch(&mut ctx, regs);
                {
                    let mut s = shared.borrow_mut();
                    s.kernel = ctx.kernel;
                    match &outcome {
                        Ok(SyscallOutcome::Exit(reason)) => s.exit_reason = Some(reason.clone()),
                        Err(e) => s.engine_error = Some(e.to_string()),
                        _ => {}
                    }
                }
                outcome
            };

            match outcome {
                Ok(SyscallOutcome::Return { ret }) => {
                    let _ = uc.reg_write(RegisterX86::RAX, ret as u64);
                }
                Ok(SyscallOutcome::Exit(_)) => {
                    let _ = uc.emu_stop();
                }
                Err(_) => {
                    let _ = uc.emu_stop();
                }
            }
        })
        .map_err(|e| VcpuError::Init(e.to_string()))?;
        Ok(())
    }

    /// Map a loaded ELF image into the guest.
    pub fn map_elf(&mut self, image: &LoadedElf) -> VcpuResult<()> {
        let uc = self.uc.as_mut().expect("engine present");
        for seg in &image.segments {
            let start = seg.vaddr & !(PAGE_SIZE - 1);
            let end = (seg.vaddr + seg.memsz).max(seg.vaddr + seg.filesz);
            let page_end = (end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            let size = page_end - start;
            self.map.insert(start, size, seg.flags)?;

            let mut prot = Prot::NONE;
            if (seg.flags & 0x4) != 0 {
                prot |= Prot::READ;
            }
            if (seg.flags & 0x2) != 0 {
                prot |= Prot::WRITE;
            }
            if (seg.flags & 0x1) != 0 {
                prot |= Prot::EXEC;
            }

            uc.mem_map(start, size, prot).map_err(|e| VcpuError::Map {
                base: start,
                reason: e.to_string(),
            })?;
            let offset = (seg.vaddr - start) as usize;
            let data = seg.file_bytes();
            uc.mem_write(start + offset as u64, data)
                .map_err(|e| VcpuError::Map {
                    base: start,
                    reason: e.to_string(),
                })?;
            self.regions.push((start, size, seg.flags));
        }
        let high = image
            .segments
            .iter()
            .map(|s| s.vaddr + s.memsz)
            .max()
            .unwrap_or(0);
        let brk = (high + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let mut k = self.shared.borrow_mut();
        k.kernel.brk_base = brk;
        k.kernel.brk = brk;
        Ok(())
    }

    /// Map a guest memory region with explicit permissions.
    pub fn map_memory(&mut self, base: u64, size: u64, prot: Prot) -> VcpuResult<()> {
        let uc = self.uc.as_mut().expect("engine present");
        self.map.insert(base, size, 0)?;
        uc.mem_map(base, size, prot).map_err(|e| VcpuError::Map {
            base,
            reason: e.to_string(),
        })?;
        self.regions.push((base, size, 0));
        Ok(())
    }

    /// Map a guest stack region (read + write).
    pub fn map_stack(&mut self, base: u64, size: u64) -> VcpuResult<()> {
        self.map_memory(base, size, Prot::READ | Prot::WRITE)
    }

    /// Write bytes into guest memory.
    pub fn write_guest(&mut self, addr: u64, data: &[u8]) -> VcpuResult<()> {
        let uc = self.uc.as_mut().ok_or(VcpuError::NotLoaded)?;
        uc.mem_write(addr, data)
            .map_err(|e| VcpuError::RegOp(e.to_string()))
    }

    /// Set the initial register state: `entry` in RIP, RSP pointing at the
    /// initial stack frame.
    ///
    /// Linux starts the stack with `argc` at RSP, then the argv array (NULL
    /// terminated), then envp (NULL terminated), then the auxiliary vector.
    /// With `argc = 0` this is four zeroed qwords starting at `stack_top - 0x20`:
    ///
    /// ```text
    /// RSP        -> argc = 0
    /// RSP + 0x08 -> argv[0] = NULL
    /// RSP + 0x10 -> envp[0] = NULL
    /// RSP + 0x18 -> auxv[0] = AT_NULL
    /// ```
    pub fn setup_initial_state(&mut self, entry: u64, stack_top: u64) -> VcpuResult<()> {
        let uc = self.uc.as_mut().expect("engine present");
        let argc_addr = stack_top - 0x20;
        let layout = [0u8; 0x20];
        uc.mem_write(argc_addr, &layout)
            .map_err(|e| VcpuError::RegOp(e.to_string()))?;
        uc.reg_write(RegisterX86::RSP, argc_addr)
            .map_err(|e| VcpuError::RegOp(e.to_string()))?;
        uc.reg_write(RegisterX86::RIP, entry)
            .map_err(|e| VcpuError::RegOp(e.to_string()))?;
        Ok(())
    }

    /// Build a Linux-style argv/envp/auxv stack and point RSP at it.
    pub fn setup_linux_stack(
        &mut self,
        entry: u64,
        stack_top: u64,
        argv: &[String],
        envp: &[String],
    ) -> VcpuResult<()> {
        let mut strings: Vec<Vec<u8>> = Vec::new();
        for s in argv.iter().chain(envp.iter()) {
            let mut b = s.as_bytes().to_vec();
            b.push(0);
            strings.push(b);
        }
        let str_bytes: usize = strings.iter().map(|s| s.len()).sum();
        let random_sz = 16usize;
        // argc + argv pointers + NULL + env pointers + NULL + 7 auxv pairs + AT_NULL
        let ptr_count = 1 + argv.len() + 1 + envp.len() + 1 + 8 * 2;
        let ptr_bytes = ptr_count * 8;
        let mut total = ptr_bytes + str_bytes + random_sz + 16;
        total = (total + 0xf) & !0xf;
        let rsp = stack_top - total as u64;
        let mut cursor = stack_top;
        cursor -= random_sz as u64;
        let random_addr = cursor;
        let mut rand = [0u8; 16];
        {
            let mut k = self.shared.borrow_mut();
            for chunk in rand.chunks_mut(8) {
                chunk.copy_from_slice(&k.kernel.next_rand().to_le_bytes()[..chunk.len()]);
            }
        }
        self.write_guest(random_addr, &rand)?;
        let mut str_addrs = Vec::new();
        for s in &strings {
            cursor -= s.len() as u64;
            self.write_guest(cursor, s)?;
            str_addrs.push(cursor);
        }
        let mut table = Vec::with_capacity(ptr_bytes);
        table.extend_from_slice(&(argv.len() as u64).to_le_bytes());
        for i in 0..argv.len() {
            table.extend_from_slice(&str_addrs[i].to_le_bytes());
        }
        table.extend_from_slice(&0u64.to_le_bytes());
        for i in 0..envp.len() {
            table.extend_from_slice(&str_addrs[argv.len() + i].to_le_bytes());
        }
        table.extend_from_slice(&0u64.to_le_bytes());
        let (uid, gid) = {
            let k = self.shared.borrow();
            (k.kernel.uid as u64, k.kernel.gid as u64)
        };
        let aux = [
            (6u64, 0x1000u64),
            (9, entry),
            (11, uid),
            (12, uid),
            (13, gid),
            (14, gid),
            (25, random_addr),
            (0, 0),
        ];
        for (t, v) in aux {
            table.extend_from_slice(&t.to_le_bytes());
            table.extend_from_slice(&v.to_le_bytes());
        }
        self.write_guest(rsp, &table)?;
        let uc = self.uc.as_mut().expect("engine present");
        uc.reg_write(RegisterX86::RSP, rsp)
            .map_err(|e| VcpuError::RegOp(e.to_string()))?;
        uc.reg_write(RegisterX86::RIP, entry)
            .map_err(|e| VcpuError::RegOp(e.to_string()))?;
        Ok(())
    }

    /// Run until the process exits, hits an unmapped page, or times out.
    ///
    /// `timeout_ms` of 0 means no time limit. Returns the [`ExitReason`].
    pub fn run(&mut self, timeout_ms: u64) -> VcpuResult<ExitReason> {
        let uc = self.uc.as_mut().expect("engine present");
        let start = uc.reg_read(RegisterX86::RIP).unwrap_or(0);
        // `until` is an exclusive stop address; `u64::MAX` means "no bound", so
        // the guest only stops on a syscall-driven `emu_stop`, an unmapped
        // access, or the timeout.
        let err = uc.emu_start(start, u64::MAX, timeout_ms, 0);
        if let Err(e) = err {
            // The guest tried to read/fetch from an unmapped page: that is the
            // normal "fell off the image" path.
            let msg = e.to_string();
            if msg.contains("READ_PROTECT")
                || msg.contains("FETCH_PROTECT")
                || msg.contains("READ_UNMAPPED")
                || msg.contains("FETCH_UNMAPPED")
                || msg.contains("WRITE_UNMAPPED")
            {
                let rip = uc.reg_read(RegisterX86::RIP).unwrap_or(0);
                return Ok(ExitReason::Falloff { rip });
            }
            return Err(VcpuError::Exec(msg));
        }

        let state = self.shared.borrow();
        if let Some(reason) = &state.exit_reason {
            return Ok(reason.clone());
        }
        if let Some(e) = &state.engine_error {
            return Err(VcpuError::Exec(e.clone()));
        }
        Ok(ExitReason::Stopped)
    }

    /// Read a register.
    pub fn reg(&self, reg: RegisterX86) -> VcpuResult<u64> {
        let uc = self.uc.as_ref().ok_or(VcpuError::NotLoaded)?;
        uc.reg_read(reg)
            .map_err(|e| VcpuError::RegOp(e.to_string()))
    }

    /// Write a register.
    pub fn set_reg(&mut self, reg: RegisterX86, value: u64) -> VcpuResult<()> {
        let uc = self.uc.as_mut().ok_or(VcpuError::NotLoaded)?;
        uc.reg_write(reg, value)
            .map_err(|e| VcpuError::RegOp(e.to_string()))
    }

    /// Read guest memory.
    pub fn read_guest(&self, addr: u64, len: usize) -> VcpuResult<Vec<u8>> {
        let uc = self.uc.as_ref().ok_or(VcpuError::NotLoaded)?;
        let mut buf = vec![0u8; len];
        uc.mem_read(addr, &mut buf)
            .map_err(|e| VcpuError::RegOp(e.to_string()))?;
        Ok(buf)
    }

    /// The captured combined stdout+stderr so far.
    pub fn output(&self) -> Vec<u8> {
        self.shared.borrow().kernel.io.combined()
    }

    /// Snapshot of kernel state after a run (VFS dirty files, cwd, brk).
    pub fn kernel(&self) -> KernelState {
        self.shared.borrow().kernel.clone()
    }

    /// The exit reason observed during the last [`Vcpu::run`], if any.
    pub fn exit_reason(&self) -> Option<ExitReason> {
        self.shared.borrow().exit_reason.clone()
    }
}

impl Drop for Vcpu {
    fn drop(&mut self) {
        // Unicorn engines are unmapped on drop; the shared state is Rc-dropped.
        self.uc.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elf::load_elf;
    use crate::syscall::dispatch_impl;

    const HELLO: &[u8] = include_bytes!("../../tests/fixtures/hello_static");

    fn vcpu() -> Vcpu {
        let mut dispatch = Dispatch::new();
        dispatch_impl::register_all(&mut dispatch);
        Vcpu::new(Arc::new(dispatch)).unwrap()
    }

    #[test]
    fn maps_and_runs_hello() {
        let image = load_elf(HELLO).unwrap();
        let mut vcpu = vcpu();
        vcpu.map_elf(&image).unwrap();
        vcpu.map_stack(STACK_BASE, STACK_SIZE).unwrap();
        vcpu.setup_initial_state(image.entry, STACK_BASE + STACK_SIZE)
            .unwrap();

        let reason = vcpu.run(0).unwrap();
        assert_eq!(reason, ExitReason::Exit { code: 0 });
        assert_eq!(vcpu.output(), b"hello, winVpwn\n");
    }

    #[test]
    fn stack_layout_is_zeroed() {
        let mut vcpu = vcpu();
        vcpu.map_stack(STACK_BASE, STACK_SIZE).unwrap();
        vcpu.setup_initial_state(0x401000, STACK_BASE + STACK_SIZE)
            .unwrap();
        assert_eq!(vcpu.reg(RegisterX86::RIP).unwrap(), 0x401000);
        assert_eq!(
            vcpu.reg(RegisterX86::RSP).unwrap(),
            STACK_BASE + STACK_SIZE - 0x20
        );
    }

    #[test]
    fn run_falls_off_image_returns_falloff() {
        // A page containing `ret`; the stack is mapped and zeroed, so the
        // return address is 0 — an unmapped fetch — which is a falloff.
        let mut vcpu = vcpu();
        vcpu.map_memory(0x400000, 0x1000, Prot::READ | Prot::EXEC)
            .unwrap();
        vcpu.write_guest(0x400000, &[0xc3]).unwrap();
        vcpu.map_stack(STACK_BASE, STACK_SIZE).unwrap();
        vcpu.set_reg(RegisterX86::RIP, 0x400000).unwrap();
        vcpu.set_reg(RegisterX86::RSP, STACK_BASE + STACK_SIZE - 8)
            .unwrap();

        let reason = vcpu.run(0).unwrap();
        assert!(
            matches!(reason, ExitReason::Falloff { .. }),
            "got {reason:?}"
        );
    }
}
