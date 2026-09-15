# Compatibility

## Supported images (stage 1)

- Static `ET_EXEC` x86_64 (little-endian) ELF images.
- `PT_LOAD` segments with the full `.text`/`.data`/`.bss` range required by a
  hello-world static binary.

## Not supported yet

- PIE (`ET_DYN`) and shared libraries.
- 32-bit (`EM_386`) images.
- Dynamic linking, threads, signals.
- Any syscall beyond `write`, `exit`, `exit_group` (returns `-ENOSYS`).
- mmap/mprotect requested by the guest at runtime.

## Execution model

- The guest stack is placed at `0x7ffffffde000` with a minimal
  `argc=0, argv=[NULL], envp=[NULL]` layout (no arguments or environment in
  stage 1).
- Output written to fds 1/2 is captured in-memory; nothing touches the host
  filesystem or network.
- Emulation stops when the process calls `exit`/`exit_group`, when the guest
  fetches from an unmapped page, or after the timeout.

## Host support

The core engine and CLI are host-agnostic (the emulator runs on Windows too).
Windows toolchain support for the `winvpwn asm`/`disasm` helpers ships in a
later stage.
