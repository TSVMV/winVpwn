//! Core engine of winvpwn.
//!
//! Five-layer architecture:
//!
//! 1. [`elf`] — ELF parsing and loading (static `ET_EXEC` x86_64).
//! 2. [`cpu`] — Unicorn-based CPU emulation and register context.
//! 3. [`syscall`] — Linux syscall translation with errno semantics.
//! 4. [`vkernel`] — virtual process, FD table, VFS, brk/mmap.
//! 5. [`memory`] — virtual address space bookkeeping.
//!
//! The [`python`] module (feature `python`) exposes the engine to Python
//! through PyO3.
//!
//! ## Security
//!
//! Guest I/O stays inside the virtual kernel. Stdio is in-memory. Host files
//! enter the VFS only through explicit maps supplied at run start. Unmapped
//! paths return `-ENOENT`. There is no guest network.

pub mod cpu;
pub mod elf;
pub mod memory;
pub mod syscall;
pub mod trace;
pub mod vkernel;

#[cfg(feature = "python")]
pub mod python;
