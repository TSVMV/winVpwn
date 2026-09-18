//! Linux x86_64 syscall numbers implemented by the virtual kernel.

pub const READ: i64 = 0;
pub const WRITE: i64 = 1;
pub const OPEN: i64 = 2;
pub const CLOSE: i64 = 3;
pub const STAT: i64 = 4;
pub const FSTAT: i64 = 5;
pub const LSTAT: i64 = 6;
pub const POLL: i64 = 7;
pub const LSEEK: i64 = 8;
pub const MMAP: i64 = 9;
pub const MPROTECT: i64 = 10;
pub const MUNMAP: i64 = 11;
pub const BRK: i64 = 12;
pub const RT_SIGACTION: i64 = 13;
pub const RT_SIGPROCMASK: i64 = 14;
pub const IOCTL: i64 = 16;
pub const PREAD64: i64 = 17;
pub const PWRITE64: i64 = 18;
pub const READV: i64 = 19;
pub const WRITEV: i64 = 20;
pub const ACCESS: i64 = 21;
pub const PIPE: i64 = 22;
pub const DUP: i64 = 32;
pub const DUP2: i64 = 33;
pub const GETPID: i64 = 39;
pub const EXIT: i64 = 60;
pub const UNAME: i64 = 63;
pub const FCNTL: i64 = 72;
pub const GETDENTS: i64 = 78;
pub const GETCWD: i64 = 79;
pub const CHDIR: i64 = 80;
pub const READLINK: i64 = 89;
pub const GETTIMEOFDAY: i64 = 96;
pub const GETUID: i64 = 102;
pub const GETGID: i64 = 104;
pub const GETEUID: i64 = 107;
pub const GETEGID: i64 = 108;
pub const GETPPID: i64 = 110;
pub const ARCH_PRCTL: i64 = 158;
pub const GETTID: i64 = 186;
pub const TIME: i64 = 201;
pub const FUTEX: i64 = 202;
pub const SET_TID_ADDRESS: i64 = 218;
pub const CLOCK_GETTIME: i64 = 228;
pub const EXIT_GROUP: i64 = 231;
pub const OPENAT: i64 = 257;
pub const NEWFSTATAT: i64 = 262;
pub const SET_ROBUST_LIST: i64 = 273;
pub const GETRANDOM: i64 = 318;
pub const PRLIMIT64: i64 = 302;
pub const RSEQ: i64 = 334;

/// `AT_FDCWD` for `openat`/`newfstatat`.
pub const AT_FDCWD: i32 = -100;

/// `arch_prctl` codes.
pub const ARCH_SET_FS: i64 = 0x1002;
pub const ARCH_GET_FS: i64 = 0x1003;
pub const ARCH_SET_GS: i64 = 0x1001;
pub const ARCH_GET_GS: i64 = 0x1004;
