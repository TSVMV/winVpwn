//! Syscall handlers for the virtual kernel.
//!
//! Stage 2–4: stdio, explicit VFS maps, brk/mmap, and the libc bootstrap
//! set used by static musl/glibc images. Guest I/O never touches a host
//! descriptor during emulation.

use std::sync::Arc;

use crate::syscall::dispatch::{DispatchError, SyscallHandler, SyscallOutcome, SyscallRegs};
use crate::syscall::errno::{err, linux};
use crate::syscall::nr;
use crate::syscall::util::{fault, fill_stat, page_down, page_up, read_cstring, STAT_SIZE};
use crate::vkernel::fd::{Fd, FdKind};
use crate::vkernel::process::ProcessState;
use crate::vkernel::vfs::{self, normalize};
use crate::vkernel::{Context, ExitReason};

const O_ACCMODE: i32 = 0o3;
const O_WRONLY: i32 = 1;
const O_RDWR: i32 = 2;
const O_CREAT: i32 = 0o100;
const O_EXCL: i32 = 0o200;
const O_TRUNC: i32 = 0o1000;
const O_APPEND: i32 = 0o2000;
const O_DIRECTORY: i32 = 0o200000;
const O_CLOEXEC: i32 = 0o2000000;

const SEEK_SET: i32 = 0;
const SEEK_CUR: i32 = 1;
const SEEK_END: i32 = 2;

const PROT_READ: u32 = 1;
const MAP_FIXED: u32 = 0x10;
const MAP_ANONYMOUS: u32 = 0x20;

const F_DUPFD: i32 = 0;
const F_GETFD: i32 = 1;
const F_SETFD: i32 = 2;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const F_DUPFD_CLOEXEC: i32 = 1030;

macro_rules! handler {
    ($ty:ident, $nr:expr, $name:expr, $body:expr) => {
        pub struct $ty;
        impl SyscallHandler for $ty {
            fn number(&self) -> i64 {
                $nr
            }
            fn name(&self) -> &'static str {
                $name
            }
            fn handle(
                &self,
                ctx: &mut Context<'_>,
                regs: SyscallRegs,
            ) -> Result<SyscallOutcome, DispatchError> {
                $body(ctx, regs)
            }
        }
    };
}

fn ret(v: i64) -> Result<SyscallOutcome, DispatchError> {
    Ok(SyscallOutcome::Return { ret: v })
}

fn e(n: i64) -> Result<SyscallOutcome, DispatchError> {
    ret(err(n))
}

handler!(ReadHandler, nr::READ, "read", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_read(ctx, regs.arg0 as i32, regs.arg1, regs.arg2 as usize, None)
});

handler!(WriteHandler, nr::WRITE, "write", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_write(ctx, regs.arg0 as i32, regs.arg1, regs.arg2 as usize, None)
});

handler!(OpenHandler, nr::OPEN, "open", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg0, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    do_open(ctx, None, &path, regs.arg1 as i32)
});

handler!(CloseHandler, nr::CLOSE, "close", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let fd = regs.arg0 as i32;
    if ctx.kernel.fds.close(fd) {
        ret(0)
    } else {
        e(linux::EBADF)
    }
});

handler!(StatHandler, nr::STAT, "stat", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg0, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    do_stat_path(ctx, &path, regs.arg1)
});

handler!(FstatHandler, nr::FSTAT, "fstat", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_fstat(ctx, regs.arg0 as i32, regs.arg1)
});

handler!(LstatHandler, nr::LSTAT, "lstat", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg0, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    do_stat_path(ctx, &path, regs.arg1)
});

handler!(PollHandler, nr::POLL, "poll", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

handler!(LseekHandler, nr::LSEEK, "lseek", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_lseek(ctx, regs.arg0 as i32, regs.arg1 as i64, regs.arg2 as i32)
});

handler!(MmapHandler, nr::MMAP, "mmap", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_mmap(
        ctx,
        regs.arg0,
        regs.arg1,
        regs.arg2 as u32,
        regs.arg3 as u32,
        regs.arg4 as i32,
        regs.arg5 as i64,
    )
});

handler!(MprotectHandler, nr::MPROTECT, "mprotect", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let addr = page_down(regs.arg0);
    let len = page_up(regs.arg1) as usize;
    let prot = regs.arg2 as u32;
    match ctx.mem.protect(addr, len, prot) {
        Ok(()) => ret(0),
        Err(_) => e(linux::ENOMEM),
    }
});

handler!(MunmapHandler, nr::MUNMAP, "munmap", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let addr = page_down(regs.arg0);
    let len = page_up(regs.arg1) as usize;
    match ctx.mem.unmap(addr, len) {
        Ok(()) => ret(0),
        Err(_) => e(linux::EINVAL),
    }
});

handler!(BrkHandler, nr::BRK, "brk", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_brk(ctx, regs.arg0)
});

