# winVpwn

Run Linux ELF binaries on Windows without WSL, a VM, or Docker.

winVpwn is a compatibility layer that loads Linux ELF binaries into an in-process
CPU emulator (Unicorn Engine) and translates Linux system calls into a virtual
kernel with virtual processes, file descriptors, and a virtual filesystem.

## Status

0.2.0 — virtual kernel for static `ET_EXEC` x86_64.

- ELF loader for static `ET_EXEC` x86_64 binaries
- Unicorn execution loop with `syscall` interception
- Virtual FD table, in-memory VFS, explicit host maps
- I/O, brk/mmap, and libc bootstrap syscalls
- `winvpwn run` / `elf` / `asm` / `disasm` / `doctor`

## Quick start

```console
$ pip install winvpwn
$ winvpwn doctor
$ winvpwn run hello_static
hello, winVpwn
$ winvpwn run pwn --stdin input.bin --map /flag=./flag.txt
```

## Documentation

- [Architecture](architecture.md)
- [Compatibility](compatibility.md)
- [Security model](security.md)
- [Contributing](contributing.md)

## Development

```console
$ cargo test --no-default-features
$ maturin develop
$ pytest
$ ruff check src tests
$ mypy src tests
```
