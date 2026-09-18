//! Linux errno constants used by the syscall layer.

/// Negative errno values returned to the guest on failure.
pub mod linux {
    /// `EPERM` — operation not permitted.
    pub const EPERM: i64 = 1;
    /// `ENOENT` — no such file or directory.
    pub const ENOENT: i64 = 2;
    /// `EIO` — I/O error.
    pub const EIO: i64 = 5;
    /// `ENXIO` — no such device or address.
    pub const ENXIO: i64 = 6;
    /// `EBADF` — bad file descriptor.
    pub const EBADF: i64 = 9;
    /// `ENOMEM` — out of memory.
    pub const ENOMEM: i64 = 12;
    /// `EACCES` — permission denied.
    pub const EACCES: i64 = 13;
    /// `EFAULT` — bad address.
    pub const EFAULT: i64 = 14;
    /// `EBUSY` — device or resource busy.
    pub const EBUSY: i64 = 16;
    /// `EEXIST` — file exists.
    pub const EEXIST: i64 = 17;
    /// `ENOTDIR` — not a directory.
    pub const ENOTDIR: i64 = 20;
    /// `EISDIR` — is a directory.
    pub const EISDIR: i64 = 21;
    /// `EINVAL` — invalid argument.
    pub const EINVAL: i64 = 22;
    /// `ENFILE` — file table overflow.
    pub const ENFILE: i64 = 23;
    /// `EMFILE` — too many open files.
    pub const EMFILE: i64 = 24;
    /// `ENOSPC` — no space left on device.
    pub const ENOSPC: i64 = 28;
    /// `ESPIPE` — illegal seek.
    pub const ESPIPE: i64 = 29;
    /// `ERANGE` — result too large.
    pub const ERANGE: i64 = 34;
    /// `ENOSYS` — function not implemented.
    pub const ENOSYS: i64 = 38;
    /// `ENOTEMPTY` — directory not empty.
    pub const ENOTEMPTY: i64 = 39;
    /// `ENOTTY` — not a typewriter.
    pub const ENOTTY: i64 = 25;
    /// `EAGAIN` — try again.
    pub const EAGAIN: i64 = 11;
}

/// Wrap a Linux errno as a guest-facing negative value.
pub fn err(errno: i64) -> i64 {
    -errno
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_errno() {
        assert_eq!(err(linux::ENOSYS), -38);
        assert_eq!(err(linux::ENOENT), -2);
    }
}