handler!(RtSigactionHandler, nr::RT_SIGACTION, "rt_sigaction", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

handler!(RtSigprocmaskHandler, nr::RT_SIGPROCMASK, "rt_sigprocmask", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

handler!(IoctlHandler, nr::IOCTL, "ioctl", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let fd = regs.arg0 as i32;
    if !ctx.kernel.fds.is_open(fd) {
        return e(linux::EBADF);
    }
    e(linux::ENOTTY)
});

handler!(PreadHandler, nr::PREAD64, "pread64", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_read(
        ctx,
        regs.arg0 as i32,
        regs.arg1,
        regs.arg2 as usize,
        Some(regs.arg3),
    )
});

handler!(PwriteHandler, nr::PWRITE64, "pwrite64", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_write(
        ctx,
        regs.arg0 as i32,
        regs.arg1,
        regs.arg2 as usize,
        Some(regs.arg3),
    )
});

handler!(ReadvHandler, nr::READV, "readv", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_readv(ctx, regs.arg0 as i32, regs.arg1, regs.arg2 as i64)
});

handler!(WritevHandler, nr::WRITEV, "writev", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_writev(ctx, regs.arg0 as i32, regs.arg1, regs.arg2 as i64)
});

handler!(AccessHandler, nr::ACCESS, "access", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg0, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    let abs = match normalize(&path, &ctx.kernel.cwd) {
        Ok(p) => p,
        Err(n) => return e(n),
    };
    if ctx.kernel.vfs.contains(&abs) || vfs::is_dir(&ctx.kernel.vfs, &abs) {
        ret(0)
    } else {
        e(linux::ENOENT)
    }
});

handler!(DupHandler, nr::DUP, "dup", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    match ctx.kernel.fds.dup(regs.arg0 as i32, 0) {
        Some(n) => ret(n as i64),
        None => e(linux::EBADF),
    }
});

handler!(Dup2Handler, nr::DUP2, "dup2", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    match ctx.kernel.fds.dup2(regs.arg0 as i32, regs.arg1 as i32) {
        Some(n) => ret(n as i64),
        None => e(linux::EBADF),
    }
});

handler!(GetpidHandler, nr::GETPID, "getpid", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    ret(ctx.kernel.pid as i64)
});

handler!(ExitHandler, nr::EXIT, "exit", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let code = (regs.arg0 & 0xff) as u8;
    ctx.kernel.process = ProcessState::Exited { code };
    Ok(SyscallOutcome::Exit(ExitReason::Exit { code }))
});

handler!(UnameHandler, nr::UNAME, "uname", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let mut buf = [0u8; 390];
    write_uts(&mut buf[0..65], b"Linux");
    write_uts(&mut buf[65..130], b"winvpwn");
    write_uts(&mut buf[130..195], b"6.1.0");
    write_uts(&mut buf[195..260], b"#1 SMP");
    write_uts(&mut buf[260..325], b"x86_64");
    write_uts(&mut buf[325..390], b"gnu");
    match ctx.mem.write(regs.arg0, &buf) {
        Ok(()) => ret(0),
        Err(_) => Ok(fault()),
    }
});

handler!(FcntlHandler, nr::FCNTL, "fcntl", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    do_fcntl(ctx, regs.arg0 as i32, regs.arg1 as i32, regs.arg2)
});

handler!(GetcwdHandler, nr::GETCWD, "getcwd", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let mut bytes = ctx.kernel.cwd.as_bytes().to_vec();
    bytes.push(0);
    if bytes.len() as u64 > regs.arg1 {
        return e(linux::ERANGE);
    }
    match ctx.mem.write(regs.arg0, &bytes) {
        Ok(()) => ret(regs.arg0 as i64),
        Err(_) => Ok(fault()),
    }
});

handler!(ChdirHandler, nr::CHDIR, "chdir", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg0, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    let abs = match normalize(&path, &ctx.kernel.cwd) {
        Ok(p) => p,
        Err(n) => return e(n),
    };
    match vfs::check_dir(&ctx.kernel.vfs, &abs) {
        Ok(()) => {
            ctx.kernel.cwd = abs;
            ret(0)
        }
        Err(n) => e(n),
    }
});

handler!(ReadlinkHandler, nr::READLINK, "readlink", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg0, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    let abs = match normalize(&path, &ctx.kernel.cwd) {
        Ok(p) => p,
        Err(n) => return e(n),
    };
    if abs == "/proc/self/exe" {
        let target = ctx.kernel.exec_path.as_bytes();
        let n = target.len().min(regs.arg2 as usize);
        match ctx.mem.write(regs.arg1, &target[..n]) {
            Ok(()) => ret(n as i64),
            Err(_) => Ok(fault()),
        }
    } else {
        e(linux::EINVAL)
    }
});

