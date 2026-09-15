//! Captured process output.
//!
//! `write` never touches a host descriptor: bytes are appended to in-memory
//! buffers owned by the virtual process. The Python layer decides how to
//! present them (terminal, log file, JSON audit).

use std::collections::BTreeMap;

/// Captured stdout/stderr of a simulated process.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OutputCapture {
    buffers: BTreeMap<i64, Vec<u8>>,
}

impl OutputCapture {
    /// Append `data` to the buffer of file descriptor `fd`.
    pub fn write(&mut self, fd: i64, data: &[u8]) {
        self.buffers.entry(fd).or_default().extend_from_slice(data);
    }

    /// The captured bytes for `fd` (empty when nothing was written).
    pub fn get(&self, fd: i64) -> &[u8] {
        self.buffers.get(&fd).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The captured stdout bytes.
    pub fn stdout(&self) -> &[u8] {
        self.get(1)
    }

    /// The captured stderr bytes.
    pub fn stderr(&self) -> &[u8] {
        self.get(2)
    }

    /// Combined stdout+stderr in fd order.
    pub fn combined(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (fd, bytes) in self.buffers.iter() {
            if *fd == 1 || *fd == 2 {
                out.extend_from_slice(bytes);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_stdout() {
        let mut cap = OutputCapture::default();
        cap.write(1, b"a");
        cap.write(1, b"b");
        assert_eq!(cap.stdout(), b"ab");
        assert!(cap.stderr().is_empty());
    }

    #[test]
    fn capture_stderr_separately() {
        let mut cap = OutputCapture::default();
        cap.write(2, b"err");
        assert!(cap.stdout().is_empty());
        assert_eq!(cap.stderr(), b"err");
    }

    #[test]
    fn combined_in_fd_order() {
        let mut cap = OutputCapture::default();
        cap.write(2, b"e");
        cap.write(1, b"o");
        assert_eq!(cap.combined(), b"oe");
    }
}
