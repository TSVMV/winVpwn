# Compatibility

## Supported images (0.2)

- Static `ET_EXEC` x86_64 (little-endian) ELF images.
- `PT_LOAD` segments covering `.text` / `.data` / `.bss`.
- Programs that use stdin, mapped files, `brk`, and anonymous `mmap`.
- Static musl/glibc bootstrap syscalls (`arch_prctl`, `uname`, `set_tid_address`, …).

## Not supported yet

- PIE (`ET_DYN`) and shared libraries.
- 32-bit (`EM_386`) images.
- Dynamic linking, threads, signals as real delivery.
- Guest networking.
- Runtime `mmap` of host files except through an already-open VFS fd.

## Execution model

- The guest stack is placed at `0x7ffffffde000` with a Linux argv/envp/auxv
  layout (`AT_PAGESZ`, `AT_ENTRY`, `AT_UID`/`AT_EUID`/`AT_GID`/`AT_EGID`,
  `AT_RANDOM`). Default argv is the ELF path.
- Fd 0 reads the `--stdin` buffer. Fd 1/2 are captured in-memory.
- Host files appear in the VFS only when passed as `--map guest=host` or
  `--map guest=host:rw`. Unmapped paths return `-ENOENT`.
- Guest `O_CREAT` creates an in-memory node that is never flushed to the host.
- Emulation stops when the process calls `exit`/`exit_group`, when the guest
  fetches from an unmapped page, or after the timeout.

## Host support

The core engine and CLI are host-agnostic (the emulator runs on Windows too).
`--map` paths are host-native; guest paths are always Linux-style (`/flag`).
