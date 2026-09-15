//! Linux errno constants used by the syscall layer.

/// Negative errno values returned to the guest on failure.
pub mod linux {
    /// `ENOSYS` — function not implemented.
    pub const ENOSYS: i64 = 38;
    /// `EINVAL` — invalid argument.
    pub const EINVAL: i64 = 22;
    /// `EBADF` — bad file descriptor.
    pub const EBADF: i64 = 9;
    /// `EFAULT` — bad address.
    pub const EFAULT: i64 = 14;
}

/// Wrap a Linux errno as a guest-facing negative value.
pub fn err(errno: i64) -> i64 {
    -errno
}
