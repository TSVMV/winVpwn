//! Guest memory access for syscall handlers.
//!
//! Two backends implement [`GuestMemory`]:
//!
//! - [`InMemGuest`]: a plain byte map used by unit tests and non-emulated
//!   contexts.
//! - [`UnicornGuest`]: delegates reads to a live Unicorn emulator, so syscall
//!   handlers observe the real guest address space during execution.

use unicorn_engine::unicorn_const::uc_error;
use unicorn_engine::Unicorn;

/// An error accessing guest memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestMemError {
    /// The guest address that failed to read or write.
    pub addr: u64,
    /// Number of bytes that could not be accessed.
    pub len: usize,
}

impl std::fmt::Display for GuestMemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "E401: guest memory access failed at 0x{:x} ({} bytes)",
            self.addr, self.len
        )
    }
}

impl std::error::Error for GuestMemError {}

/// The guest-memory interface used by syscall handlers.
pub trait GuestMemory {
    /// Read `len` bytes from `addr`.
    fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>, GuestMemError>;
    /// Write `data` to `addr`.
    fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), GuestMemError>;
    /// Map `len` anonymous bytes at `base` with permission `prot`.
    fn map_anon(&mut self, base: u64, len: usize, prot: u32) -> Result<(), GuestMemError>;
}

/// In-memory backend: a flat byte map, no emulator involved.
///
/// Used by unit tests and as the fallback when no Unicorn engine is attached.
#[derive(Debug, Default)]
pub struct InMemGuest {
    bytes: std::collections::BTreeMap<u64, Vec<u8>>,
}

impl InMemGuest {
    /// Create an empty guest memory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up the region containing `addr`.
    fn region(&self, addr: u64) -> Option<(u64, &Vec<u8>)> {
        self.bytes
            .range(..=addr)
            .next_back()
            .map(|(base, bytes)| (*base, bytes))
            .filter(|(base, bytes)| {
                let offset = addr - *base;
                offset < bytes.len() as u64
            })
    }

    /// Look up the region containing `addr`, with mutable access to the bytes.
    fn region_mut(&mut self, addr: u64) -> Option<(u64, &mut Vec<u8>)> {
        self.bytes
            .range_mut(..=addr)
            .next_back()
            .map(|(base, bytes)| (*base, bytes))
            .filter(|(base, bytes)| {
                let offset = addr - *base;
                offset < bytes.len() as u64
            })
    }
}

impl GuestMemory for InMemGuest {
    fn map_anon(&mut self, base: u64, len: usize, _prot: u32) -> Result<(), GuestMemError> {
        if self.bytes.contains_key(&base) {
            return Err(GuestMemError { addr: base, len });
        }
        self.bytes.insert(base, vec![0u8; len]);
        Ok(())
    }

    fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>, GuestMemError> {
        let mut out = Vec::with_capacity(len);
        let mut cursor = addr;
        while out.len() < len {
            let remaining = len - out.len();
            let chunk = match self.region(cursor) {
                Some((base, bytes)) => {
                    let offset = (cursor - base) as usize;
                    let take = remaining.min(bytes.len() - offset);
                    out.extend_from_slice(&bytes[offset..offset + take]);
                    take as u64
                }
                None => {
                    return Err(GuestMemError {
                        addr: cursor,
                        len: remaining,
                    })
                }
            };
            cursor += chunk;
        }
        Ok(out)
    }

    fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), GuestMemError> {
        let mut cursor = addr;
        let mut idx = 0usize;
        while idx < data.len() {
            let remaining = data.len() - idx;
            let (base, bytes) = match self.region_mut(cursor) {
                Some(region) => region,
                None => {
                    return Err(GuestMemError {
                        addr: cursor,
                        len: remaining,
                    })
                }
            };
            let offset = (cursor - base) as usize;
            let take = remaining.min(bytes.len() - offset);
            bytes[offset..offset + take].copy_from_slice(&data[idx..idx + take]);
            idx += take;
            cursor += take as u64;
        }
        Ok(())
    }
}

/// Unicorn-backed backend: reads reflect the emulator's live guest memory.
pub struct UnicornGuest<'a, 'u> {
    uc: &'a mut Unicorn<'u, ()>,
}

impl<'a, 'u> UnicornGuest<'a, 'u> {
    /// Borrow an engine; the borrow lives as long as `uc`.
    pub fn new(uc: &'a mut Unicorn<'u, ()>) -> Self {
        Self { uc }
    }
}

fn map_uc_error(e: uc_error, addr: u64, len: usize) -> GuestMemError {
    let _ = e;
    GuestMemError { addr, len }
}

impl GuestMemory for UnicornGuest<'_, '_> {
    fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>, GuestMemError> {
        let mut buf = vec![0u8; len];
        self.uc
            .mem_read(addr, &mut buf)
            .map_err(|e| map_uc_error(e, addr, len))?;
        Ok(buf)
    }

    fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), GuestMemError> {
        self.uc
            .mem_write(addr, data)
            .map_err(|e| map_uc_error(e, addr, data.len()))
    }

    fn map_anon(&mut self, _base: u64, _len: usize, _prot: u32) -> Result<(), GuestMemError> {
        // Guest regions are mapped by the emulator owner, not by handlers.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_and_read() {
        let mut mem = InMemGuest::new();
        mem.map_anon(0x400000, 32, 3).unwrap();
        mem.write(0x400000, b"hello").unwrap();
        assert_eq!(mem.read(0x400000, 5).unwrap(), b"hello");
    }

    #[test]
    fn read_returns_io_error_outside_map() {
        let mut mem = InMemGuest::new();
        mem.map_anon(0x1000, 16, 3).unwrap();
        let err = mem.read(0x2000, 1).unwrap_err();
        assert_eq!(err.addr, 0x2000);
    }

    #[test]
    fn double_map_rejected() {
        let mut mem = InMemGuest::new();
        mem.map_anon(0x1000, 16, 3).unwrap();
        assert!(mem.map_anon(0x1000, 16, 3).is_err());
    }

    #[test]
    fn write_past_end_is_io_error() {
        let mut mem = InMemGuest::new();
        mem.map_anon(0x1000, 8, 3).unwrap();
        assert!(mem.write(0x1004, b"toolong").is_err());
    }
}
