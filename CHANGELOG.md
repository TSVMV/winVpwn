# Changelog

## [0.1.0] - Unreleased

Stage 1: infrastructure and the minimal executable path.

### Added
- Rust workspace with `winvpwn-core` crate (PyO3, Unicorn Engine, goblin).
- ELF loader supporting static `ET_EXEC` x86_64 images with PT_LOAD mapping
  and overlap validation.
- Unicorn-based execution loop with `syscall`/`int 0x80` interception and
  register context handling.
- Syscall dispatch for `write`, `exit`, `exit_group` with errno semantics.
- Python package exposing `run_elf` / `parse_elf` through PyO3.
- CLI commands: `doctor`, `run`, `elf`, `asm`, `disasm`, `version`
  (rich-based UI, `--json`, `--no-color`, `--quiet`).
- Test suite: Rust unit tests, Python unit tests, CLI integration tests.
- CI matrix (Windows + Linux, Python 3.10-3.12).
