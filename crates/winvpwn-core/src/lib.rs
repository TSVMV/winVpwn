//! Core engine of winvpwn.
//!
//! Five-layer architecture (stage 1 subset):
//!
//! 1. [`elf`] — ELF parsing and loading (static `ET_EXEC` x86_64).
//! 2. [`cpu`] — Unicorn-based CPU emulation and register context.
//! 3. [`syscall`] — Linux syscall translation with errno semantics.
//! 4. [`vkernel`] — minimal virtual process model.
//! 5. [`memory`] — virtual address space bookkeeping.
//!
//! The [`python`] module (feature `python`) exposes the engine to Python
//! through PyO3.
//!
//! ## Security
//!
//! Stage 1 only implements `write`, `exit` and `exit_group`. No simulated
//! process can touch the host filesystem, the host network, or any host API
//! that is not explicitly implemented in the syscall layer. The guest writes
//! to an in-memory capture buffer owned by the emulator, never to a host
//! descriptor.

pub mod cpu;
pub mod elf;
pub mod memory;
pub mod syscall;
pub mod trace;
pub mod vkernel;

#[cfg(feature = "python")]
pub mod python;
