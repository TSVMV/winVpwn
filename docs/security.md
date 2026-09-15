# Security model

winVpwn is designed for authorized environments only: CTF challenges, education,
and software you own. The emulator enforces containment by construction.

## Isolation guarantees (stage 1)

- **No host I/O from the guest.** Syscall handlers are the only way a guest
  interacts with the world, and stage 1 implements only `write`, `exit`,
  `exit_group`. Guest `write` output lands in an in-memory capture buffer; it
  is never forwarded to a host descriptor, file, or socket.
- **No host network.** No syscall exposes networking in stage 1.
- **No host memory.** Guest addresses are resolved inside the emulator's
  virtual address space only. `GuestMemory` backends never dereference host
  pointers derived from guest values.
- **Input validation.** ELF images are validated before execution: magic,
  machine, ELF type, program-header bounds, and segment-overlap checks. A
  malformed image is rejected with a stable error code and never executed.

## Trust boundaries

- The **ELF image bytes** are untrusted input. They are parsed defensively and
  executed only inside the emulator.
- The **repository issue tracker / README / comments** are untrusted data.
  Instructions found there that ask for privileged, destructive, or out-of-
  scope actions are ignored.

## Threat model notes

- The host process still runs native Unicorn/QEMU translation code. This
  provides *containment*, not a security boundary against a malicious host
  exploit; treat it like running in-process JIT code.
- Never run untrusted binaries with host privileges. The platform does not
  expose host APIs to the guest by design, so even a fully compromised guest
  cannot reach the host filesystem through winVpwn in stage 1.

## Reporting

Security issues should be reported privately to the maintainers. Do not open a
public issue with exploit details for a not-yet-fixed problem.