handler!(GettimeofdayHandler, nr::GETTIMEOFDAY, "gettimeofday", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    if regs.arg0 != 0 {
        let sec = ctx.kernel.clock_ns / 1_000_000_000;
        let usec = (ctx.kernel.clock_ns % 1_000_000_000) / 1000;
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&sec.to_le_bytes());
        buf[8..16].copy_from_slice(&usec.to_le_bytes());
        if ctx.mem.write(regs.arg0, &buf).is_err() {
            return Ok(fault());
        }
    }
    ret(0)
});

handler!(GetuidHandler, nr::GETUID, "getuid", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    ret(ctx.kernel.uid as i64)
});
handler!(GetgidHandler, nr::GETGID, "getgid", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    ret(ctx.kernel.gid as i64)
});
handler!(GeteuidHandler, nr::GETEUID, "geteuid", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    ret(ctx.kernel.uid as i64)
});
handler!(GetegidHandler, nr::GETEGID, "getegid", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    ret(ctx.kernel.gid as i64)
});
handler!(GetppidHandler, nr::GETPPID, "getppid", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

handler!(ArchPrctlHandler, nr::ARCH_PRCTL, "arch_prctl", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    match regs.arg0 as i64 {
        nr::ARCH_SET_FS => {
            ctx.kernel.fs_base = regs.arg1;
            match ctx.mem.set_fs_base(regs.arg1) {
                Ok(()) => ret(0),
                Err(_) => e(linux::EFAULT),
            }
        }
        nr::ARCH_GET_FS => match ctx.mem.write(regs.arg1, &ctx.kernel.fs_base.to_le_bytes()) {
            Ok(()) => ret(0),
            Err(_) => Ok(fault()),
        },
        _ => e(linux::EINVAL),
    }
});

handler!(GettidHandler, nr::GETTID, "gettid", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    ret(ctx.kernel.pid as i64)
});

handler!(TimeHandler, nr::TIME, "time", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let sec = (ctx.kernel.clock_ns / 1_000_000_000) as i64;
    if regs.arg0 != 0 {
        if ctx.mem.write(regs.arg0, &sec.to_le_bytes()).is_err() {
            return Ok(fault());
        }
    }
    ret(sec)
});

handler!(FutexHandler, nr::FUTEX, "futex", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

handler!(SetTidAddressHandler, nr::SET_TID_ADDRESS, "set_tid_address", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    ctx.kernel.clear_tid = regs.arg0;
    ret(ctx.kernel.pid as i64)
});

handler!(ClockGettimeHandler, nr::CLOCK_GETTIME, "clock_gettime", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let sec = ctx.kernel.clock_ns / 1_000_000_000;
    let nsec = ctx.kernel.clock_ns % 1_000_000_000;
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&sec.to_le_bytes());
    buf[8..16].copy_from_slice(&nsec.to_le_bytes());
    match ctx.mem.write(regs.arg1, &buf) {
        Ok(()) => ret(0),
        Err(_) => Ok(fault()),
    }
});

handler!(ExitGroupHandler, nr::EXIT_GROUP, "exit_group", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let code = (regs.arg0 & 0xff) as u8;
    ctx.kernel.process = ProcessState::Exited { code };
    Ok(SyscallOutcome::Exit(ExitReason::Exit { code }))
});

handler!(OpenatHandler, nr::OPENAT, "openat", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg1, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    do_open(ctx, Some(regs.arg0 as i32), &path, regs.arg2 as i32)
});

handler!(NewfstatatHandler, nr::NEWFSTATAT, "newfstatat", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let path = match read_cstring(ctx, regs.arg1, 4096) {
        Ok(p) => p,
        Err(_) => return Ok(fault()),
    };
    let dirfd = regs.arg0 as i32;
    let base = dir_base(ctx, dirfd);
    let abs = match normalize(&path, &base) {
        Ok(p) => p,
        Err(n) => return e(n),
    };
    do_stat_abs(ctx, &abs, regs.arg2)
});

handler!(SetRobustListHandler, nr::SET_ROBUST_LIST, "set_robust_list", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

handler!(PrlimitHandler, nr::PRLIMIT64, "prlimit64", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

handler!(GetrandomHandler, nr::GETRANDOM, "getrandom", |ctx: &mut Context<'_>, regs: SyscallRegs| {
    let n = regs.arg1 as usize;
    let mut buf = vec![0u8; n];
    let mut i = 0;
    while i < n {
        let r = ctx.kernel.next_rand().to_le_bytes();
        let take = (n - i).min(8);
        buf[i..i + take].copy_from_slice(&r[..take]);
        i += take;
    }
    match ctx.mem.write(regs.arg0, &buf) {
        Ok(()) => ret(n as i64),
        Err(_) => Ok(fault()),
    }
});

handler!(RseqHandler, nr::RSEQ, "rseq", |ctx: &mut Context<'_>, _regs: SyscallRegs| {
    let _ = ctx;
    ret(0)
});

