//! In-memory virtual filesystem.
//!
//! Host files enter the VFS only through explicit maps supplied at run start.
//! Guest `open`/`read`/`write` never touch the host path during emulation.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::syscall::errno::linux::{EACCES, EINVAL, EISDIR, ENOENT, ENOTDIR};

/// A file stored in the VFS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsFile {
    pub data: Vec<u8>,
    pub writable: bool,
    pub dirty: bool,
    /// Host path to flush to after exit; `None` for purely in-memory nodes.
    pub host_path: Option<PathBuf>,
}

/// Virtual filesystem keyed by normalized guest paths (`/a/b`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vfs {
    files: BTreeMap<String, VfsFile>,
}

impl Vfs {
    /// Empty VFS.
    pub fn new() -> Self {
        Self::default()
    }

    /// Install an explicit map from a host file (already read into `data`).
    pub fn map_file(
        &mut self,
        guest_path: &str,
        data: Vec<u8>,
        writable: bool,
        host_path: Option<PathBuf>,
    ) -> Result<(), i64> {
        let path = normalize(guest_path, "/")?;
        if path == "/" {
            return Err(EISDIR);
        }
        self.files.insert(
            path,
            VfsFile {
                data,
                writable,
                dirty: false,
                host_path,
            },
        );
        Ok(())
    }

    /// Look up a file.
    pub fn get(&self, path: &str) -> Option<&VfsFile> {
        self.files.get(path)
    }

    /// Look up a file, mutably.
    pub fn get_mut(&mut self, path: &str) -> Option<&mut VfsFile> {
        self.files.get_mut(path)
    }

    /// True when `path` is a mapped file.
    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// Create an in-memory file (guest `O_CREAT`). Never bound to a host path.
    pub fn create(&mut self, path: &str, writable: bool) -> Result<&mut VfsFile, i64> {
        if path == "/" {
            return Err(EISDIR);
        }
        self.files.entry(path.to_string()).or_insert(VfsFile {
            data: Vec::new(),
            writable,
            dirty: false,
            host_path: None,
        });
        self.files.get_mut(path).ok_or(ENOENT)
    }

    /// Truncate an existing file.
    pub fn truncate(&mut self, path: &str) -> Result<(), i64> {
        let f = self.files.get_mut(path).ok_or(ENOENT)?;
        if !f.writable {
            return Err(EACCES);
        }
        f.data.clear();
        f.dirty = true;
        Ok(())
    }

    /// Files that should be written back to the host after the guest exits.
    pub fn dirty_host_files(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.files
            .values()
            .filter(|f| f.dirty)
            .filter_map(|f| f.host_path.as_ref().map(|p| (p.clone(), f.data.clone())))
            .collect()
    }
}

/// Join `path` against `cwd` and collapse `.` / `..`.
pub fn normalize(path: &str, cwd: &str) -> Result<String, i64> {
    if path.is_empty() {
        return Err(EINVAL);
    }
    let combined = if path.starts_with('/') {
        path.to_string()
    } else {
        let cwd = if cwd.is_empty() { "/" } else { cwd };
        if cwd.ends_with('/') {
            format!("{cwd}{path}")
        } else {
            format!("{cwd}/{path}")
        }
    };
    let mut out: Vec<&str> = Vec::new();
    for part in combined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    if out.is_empty() {
        Ok("/".into())
    } else {
        Ok(format!("/{}", out.join("/")))
    }
}

/// True when `path` should be treated as a directory (the root, or a prefix of a file).
pub fn is_dir(vfs: &Vfs, path: &str) -> bool {
    if path == "/" {
        return true;
    }
    let prefix = format!("{path}/");
    vfs.files.keys().any(|p| p.starts_with(&prefix))
}

/// Open-time type check: file vs directory.
pub fn check_file(vfs: &Vfs, path: &str) -> Result<(), i64> {
    if vfs.contains(path) {
        return Ok(());
    }
    if is_dir(vfs, path) {
        return Err(EISDIR);
    }
    Err(ENOENT)
}

/// Open-time directory check.
pub fn check_dir(vfs: &Vfs, path: &str) -> Result<(), i64> {
    if path == "/" || is_dir(vfs, path) {
        return Ok(());
    }
    if vfs.contains(path) {
        return Err(ENOTDIR);
    }
    Err(ENOENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_dotdot() {
        assert_eq!(normalize("/a/b/../c", "/").unwrap(), "/a/c");
        assert_eq!(normalize("x", "/tmp").unwrap(), "/tmp/x");
        assert_eq!(normalize("..", "/tmp").unwrap(), "/");
    }

    #[test]
    fn map_and_lookup() {
        let mut vfs = Vfs::new();
        vfs.map_file("/flag", b"hi".to_vec(), false, None).unwrap();
        assert_eq!(vfs.get("/flag").unwrap().data, b"hi");
        assert!(check_file(&vfs, "/flag").is_ok());
        assert_eq!(check_file(&vfs, "/missing").unwrap_err(), ENOENT);
    }

    #[test]
    fn create_in_memory() {
        let mut vfs = Vfs::new();
        vfs.create("/tmp/x", true).unwrap();
        vfs.get_mut("/tmp/x").unwrap().data.extend_from_slice(b"ab");
        assert_eq!(vfs.get("/tmp/x").unwrap().data, b"ab");
        assert!(vfs.dirty_host_files().is_empty());
    }
}
