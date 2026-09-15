//! ELF parsing and loading.

pub mod loader;

pub use loader::{load_elf, ElfError, LoadedElf, Segment};