fn write_uts(slot: &mut [u8], s: &[u8]) {
    let n = s.len().min(slot.len() - 1);
    slot[..n].copy_from_slice(&s[..n]);
}

fn dir_base(ctx: &Context<'_>, dirfd: i32) -> String {
    if dirfd == nr::AT_FDCWD || dirfd < 0 {
        return ctx.kernel.cwd.clone();
    }
    match ctx.kernel.fds.get(dirfd).map(|d| &d.kind) {
        Some(FdKind::Dir { path }) => path.clone(),
        Some(FdKind::File { path, .. }) => path.clone(),
        _ => ctx.kernel.cwd.clone(),
    }
}

fn do_open(
    ctx: &mut Context<'_>,
    dirfd: Option<i32>,
    path: &str,
    flags: i32,
) -> Result<SyscallOutcome, DispatchError> {
    let base = match dirfd {
        Some(fd) => dir_base(ctx, fd),
        None => ctx.kernel.cwd.clone(),
    };
    let abs = match normalize(path, &base) {
        Ok(p) => p,
        Err(n) => return e(n),
    };
    let acc = flags & O_ACCMODE;
    let writable = acc == O_WRONLY || acc == O_RDWR;
    let creat = flags & O_CREAT != 0;
    let excl = flags & O_EXCL != 0;
    let trunc = flags & O_TRUNC != 0;
    let append = flags & O_APPEND != 0;
    let directory = flags & O_DIRECTORY != 0;
    let cloexec = flags & O_CLOEXEC != 0;

    if directory {
        match vfs::check_dir(&ctx.kernel.vfs, &abs) {
            Ok(()) => {
                let fd = ctx.kernel.fds.insert(
                    0,
                    Fd {
                        kind: FdKind::Dir { path: abs },
                        cloexec,
                    },
                );
                return ret(fd as i64);
            }
            Err(n) => return e(n),
        }
    }

    let exists = ctx.kernel.vfs.contains(&abs);
    if exists && excl && creat {
        return e(linux::EEXIST);
    }
    if !exists {
        if creat {
            if ctx.kernel.vfs.create(&abs, true).is_err() {
                return e(linux::EACCES);
            }
        } else {
            return e(linux::ENOENT);
        }
    }
    if trunc {
        let _ = ctx.kernel.vfs.truncate(&abs);
    }
    let file = match ctx.kernel.vfs.get(&abs) {
        Some(f) => f,
        None => return e(linux::ENOENT),
    };
    if writable && !file.writable && file.host_path.is_some() {
        return e(linux::EACCES);
    }
    let cursor = if append { file.data.len() as u64 } else { 0 };
    let fd = ctx.kernel.fds.insert(
        0,
        Fd {
            kind: FdKind::File {
                path: abs,
                cursor,
                writable,
                append,
            },
            cloexec,
        },
    );
    ret(fd as i64)
}

fn do_read(
    ctx: &mut Context<'_>,
    fd: i32,
    ptr: u64,
    count: usize,
    at: Option<u64>,
) -> Result<SyscallOutcome, DispatchError> {
    enum Src {
        Stdin { start: usize },
        File { path: String, start: u64 },
        Bad,
    }
    let src = match ctx.kernel.fds.get(fd).map(|d| &d.kind) {
        Some(FdKind::Stdin { cursor }) => Src::Stdin {
            start: at.unwrap_or(*cursor) as usize,
        },
        Some(FdKind::File { path, cursor, .. }) => Src::File {
            path: path.clone(),
            start: at.unwrap_or(*cursor),
        },
        Some(_) => Src::Bad,
        None => return e(linux::EBADF),
    };
    match src {
        Src::Stdin { start } => {
            if start >= ctx.kernel.stdin.len() {
                return ret(0);
            }
            let end = (start + count).min(ctx.kernel.stdin.len());
            let slice = ctx.kernel.stdin[start..end].to_vec();
            if ctx.mem.write(ptr, &slice).is_err() {
                return Ok(fault());
            }
            if at.is_none() {
                if let Some(Fd {
                    kind: FdKind::Stdin { cursor },
                    ..
                }) = ctx.kernel.fds.get_mut(fd)
                {
                    *cursor = end as u64;
                }
            }
            ret(slice.len() as i64)
        }
        Src::File { path, start } => {
            let file = match ctx.kernel.vfs.get(&path) {
                Some(f) => f,
                None => return e(linux::ENOENT),
            };
            let start_u = start as usize;
            if start_u >= file.data.len() {
                return ret(0);
            }
            let end = (start_u + count).min(file.data.len());
            let slice = file.data[start_u..end].to_vec();
            if ctx.mem.write(ptr, &slice).is_err() {
                return Ok(fault());
            }
            if at.is_none() {
                if let Some(Fd {
                    kind: FdKind::File { cursor, .. },
                    ..
                }) = ctx.kernel.fds.get_mut(fd)
                {
                    *cursor = end as u64;
                }
            }
            ret(slice.len() as i64)
        }
        Src::Bad => e(linux::EBADF),
    }
}

