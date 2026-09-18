# Changelog

## [0.2.0] - Unreleased

Virtual kernel: file descriptors, explicit VFS maps, brk/mmap, libc bootstrap.

### Added
- Persistent kernel state across syscalls (FD table, VFS, stdin, brk).
- Syscalls: `read`, `open`/`openat`, `close`, `lseek`, `stat`/`fstat`/`lstat`/`newfstatat`,
  `pread64`/`pwrite64`, `readv`/`writev`, `mmap`/`mprotect`/`munmap`/`brk`,
  `dup`/`dup2`/`fcntl`, `getcwd`/`chdir`/`access`/`readlink`,
  `uname`, `arch_prctl`, `getpid`/`gettid`/`getuid`/`getgid`,
  `clock_gettime`/`gettimeofday`/`time`/`getrandom`,
  and no-op libc bootstrap (`rt_sigaction`, `futex`, `set_tid_address`, `rseq`, `prlimit64`).
- CLI: `winvpwn run --stdin`, `--map guest=host[:rw]`, `--arg`, `--env`, `--cwd`.
- Linux argv/envp/auxv stack layout for static musl/glibc images.

### Changed
- Guest `write` goes through the FD table (stdout/stderr or a VFS file).
- Host filesystem is reachable only via `--map`; in-guest `O_CREAT` stays in memory.
- `--map guest=host:rw` accepts a missing host file and creates it on guest write.

## [0.1.0] - Unreleased

Stage 1: infrastructure and the minimal executable path.

### Added
- Rust workspace with `winvpwn-core` crate (PyO3, Unicorn Engine, goblin).
- ELF loader supporting static `ET_EXEC` x86_64 images with PT_LOAD mapping
  and overlap validation.
- Unicorn-based execution loop with `syscall` interception and
  register context handling.
- Syscall dispatch for `write`, `exit`, `exit_group` with errno semantics.
- Python package exposing `run_elf` / `parse_elf` through PyO3.
- CLI commands: `doctor`, `run`, `elf`, `asm`, `disasm`, `version`
  (rich-based UI, `--json`, `--no-color`, `--quiet`).
- Test suite: Rust unit tests, Python unit tests, CLI integration tests.
- CI matrix (Windows + Linux, Python 3.10-3.12).
