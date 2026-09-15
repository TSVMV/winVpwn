//! A minimal virtual address-space map.
//!
//! Stage 1 uses this to validate that loadable segments do not overlap and to
//! remember which guest ranges have been claimed. It deliberately does not
//! grant any host access: it is pure bookkeeping over guest virtual addresses.

use std::fmt;

/// An error inserting a region into the address space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmapError {
    /// The base address that failed to insert.
    pub base: u64,
}

impl fmt::Display for MmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E101: cannot map region at 0x{:x}: overlap", self.base)
    }
}

impl std::error::Error for MmapError {}

/// A claimed region of guest virtual memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub base: u64,
    pub size: u64,
    /// ELF `p_flags` bits (`PF_R`/`PF_W`/`PF_X`) or 0 for anonymous maps.
    pub prot: u32,
}

impl Region {
    /// The first address past this region.
    pub fn end(&self) -> u64 {
        self.base + self.size
    }

    /// Whether `addr` lies within this region.
    pub fn contains(&self, addr: u64) -> bool {
        self.base <= addr && addr < self.end()
    }

    /// Whether this region overlaps `[base, base + size)`.
    pub fn overlaps(&self, base: u64, size: u64) -> bool {
        self.base < base + size && base < self.end()
    }
}

/// A sorted collection of non-overlapping guest memory regions.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MemoryMap {
    regions: Vec<Region>,
}

impl MemoryMap {
    /// Create an empty address space.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a region; fails with [`MmapError`] on overlap.
    pub fn insert(&mut self, base: u64, size: u64, prot: u32) -> Result<(), MmapError> {
        if size == 0 {
            return Ok(());
        }
        if self.regions.iter().any(|r| r.overlaps(base, size)) {
            return Err(MmapError { base });
        }
        self.regions.push(Region { base, size, prot });
        self.regions.sort_by_key(|r| r.base);
        Ok(())
    }

    /// All regions currently claimed, in address order.
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query() {
        let mut map = MemoryMap::new();
        map.insert(0x400000, 0x1000, 5).unwrap();
        map.insert(0x401000, 0x2000, 6).unwrap();
        assert!(map.regions().len() == 2);
        assert!(map.regions()[0].contains(0x400800));
        assert!(!map.regions()[0].contains(0x401000));
    }

    #[test]
    fn rejects_overlap() {
        let mut map = MemoryMap::new();
        map.insert(0x1000, 0x3000, 0).unwrap();
        assert!(map.insert(0x2000, 0x100, 0).is_err());
        assert!(map.insert(0x3f00, 0x200, 0).is_err());
        assert!(map.insert(0x4000, 0x100, 0).is_ok());
    }

    #[test]
    fn zero_size_is_noop() {
        let mut map = MemoryMap::new();
        map.insert(0x1000, 0, 0).unwrap();
        assert!(map.regions().is_empty());
    }

    #[test]
    fn adjacent_does_not_overlap() {
        let mut map = MemoryMap::new();
        map.insert(0x1000, 0x1000, 0).unwrap();
        assert!(map.insert(0x2000, 0x1000, 0).is_ok());
    }
}