fn do_write(
    ctx: &mut Context<'_>,
    fd: i32,
    ptr: u64,
    count: usize,
    at: Option<u64>,
) -> Result<SyscallOutcome, DispatchError> {
    let data = match ctx.mem.read(ptr, count) {
        Ok(d) => d,
        Err(_) => return Ok(fault()),
    };
    enum Dst {
        Stdout,
        Stderr,
        File {
            path: String,
            cursor: u64,
            writable: bool,
            append: bool,
        },
        Bad,
    }
    let dst = match ctx.kernel.fds.get(fd).map(|d| &d.kind) {
        Some(FdKind::Stdout) => Dst::Stdout,
        Some(FdKind::Stderr) => Dst::Stderr,
        Some(FdKind::File {
            path,
            cursor,
            writable,
            append,
        }) => Dst::File {
            path: path.clone(),
            cursor: *cursor,
            writable: *writable,
            append: *append,
        },
        Some(_) => Dst::Bad,
        None => return e(linux::EBADF),
    };
    match dst {
        Dst::Stdout => {
            ctx.kernel.io.write(1, &data);
            ret(data.len() as i64)
        }
        Dst::Stderr => {
            ctx.kernel.io.write(2, &data);
            ret(data.len() as i64)
        }
        Dst::File {
            path,
            cursor,
            writable,
            append,
        } => {
            if !writable {
                return e(linux::EBADF);
            }
            let file = match ctx.kernel.vfs.get_mut(&path) {
                Some(f) => f,
                None => return e(linux::ENOENT),
            };
            let pos = if append {
                file.data.len()
            } else {
                at.unwrap_or(cursor) as usize
            };
            if pos + data.len() > file.data.len() {
                file.data.resize(pos + data.len(), 0);
            }
            file.data[pos..pos + data.len()].copy_from_slice(&data);
            file.dirty = true;
            let new_cur = (pos + data.len()) as u64;
            if at.is_none() {
                if let Some(Fd {
                    kind: FdKind::File { cursor, .. },
                    ..
                }) = ctx.kernel.fds.get_mut(fd)
                {
                    *cursor = new_cur;
                }
            }
            ret(data.len() as i64)
        }
        Dst::Bad => e(linux::EBADF),
    }
}

fn do_lseek(
    ctx: &mut Context<'_>,
    fd: i32,
    offset: i64,
    whence: i32,
) -> Result<SyscallOutcome, DispatchError> {
    enum Seek {
        Stdin { cur: u64 },
        File { path: String, cur: u64 },
        Pipe,
        Missing,
    }
    let seek = match ctx.kernel.fds.get(fd).map(|d| &d.kind) {
        Some(FdKind::Stdin { cursor }) => Seek::Stdin { cur: *cursor },
        Some(FdKind::File { path, cursor, .. }) => Seek::File {
            path: path.clone(),
            cur: *cursor,
        },
        Some(_) => Seek::Pipe,
        None => Seek::Missing,
    };
    match seek {
        Seek::Missing => e(linux::EBADF),
        Seek::Pipe => e(linux::ESPIPE),
        Seek::Stdin { cur } => {
            let len = ctx.kernel.stdin.len() as i64;
            match seek_from(cur as i64, len, offset, whence) {
                Ok(new) => {
                    if let Some(Fd {
                        kind: FdKind::Stdin { cursor },
                        ..
                    }) = ctx.kernel.fds.get_mut(fd)
                    {
                        *cursor = new as u64;
                    }
                    ret(new)
                }
                Err(n) => e(n),
            }
        }
        Seek::File { path, cur } => {
            let len = ctx
                .kernel
                .vfs
                .get(&path)
                .map(|f| f.data.len() as i64)
                .unwrap_or(0);
            match seek_from(cur as i64, len, offset, whence) {
                Ok(new) => {
                    if let Some(Fd {
                        kind: FdKind::File { cursor, .. },
                        ..
                    }) = ctx.kernel.fds.get_mut(fd)
                    {
                        *cursor = new as u64;
                    }
                    ret(new)
                }
                Err(n) => e(n),
            }
        }
    }
}

fn seek_from(cur: i64, len: i64, offset: i64, whence: i32) -> Result<i64, i64> {
    let base = match whence {
        SEEK_SET => 0,
        SEEK_CUR => cur,
        SEEK_END => len,
        _ => return Err(linux::EINVAL),
    };
    let new = base.saturating_add(offset);
    if new < 0 {
        return Err(linux::EINVAL);
    }
    Ok(new)
}

