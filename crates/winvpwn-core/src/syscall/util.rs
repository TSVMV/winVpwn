//! Guest-memory helpers used by syscall handlers.

use crate::syscall::dispatch::DispatchError;
use crate::syscall::errno::{err, linux::EFAULT};
use crate::syscall::SyscallOutcome;
use crate::vkernel::Context;

/// Read a NUL-terminated string from guest memory, at most `max` bytes.
pub fn read_cstring(
    ctx: &Context<'_>,
    addr: u64,
    max: usize,
) -> Result<String, DispatchError> {
    if addr == 0 {
        return Err(DispatchError::Engine("null cstring".into()));
    }
    let mut buf = Vec::new();
    let mut cursor = addr;
    while buf.len() < max {
        let chunk = ctx
            .mem
            .read(cursor, 1)
            .map_err(|_| DispatchError::Engine("cstring read".into()))?;
        if chunk[0] == 0 {
            break;
        }
        buf.push(chunk[0]);
        cursor += 1;
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Guest-visible fault instead of an engine error.
pub fn fault() -> SyscallOutcome {
    SyscallOutcome::Return { ret: err(EFAULT) }
}

/// Round `addr` down to a page boundary.
pub fn page_down(addr: u64) -> u64 {
    addr & !0xfff
}

/// Round `addr` up to a page boundary.
pub fn page_up(addr: u64) -> u64 {
    (addr + 0xfff) & !0xfff
}

/// Linux `struct stat` (x86_64) size.
pub const STAT_SIZE: usize = 144;

/// Fill a `struct stat` for a regular file of `size` bytes.
pub fn fill_stat(size: u64, is_dir: bool) -> [u8; STAT_SIZE] {
    let mut buf = [0u8; STAT_SIZE];
    let mode: u32 = if is_dir { 0o040755 } else { 0o100644 };
    buf[0..8].copy_from_slice(&1u64.to_le_bytes());
    buf[8..16].copy_from_slice(&1u64.to_le_bytes());
    buf[16..24].copy_from_slice(&1u64.to_le_bytes());
    buf[24..28].copy_from_slice(&mode.to_le_bytes());
    buf[28..32].copy_from_slice(&1000u32.to_le_bytes());
    buf[32..36].copy_from_slice(&1000u32.to_le_bytes());
    buf[40..48].copy_from_slice(&0u64.to_le_bytes());
    buf[48..56].copy_from_slice(&size.to_le_bytes());
    buf[56..64].copy_from_slice(&4096u64.to_le_bytes());
    let blocks = size.div_ceil(512);
    buf[64..72].copy_from_slice(&blocks.to_le_bytes());
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_rounding() {
        assert_eq!(page_down(0x400123), 0x400000);
        assert_eq!(page_up(0x400000), 0x400000);
        assert_eq!(page_up(0x400001), 0x401000);
    }
}
