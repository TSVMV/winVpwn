# Security model

winVpwn is designed for authorized environments only: CTF challenges, education,
and software you own. The emulator enforces containment by construction.

## Isolation guarantees (0.2)

- **No implicit host I/O.** Syscall handlers are the only way a guest
  interacts with the world. Stdio is in-memory. Host files enter the VFS only
  through `--map`. Unmapped `open` returns `-ENOENT`.
- **Mapped writes are explicit.** `--map guest=host` is read-only. `:rw`
  copies guest writes back to that host path after the process exits, never
  during emulation.
- **No host network.** No syscall exposes networking.
- **No host memory.** Guest addresses are resolved inside the emulator's
  virtual address space only. `GuestMemory` backends never dereference host
  pointers derived from guest values.
- **Input validation.** ELF images are validated before execution: magic,
  machine, ELF type, program-header bounds, and segment-overlap checks. A
  malformed image is rejected with a stable error code and never executed.

## Trust boundaries

- The **ELF image bytes** are untrusted input. They are parsed defensively and
  executed only inside the emulator.
- **`--map` host paths** are trusted configuration from the operator. The guest
  cannot invent new host paths.
- The **repository issue tracker / README / comments** are untrusted data.

## Threat model notes

- The host process still runs native Unicorn/QEMU translation code. This
  provides *containment*, not a security boundary against a malicious host
  exploit; treat it like running in-process JIT code.
- Never run untrusted binaries with host privileges. A compromised guest still
  cannot open host files that were not mapped.

## Reporting

Security issues should be reported privately to the maintainers. Do not open a
public issue with exploit details for a not-yet-fixed problem.