fn do_stat_path(
    ctx: &mut Context<'_>,
    path: &str,
    buf: u64,
) -> Result<SyscallOutcome, DispatchError> {
    let abs = match normalize(path, &ctx.kernel.cwd) {
        Ok(p) => p,
        Err(n) => return e(n),
    };
    do_stat_abs(ctx, &abs, buf)
}

fn do_stat_abs(ctx: &mut Context<'_>, abs: &str, buf: u64) -> Result<SyscallOutcome, DispatchError> {
    let is_dir = vfs::is_dir(&ctx.kernel.vfs, abs);
    let size = ctx
        .kernel
        .vfs
        .get(abs)
        .map(|f| f.data.len() as u64)
        .unwrap_or(0);
    if !is_dir && ctx.kernel.vfs.get(abs).is_none() {
        return e(linux::ENOENT);
    }
    let st = fill_stat(size, is_dir || abs == "/");
    match ctx.mem.write(buf, &st[..STAT_SIZE]) {
        Ok(()) => ret(0),
        Err(_) => Ok(fault()),
    }
}

fn do_fstat(ctx: &mut Context<'_>, fd: i32, buf: u64) -> Result<SyscallOutcome, DispatchError> {
    let kind = match ctx.kernel.fds.get(fd) {
        Some(d) => d.kind.clone(),
        None => return e(linux::EBADF),
    };
    let (size, is_dir) = match &kind {
        FdKind::Stdin { .. } => (ctx.kernel.stdin.len() as u64, false),
        FdKind::Stdout | FdKind::Stderr => (0, false),
        FdKind::Dir { .. } => (0, true),
        FdKind::File { path, .. } => (
            ctx.kernel
                .vfs
                .get(path)
                .map(|f| f.data.len() as u64)
                .unwrap_or(0),
            false,
        ),
    };
    let st = fill_stat(size, is_dir);
    match ctx.mem.write(buf, &st[..STAT_SIZE]) {
        Ok(()) => ret(0),
        Err(_) => Ok(fault()),
    }
}

fn do_mmap(
    ctx: &mut Context<'_>,
    addr: u64,
    length: u64,
    prot: u32,
    flags: u32,
    fd: i32,
    offset: i64,
) -> Result<SyscallOutcome, DispatchError> {
    if length == 0 {
        return e(linux::EINVAL);
    }
    let len = page_up(length);
    let anonymous = flags & MAP_ANONYMOUS != 0;
    let fixed = flags & MAP_FIXED != 0;
    let mut at = if addr == 0 { ctx.kernel.mmap_next } else { page_down(addr) };
    if !fixed && addr == 0 {
        at = ctx.kernel.mmap_next;
        ctx.kernel.mmap_next = at.saturating_add(len);
    }
    let prot = if prot == 0 { PROT_READ } else { prot };
    if ctx.mem.map_anon(at, len as usize, prot).is_err() {
        return e(linux::ENOMEM);
    }
    if !anonymous && fd >= 0 {
        if let Some(Fd {
            kind: FdKind::File { path, .. },
            ..
        }) = ctx.kernel.fds.get(fd)
        {
            let path = path.clone();
            if let Some(file) = ctx.kernel.vfs.get(&path) {
                let start = offset.max(0) as usize;
                if start < file.data.len() {
                    let n = (len as usize).min(file.data.len() - start);
                    let _ = ctx.mem.write(at, &file.data[start..start + n]);
                }
            }
        }
    }
    ret(at as i64)
}

fn do_brk(ctx: &mut Context<'_>, req: u64) -> Result<SyscallOutcome, DispatchError> {
    if ctx.kernel.brk_base == 0 {
        ctx.kernel.brk_base = 0x405000;
        ctx.kernel.brk = ctx.kernel.brk_base;
    }
    if req == 0 {
        return ret(ctx.kernel.brk as i64);
    }
    if req < ctx.kernel.brk_base {
        return ret(ctx.kernel.brk as i64);
    }
    let old_end = page_up(ctx.kernel.brk);
    let new_end = page_up(req);
    if new_end > old_end {
        let size = (new_end - old_end) as usize;
        if ctx
            .mem
            .map_anon(old_end, size, PROT_READ | 2)
            .is_err()
        {
            return ret(ctx.kernel.brk as i64);
        }
    }
    ctx.kernel.brk = req;
    ret(req as i64)
}

