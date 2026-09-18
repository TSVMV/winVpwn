//! Persistent per-process kernel state.
//!
//! Lives across syscalls. The emulator hook takes the state out of shared
//! storage, dispatches, then puts it back so FD / VFS / brk survive.

use crate::vkernel::fd::FdTable;
use crate::vkernel::io::OutputCapture;
use crate::vkernel::process::ProcessState;
use crate::vkernel::vfs::Vfs;

/// Default anonymous-mmap arena (below the guest stack).
pub const MMAP_BASE: u64 = 0x0000_7f00_0000_0000;

/// Everything a syscall handler may mutate besides guest memory.
#[derive(Debug, Clone)]
pub struct KernelState {
    pub process: ProcessState,
    pub io: OutputCapture,
    pub fds: FdTable,
    pub vfs: Vfs,
    /// Bytes supplied as guest stdin (`read` on fd 0).
    pub stdin: Vec<u8>,
    /// Current working directory (Linux path).
    pub cwd: String,
    /// Reported `/proc/self/exe`.
    pub exec_path: String,
    /// Current program break.
    pub brk: u64,
    /// Lowest legal `brk` (end of the last PT_LOAD, page-aligned).
    pub brk_base: u64,
    /// Next anonymous mmap hint.
    pub mmap_next: u64,
    pub pid: i32,
    pub uid: u32,
    pub gid: u32,
    /// Address registered by `set_tid_address`.
    pub clear_tid: u64,
    /// FS base written by `arch_prctl`.
    pub fs_base: u64,
    /// Deterministic PRNG state for `getrandom` / `AT_RANDOM`.
    pub rng: u64,
    /// Fake monotonic nanoseconds.
    pub clock_ns: u64,
}

impl Default for KernelState {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelState {
    /// Fresh process: stdio open, cwd `/`, pid 1, uid/gid 1000.
    pub fn new() -> Self {
        Self {
            process: ProcessState::Running,
            io: OutputCapture::default(),
            fds: FdTable::new(),
            vfs: Vfs::new(),
            stdin: Vec::new(),
            cwd: "/".into(),
            exec_path: "/guest".into(),
            brk: 0,
            brk_base: 0,
            mmap_next: MMAP_BASE,
            pid: 1,
            uid: 1000,
            gid: 1000,
            clear_tid: 0,
            fs_base: 0,
            rng: 0x9e37_79b9_7f4a_7c15,
            clock_ns: 1_700_000_000_000_000_000,
        }
    }

    /// Next pseudo-random u64.
    pub fn next_rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let k = KernelState::new();
        assert_eq!(k.cwd, "/");
        assert_eq!(k.pid, 1);
        assert!(k.fds.is_open(0));
    }
}
