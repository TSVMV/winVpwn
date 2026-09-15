//! ELF loading: parse an ELF image and produce loadable segments.
//!
//! Stage 1 supports static `ET_EXEC` x86_64 images only. `PT_LOAD` segments
//! are extracted with their virtual addresses and mapped with the requested
//! page permissions; overlapping or out-of-bounds segments are rejected.

use std::fmt;

use goblin::elf::header::{EM_X86_64, ET_DYN, ET_EXEC};
use goblin::elf::program_header::{PF_R, PF_W, PF_X, PT_LOAD};
use goblin::elf::Elf;

use crate::memory::mmap::MemoryMap;

/// A single loadable `PT_LOAD` segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Virtual address where the segment is loaded.
    pub vaddr: u64,
    /// Size of the segment in memory (>= `filesz`; tail is zero-filled `.bss`).
    pub memsz: u64,
    /// Size of the segment on disk.
    pub filesz: u64,
    /// File content for the first `filesz` bytes.
    pub data: Vec<u8>,
    /// ELF `p_flags` bits (`PF_R`/`PF_W`/`PF_X`).
    pub flags: u32,
    /// Alignment of the segment (`p_align`).
    pub align: u64,
}

impl Segment {
    /// The file-backed bytes of this segment.
    pub fn file_bytes(&self) -> &[u8] {
        &self.data[..self.filesz as usize]
    }
}

/// A fully loaded ELF image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedElf {
    /// Program entry point.
    pub entry: u64,
    /// All `PT_LOAD` segments, in program-header order.
    pub segments: Vec<Segment>,
    /// True when the image is position-independent (`ET_DYN`).
    pub is_pie: bool,
}

/// Loader errors. Each variant carries a stable error code (E0xx) used by the
/// CLI and the audit log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfError {
    /// The input does not start with the ELF magic bytes.
    NotElf,
    /// The image is not 64-bit little-endian (stage 1 scope).
    UnsupportedFormat(&'static str),
    /// The ELF machine is not x86_64.
    UnsupportedMachine,
    /// The ELF type is not supported in stage 1.
    UnsupportedType { found: u16 },
    /// The program header is malformed.
    BadPhdr(String),
    /// A segment lies outside the image bytes.
    Truncated(String),
    /// Two loadable segments overlap in the guest address space.
    SegmentOverlap { vaddr: u64, other: u64 },
}

impl ElfError {
    /// Stable machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            ElfError::NotElf => "E001",
            ElfError::UnsupportedFormat(_) => "E002",
            ElfError::UnsupportedMachine => "E003",
            ElfError::UnsupportedType { .. } => "E004",
            ElfError::BadPhdr(_) => "E005",
            ElfError::Truncated(_) => "E006",
            ElfError::SegmentOverlap { .. } => "E007",
        }
    }
}

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElfError::NotElf => write!(f, "{}: not an ELF file (bad magic)", self.code()),
            ElfError::UnsupportedFormat(what) => {
                write!(f, "{}: unsupported image format: {}", self.code(), what)
            }
            ElfError::UnsupportedMachine => {
                write!(
                    f,
                    "{}: unsupported machine (stage 1 supports x86_64)",
                    self.code()
                )
            }
            ElfError::UnsupportedType { found } => write!(
                f,
                "{}: unsupported ELF type {} (stage 1 supports static ET_EXEC)",
                self.code(),
                found
            ),
            ElfError::BadPhdr(reason) => {
                write!(f, "{}: malformed program header: {}", self.code(), reason)
            }
            ElfError::Truncated(reason) => {
                write!(f, "{}: truncated image: {}", self.code(), reason)
            }
            ElfError::SegmentOverlap { vaddr, other } => write!(
                f,
                "{}: loadable segment at 0x{vaddr:x} overlaps segment at 0x{other:x}",
                self.code()
            ),
        }
    }
}

impl std::error::Error for ElfError {}