fn do_fcntl(
    ctx: &mut Context<'_>,
    fd: i32,
    cmd: i32,
    arg: u64,
) -> Result<SyscallOutcome, DispatchError> {
    match cmd {
        F_GETFD => match ctx.kernel.fds.get(fd) {
            Some(d) => ret(if d.cloexec { 1 } else { 0 }),
            None => e(linux::EBADF),
        },
        F_SETFD => match ctx.kernel.fds.get_mut(fd) {
            Some(d) => {
                d.cloexec = arg & 1 != 0;
                ret(0)
            }
            None => e(linux::EBADF),
        },
        F_GETFL => {
            if ctx.kernel.fds.is_open(fd) {
                ret(0)
            } else {
                e(linux::EBADF)
            }
        }
        F_SETFL => {
            if ctx.kernel.fds.is_open(fd) {
                ret(0)
            } else {
                e(linux::EBADF)
            }
        }
        F_DUPFD | F_DUPFD_CLOEXEC => match ctx.kernel.fds.dup(fd, arg as i32) {
            Some(n) => {
                if cmd == F_DUPFD_CLOEXEC {
                    if let Some(d) = ctx.kernel.fds.get_mut(n) {
                        d.cloexec = true;
                    }
                }
                ret(n as i64)
            }
            None => e(linux::EBADF),
        },
        _ => e(linux::EINVAL),
    }
}

fn do_readv(
    ctx: &mut Context<'_>,
    fd: i32,
    iov: u64,
    iovcnt: i64,
) -> Result<SyscallOutcome, DispatchError> {
    if iovcnt <= 0 {
        return e(linux::EINVAL);
    }
    let mut total = 0i64;
    for i in 0..iovcnt as u64 {
        let base = match ctx.mem.read(iov + i * 16, 8) {
            Ok(b) => u64::from_le_bytes(b.try_into().unwrap()),
            Err(_) => return Ok(fault()),
        };
        let len = match ctx.mem.read(iov + i * 16 + 8, 8) {
            Ok(b) => u64::from_le_bytes(b.try_into().unwrap()) as usize,
            Err(_) => return Ok(fault()),
        };
        match do_read(ctx, fd, base, len, None)? {
            SyscallOutcome::Return { ret } if ret >= 0 => {
                total += ret;
                if ret as usize != len {
                    break;
                }
            }
            other => return Ok(other),
        }
    }
    ret(total)
}

fn do_writev(
    ctx: &mut Context<'_>,
    fd: i32,
    iov: u64,
    iovcnt: i64,
) -> Result<SyscallOutcome, DispatchError> {
    if iovcnt <= 0 {
        return e(linux::EINVAL);
    }
    let mut total = 0i64;
    for i in 0..iovcnt as u64 {
        let base = match ctx.mem.read(iov + i * 16, 8) {
            Ok(b) => u64::from_le_bytes(b.try_into().unwrap()),
            Err(_) => return Ok(fault()),
        };
        let len = match ctx.mem.read(iov + i * 16 + 8, 8) {
            Ok(b) => u64::from_le_bytes(b.try_into().unwrap()) as usize,
            Err(_) => return Ok(fault()),
        };
        match do_write(ctx, fd, base, len, None)? {
            SyscallOutcome::Return { ret } if ret >= 0 => total += ret,
            other => return Ok(other),
        }
    }
    ret(total)
}

