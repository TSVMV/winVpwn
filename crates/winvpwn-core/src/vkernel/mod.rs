//! The virtual kernel layer: processes, file descriptors, VFS, and the
//! per-syscall context handed to handlers.

pub mod fd;
pub mod io;
pub mod kernel;
pub mod mem;
pub mod process;
pub mod vfs;

use crate::memory::mmap::{MemoryMap, MmapError};
use crate::vkernel::kernel::KernelState;
use crate::vkernel::mem::GuestMemory;
use crate::vkernel::process::ProcessState;

/// The state of a simulated process at any point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitReason {
    /// The process exited via `exit`/`exit_group` with `code`.
    Exit { code: u8 },
    /// The emulator stopped because the guest reached an address outside the
    /// mapped image (infinite loop or jump into nowhere).
    Falloff { rip: u64 },
    /// The user stopped the run explicitly.
    Stopped,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitReason::Exit { code } => write!(f, "exit({})", code),
            ExitReason::Falloff { rip } => write!(f, "instruction falloff at 0x{rip:x}"),
            ExitReason::Stopped => write!(f, "stopped by user"),
        }
    }
}

/// An error produced by the virtual kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VkernelError {
    /// Mapping a guest region failed.
    Map(MmapError),
    /// The process reached an illegal state.
    State(String),
}

impl std::fmt::Display for VkernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VkernelError::Map(e) => write!(f, "E301: {}", e),
            VkernelError::State(msg) => write!(f, "E302: {}", msg),
        }
    }
}

impl std::error::Error for VkernelError {}

impl From<MmapError> for VkernelError {
    fn from(e: MmapError) -> Self {
        VkernelError::Map(e)
    }
}

/// Wrapper over a process that is running or has already terminated.
pub struct Process {
    state: ProcessState,
    /// Virtual memory mapped for this process.
    pub mem: crate::memory::mmap::MemoryMap,
}

impl Process {
    pub fn new() -> Self {
        Self {
            state: ProcessState::Running,
            mem: MemoryMap::new(),
        }
    }

    /// Current process state.
    pub fn state(&self) -> &ProcessState {
        &self.state
    }
}

impl Default for Process {
    fn default() -> Self {
        Self::new()
    }
}

/// The context shared by the syscall dispatcher: guest memory plus kernel
/// state that survives across syscalls (FDs, VFS, brk, stdin).
pub struct Context<'a> {
    pub mem: Box<dyn GuestMemory + 'a>,
    pub kernel: KernelState,
}

impl Context<'_> {
    /// Create an empty context backed by an in-memory guest map.
    pub fn new() -> Self {
        Self {
            mem: Box::new(crate::vkernel::mem::InMemGuest::new()),
            kernel: KernelState::new(),
        }
    }
}

impl Default for Context<'_> {
    fn default() -> Self {
        Self::new()
    }
}
