//! Virtual address-space bookkeeping.

pub mod mmap;

pub use mmap::{MemoryMap, MmapError, Region};