/// Register every implemented handler.
pub fn register_all(dispatch: &mut crate::syscall::Dispatch) {
    let handlers: Vec<Arc<dyn SyscallHandler>> = vec![
        Arc::new(ReadHandler),
        Arc::new(WriteHandler),
        Arc::new(OpenHandler),
        Arc::new(CloseHandler),
        Arc::new(StatHandler),
        Arc::new(FstatHandler),
        Arc::new(LstatHandler),
        Arc::new(PollHandler),
        Arc::new(LseekHandler),
        Arc::new(MmapHandler),
        Arc::new(MprotectHandler),
        Arc::new(MunmapHandler),
        Arc::new(BrkHandler),
        Arc::new(RtSigactionHandler),
        Arc::new(RtSigprocmaskHandler),
        Arc::new(IoctlHandler),
        Arc::new(PreadHandler),
        Arc::new(PwriteHandler),
        Arc::new(ReadvHandler),
        Arc::new(WritevHandler),
        Arc::new(AccessHandler),
        Arc::new(DupHandler),
        Arc::new(Dup2Handler),
        Arc::new(GetpidHandler),
        Arc::new(ExitHandler),
        Arc::new(UnameHandler),
        Arc::new(FcntlHandler),
        Arc::new(GetcwdHandler),
        Arc::new(ChdirHandler),
        Arc::new(ReadlinkHandler),
        Arc::new(GettimeofdayHandler),
        Arc::new(GetuidHandler),
        Arc::new(GetgidHandler),
        Arc::new(GeteuidHandler),
        Arc::new(GetegidHandler),
        Arc::new(GetppidHandler),
        Arc::new(ArchPrctlHandler),
        Arc::new(GettidHandler),
        Arc::new(TimeHandler),
        Arc::new(FutexHandler),
        Arc::new(SetTidAddressHandler),
        Arc::new(ClockGettimeHandler),
        Arc::new(ExitGroupHandler),
        Arc::new(OpenatHandler),
        Arc::new(NewfstatatHandler),
        Arc::new(SetRobustListHandler),
        Arc::new(PrlimitHandler),
        Arc::new(GetrandomHandler),
        Arc::new(RseqHandler),
    ];
    for h in handlers {
        dispatch.register(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::Dispatch;

    #[test]
    fn write_captures_output() {
        let mut ctx = Context::new();
        let addr = 0x400000u64;
        ctx.mem.map_anon(addr, 16, 3).unwrap();
        ctx.mem.write(addr, b"hello").unwrap();

        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(WriteHandler));
        let regs = SyscallRegs::from_regs(1, [1, addr, 5, 0, 0, 0], 0x400100);
        match dispatch.dispatch(&mut ctx, regs).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, 5),
            other => panic!("unexpected: {other:?}"),
        }
        assert_eq!(ctx.kernel.io.stdout(), b"hello");
    }

    #[test]
    fn write_rejects_bad_fd() {
        let mut ctx = Context::new();
        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(WriteHandler));
        let regs = SyscallRegs::from_regs(1, [42, 0, 0, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, regs).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, err(linux::EBADF)),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn exit_sets_state() {
        let mut ctx = Context::new();
        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(ExitHandler));
        let regs = SyscallRegs::from_regs(60, [7, 0, 0, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, regs).unwrap() {
            SyscallOutcome::Exit(ExitReason::Exit { code }) => assert_eq!(code, 7),
            other => panic!("unexpected: {other:?}"),
        }
        assert!(matches!(
            ctx.kernel.process,
            ProcessState::Exited { code: 7 }
        ));
    }

    #[test]
    fn register_all_installs_stage2() {
        let mut dispatch = Dispatch::new();
        register_all(&mut dispatch);
        let impld = dispatch.implemented();
        assert!(impld.contains(&0));
        assert!(impld.contains(&1));
        assert!(impld.contains(&2));
        assert!(impld.contains(&9));
        assert!(impld.contains(&12));
        assert!(impld.contains(&60));
        assert!(impld.contains(&231));
        assert!(impld.contains(&257));
    }

    #[test]
    fn read_stdin() {
        let mut ctx = Context::new();
        ctx.kernel.stdin = b"abc".to_vec();
        let addr = 0x400000u64;
        ctx.mem.map_anon(addr, 16, 3).unwrap();
        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(ReadHandler));
        let regs = SyscallRegs::from_regs(0, [0, addr, 8, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, regs).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, 3),
            other => panic!("{other:?}"),
        }
        assert_eq!(ctx.mem.read(addr, 3).unwrap(), b"abc");
    }

    #[test]
    fn open_create_read_write() {
        let mut ctx = Context::new();
        let path_addr = 0x400000u64;
        let buf_addr = 0x401000u64;
        ctx.mem.map_anon(path_addr, 0x2000, 3).unwrap();
        ctx.mem.write(path_addr, b"/tmp/x\0").unwrap();
        ctx.mem.write(buf_addr, b"hi").unwrap();
        let mut dispatch = Dispatch::new();
        register_all(&mut dispatch);
        let open = SyscallRegs::from_regs(2, [path_addr, (O_RDWR | O_CREAT) as u64, 0, 0, 0, 0], 0);
        let fd = match dispatch.dispatch(&mut ctx, open).unwrap() {
            SyscallOutcome::Return { ret } => ret,
            other => panic!("{other:?}"),
        };
        assert!(fd >= 3);
        let w = SyscallRegs::from_regs(1, [fd as u64, buf_addr, 2, 0, 0, 0], 0);
        assert!(matches!(
            dispatch.dispatch(&mut ctx, w).unwrap(),
            SyscallOutcome::Return { ret: 2 }
        ));
        let seek = SyscallRegs::from_regs(8, [fd as u64, 0, 0, 0, 0, 0], 0);
        dispatch.dispatch(&mut ctx, seek).unwrap();
        let r = SyscallRegs::from_regs(0, [fd as u64, buf_addr + 8, 2, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, r).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, 2),
            other => panic!("{other:?}"),
        }
        assert_eq!(ctx.mem.read(buf_addr + 8, 2).unwrap(), b"hi");
    }

    #[test]
    fn brk_grows() {
        let mut ctx = Context::new();
        ctx.kernel.brk_base = 0x500000;
        ctx.kernel.brk = 0x500000;
        ctx.mem.map_anon(0x500000, 0x1000, 3).unwrap();
        let mut dispatch = Dispatch::new();
        dispatch.register(Arc::new(BrkHandler));
        let q = SyscallRegs::from_regs(12, [0, 0, 0, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, q).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, 0x500000),
            other => panic!("{other:?}"),
        }
        let g = SyscallRegs::from_regs(12, [0x501000, 0, 0, 0, 0, 0], 0);
        match dispatch.dispatch(&mut ctx, g).unwrap() {
            SyscallOutcome::Return { ret } => assert_eq!(ret, 0x501000),
            other => panic!("{other:?}"),
        }
    }
}
