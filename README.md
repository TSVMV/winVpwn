# winVpwn

Run Linux ELF binaries on Windows without WSL, a VM, or Docker.

winVpwn is a compatibility layer that makes the core of pwntools usable natively on
Windows. It loads Linux ELF binaries into an in-process CPU emulator (Unicorn
Engine), translates Linux system calls to Windows APIs, and exposes a virtual
kernel with virtual processes, file descriptors, and a virtual filesystem.

> **Scope notice**: winVpwn is designed for authorized environments only: CTF
> challenges, education, and software you own. It never touches your host
> filesystem or network unless explicitly mapped, and every simulated process
> runs below the privilege of the host process.

## Quick start

```console
$ pip install winvpwn
$ winvpwn doctor
$ winvpwn run hello_static
hello, winVpwn
```

## Status

Stage 1 — infrastructure and the minimal executable path (in development):

- ELF loader for static `ET_EXEC` x86_64 binaries
- Unicorn execution loop with `syscall` interception
- `write`, `exit`, `exit_group` syscalls
- `winvpwn run` and `winvpwn elf`

## Documentation

- [Architecture](docs/architecture.md)
- [Compatibility](docs/compatibility.md)
- [Security model](docs/security.md)
- [Contributing](docs/contributing.md)

## Development

```console
$ cargo build
$ cargo test --no-default-features
$ maturin develop
$ pytest
$ ruff check .
$ mypy src
```
