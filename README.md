# winVpwn

Run Linux ELF binaries on Windows without WSL, a VM, or Docker.

winVpwn is a compatibility layer that makes the core of pwntools usable natively on
Windows. It loads Linux ELF binaries into an in-process CPU emulator (Unicorn
Engine), translates Linux system calls to a virtual kernel with virtual
processes, file descriptors, and a virtual filesystem.

> **Scope notice**: winVpwn is designed for authorized environments only: CTF
> challenges, education, and software you own. The guest never touches the host
> filesystem or network unless you pass an explicit `--map`. Every simulated
> process runs below the privilege of the host process.

## Quick start

```console
$ pip install winvpwn
$ winvpwn doctor
$ winvpwn run hello_static
hello, winVpwn
```

Feed stdin and map a host file into the guest:

```console
$ winvpwn run pwn --stdin input.bin --map /flag=C:\ctf\flag.txt
$ winvpwn run pwn --map /tmp/out=.\out.bin:rw --arg ./pwn --arg hello
```

## Status

0.2.0 — virtual kernel for static `ET_EXEC` x86_64:

- ELF loader for static `ET_EXEC` x86_64 binaries
- Unicorn execution loop with `syscall` interception
- Virtual FD table, in-memory VFS, explicit host maps
- `read` / `write` / `open` / `close` / `lseek` / `stat` family
- `brk` / `mmap` / `mprotect` / `munmap`
- libc bootstrap (`arch_prctl`, `uname`, `getpid`, `clock_gettime`, …)
- `winvpwn run`, `elf`, `asm`, `disasm`, `doctor`

Not yet: PIE, dynamic linking, 32-bit, threads, networking.

## Documentation

- [Architecture](docs/architecture.md)
- [Compatibility](docs/compatibility.md)
- [Security model](docs/security.md)
- [Contributing](docs/contributing.md)

## Development

```console
$ cargo test --no-default-features
$ maturin develop
$ pytest
$ ruff check src tests
$ mypy src tests
```