/// Parse and validate `image` and return its loadable segments.
pub fn load_elf(image: &[u8]) -> Result<LoadedElf, ElfError> {
    if image.len() < 4 || &image[..4] != b"\x7fELF" {
        return Err(ElfError::NotElf);
    }
    let elf = Elf::parse(image).map_err(|e| ElfError::BadPhdr(e.to_string()))?;
    if !elf.is_64 {
        return Err(ElfError::UnsupportedFormat(
            "32-bit ELF (stage 1 is x86_64 only)",
        ));
    }
    if elf.header.e_machine != EM_X86_64 {
        return Err(ElfError::UnsupportedMachine);
    }

    let is_pie = elf.header.e_type == ET_DYN;
    if elf.header.e_type != ET_EXEC {
        if is_pie {
            return Err(ElfError::UnsupportedType {
                found: elf.header.e_type,
            });
        }
        return Err(ElfError::UnsupportedType {
            found: elf.header.e_type,
        });
    }

    let mut segments = Vec::new();
    let mut map = MemoryMap::new();
    for phdr in elf.program_headers.iter().filter(|p| p.p_type == PT_LOAD) {
        let end = phdr
            .p_vaddr
            .checked_add(phdr.p_memsz)
            .ok_or_else(|| ElfError::BadPhdr("PT_LOAD vaddr+memsz overflows".into()))?;
        if map
            .insert(phdr.p_vaddr, end - phdr.p_vaddr, phdr.p_flags)
            .is_err()
        {
            let other = map
                .regions()
                .iter()
                .find(|r| r.contains(phdr.p_vaddr) || phdr.p_vaddr == r.base)
                .map(|r| r.base)
                .unwrap_or(0);
            return Err(ElfError::SegmentOverlap {
                vaddr: phdr.p_vaddr,
                other,
            });
        }

        let file_end = phdr
            .p_offset
            .checked_add(phdr.p_filesz)
            .ok_or_else(|| ElfError::Truncated("PT_LOAD offset+filesz overflows".into()))?;
        if file_end > image.len() as u64 {
            return Err(ElfError::Truncated(format!(
                "PT_LOAD file range {:#x}..{:#x} exceeds image size {:#x}",
                phdr.p_offset,
                file_end,
                image.len()
            )));
        }

        let mut data = vec![0u8; phdr.p_memsz as usize];
        let file = &image[phdr.p_offset as usize..file_end as usize];
        data[..file.len()].copy_from_slice(file);

        segments.push(Segment {
            vaddr: phdr.p_vaddr,
            memsz: phdr.p_memsz,
            filesz: phdr.p_filesz,
            data,
            flags: phdr.p_flags,
            align: phdr.p_align,
        });
    }

    if segments.is_empty() {
        return Err(ElfError::BadPhdr("no PT_LOAD segments".into()));
    }

    Ok(LoadedElf {
        entry: elf.entry,
        segments,
        is_pie,
    })
}

/// Human-readable permission string for `p_flags` (e.g. `R-X`).
pub fn perms_string(flags: u32) -> String {
    let mut s = String::with_capacity(3);
    s.push(if (flags & PF_R) != 0 { 'R' } else { '-' });
    s.push(if (flags & PF_W) != 0 { 'W' } else { '-' });
    s.push(if (flags & PF_X) != 0 { 'X' } else { '-' });
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &[u8] = include_bytes!("../../tests/fixtures/hello_static");

    #[test]
    fn loads_static_exec() {
        let elf = load_elf(HELLO).expect("fixture must load");
        assert!(elf.entry >= 0x400000, "entry 0x{:x} looks wrong", elf.entry);
        assert!(!elf.segments.is_empty());
        assert!(elf.segments.iter().any(|s| (s.flags & PF_X) != 0));
        assert!(elf.segments.iter().any(|s| (s.flags & PF_R) != 0));
    }

    #[test]
    fn rejects_non_elf() {
        let err = load_elf(b"MZ\x90\x00junk").unwrap_err();
        assert_eq!(err.code(), "E001");
    }

    #[test]
    fn rejects_truncated_image() {
        let err = load_elf(&HELLO[..40]).unwrap_err();
        assert!(matches!(err, ElfError::BadPhdr(_) | ElfError::Truncated(_)));
    }

    #[test]
    fn rejects_pie_in_stage1() {
        // Flip e_type of the fixture to ET_DYN (offset 16) and re-hash.
        let mut bytes = HELLO.to_vec();
        bytes[16..18].copy_from_slice(&ET_DYN.to_le_bytes());
        let err = load_elf(&bytes).unwrap_err();
        assert_eq!(err.code(), "E004");
    }

    #[test]
    fn perms_mapping() {
        assert_eq!(perms_string(PF_R), "R--");
        assert_eq!(perms_string(PF_R | PF_W), "RW-");
        assert_eq!(perms_string(PF_R | PF_X), "R-X");
    }

    #[test]
    fn file_bytes_match_memsz_bss() {
        let elf = load_elf(HELLO).unwrap();
        for seg in &elf.segments {
            assert_eq!(seg.file_bytes().len() as u64, seg.filesz);
            assert!(seg.filesz <= seg.memsz);
        }
    }
}
