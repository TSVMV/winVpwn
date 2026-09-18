//! Virtual file-descriptor table.
//!
//! Descriptors never wrap host fds. Stdio is in-memory; regular files point at
//! VFS nodes created from explicit maps or guest `O_CREAT`.

use std::collections::BTreeMap;

/// Kind of a live descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FdKind {
    /// Standard input, read from the kernel stdin buffer.
    Stdin { cursor: u64 },
    /// Standard output, captured into [`crate::vkernel::io::OutputCapture`].
    Stdout,
    /// Standard error, captured into [`crate::vkernel::io::OutputCapture`].
    Stderr,
    /// A regular VFS file.
    File {
        path: String,
        cursor: u64,
        writable: bool,
        append: bool,
    },
    /// A directory fd used only as a base for `openat`.
    Dir { path: String },
}

/// One slot in the descriptor table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fd {
    pub kind: FdKind,
    /// `O_CLOEXEC` flag (recorded, not acted on: no exec).
    pub cloexec: bool,
}

/// Process file-descriptor table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdTable {
    next: i32,
    entries: BTreeMap<i32, Fd>,
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

impl FdTable {
    /// Stdin, stdout, stderr.
    pub fn new() -> Self {
        let mut entries = BTreeMap::new();
        entries.insert(
            0,
            Fd {
                kind: FdKind::Stdin { cursor: 0 },
                cloexec: false,
            },
        );
        entries.insert(
            1,
            Fd {
                kind: FdKind::Stdout,
                cloexec: false,
            },
        );
        entries.insert(
            2,
            Fd {
                kind: FdKind::Stderr,
                cloexec: false,
            },
        );
        Self { next: 3, entries }
    }

    /// Look up a live descriptor.
    pub fn get(&self, fd: i32) -> Option<&Fd> {
        self.entries.get(&fd)
    }

    /// Look up a live descriptor, mutably.
    pub fn get_mut(&mut self, fd: i32) -> Option<&mut Fd> {
        self.entries.get_mut(&fd)
    }

    /// Insert `fd` at the lowest unused slot >= `min`, return the slot.
    pub fn insert(&mut self, min: i32, desc: Fd) -> i32 {
        let mut slot = min.max(0);
        while self.entries.contains_key(&slot) {
            slot += 1;
        }
        self.entries.insert(slot, desc);
        if slot >= self.next {
            self.next = slot + 1;
        }
        slot
    }

    /// Close `fd`. Returns false when it was not open.
    pub fn close(&mut self, fd: i32) -> bool {
        self.entries.remove(&fd).is_some()
    }

    /// Duplicate `fd` onto the lowest unused slot >= `min`.
    pub fn dup(&mut self, fd: i32, min: i32) -> Option<i32> {
        let desc = self.entries.get(&fd)?.clone();
        Some(self.insert(min, desc))
    }

    /// Duplicate `fd` onto `newfd`, closing `newfd` first if needed.
    pub fn dup2(&mut self, fd: i32, newfd: i32) -> Option<i32> {
        if fd == newfd {
            return self.entries.contains_key(&fd).then_some(newfd);
        }
        let desc = self.entries.get(&fd)?.clone();
        self.entries.insert(newfd, desc);
        if newfd >= self.next {
            self.next = newfd + 1;
        }
        Some(newfd)
    }

    /// True when `fd` is currently open.
    pub fn is_open(&self, fd: i32) -> bool {
        self.entries.contains_key(&fd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_preinstalled() {
        let t = FdTable::new();
        assert!(matches!(t.get(0).unwrap().kind, FdKind::Stdin { .. }));
        assert!(matches!(t.get(1).unwrap().kind, FdKind::Stdout));
        assert!(matches!(t.get(2).unwrap().kind, FdKind::Stderr));
    }

    #[test]
    fn insert_and_close() {
        let mut t = FdTable::new();
        let fd = t.insert(
            0,
            Fd {
                kind: FdKind::File {
                    path: "/a".into(),
                    cursor: 0,
                    writable: false,
                    append: false,
                },
                cloexec: false,
            },
        );
        assert_eq!(fd, 3);
        assert!(t.close(fd));
        assert!(!t.is_open(fd));
    }

    #[test]
    fn dup_copies_slot() {
        let mut t = FdTable::new();
        let n = t.dup(1, 0).unwrap();
        assert_eq!(n, 3);
        assert!(matches!(t.get(n).unwrap().kind, FdKind::Stdout));
    }
}
