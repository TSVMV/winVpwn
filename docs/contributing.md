# Contributing

## Development setup

Requirements: Python >= 3.10, Rust stable (2021 edition), clang/libclang
(for the Unicorn bindgen step).

```console
$ cargo test --no-default-features   # core logic without Python bindings
$ maturin develop                    # build + install winvpwn_core
$ pip install -e . pytest hypothesis ruff mypy
$ pytest                             # Python + CLI tests
```

## Quality gates

All checks must pass before a merge:

```console
$ cargo fmt --all -- --check
$ cargo clippy --all-targets --no-default-features -- -D warnings
$ ruff check src tests
$ mypy src tests
$ pytest
```

Linux CI additionally enforces `--cov-fail-under=80` for the Python package.

## Code conventions

- Rust: `cargo fmt` style; `#![deny(warnings)]`-equivalent clippy hygiene.
- Python: ruff defaults (E, F, W, I, UP, B, SIM), 100-column lines, mypy
  `--strict`.
- No `eval`/`exec`/`pickle` on untrusted input.
- No hard-coded absolute paths.
- Every error carries a stable `E0xx` machine-readable code.

## Testing notes

- Rust unit tests use `crates/winvpwn-core/tests/fixtures/hello_static`, a
  statically linked `ET_EXEC` x86_64 binary.
- Python tests reuse that fixture through a symlink under `tests/fixtures`.
- Keep the emulator tests fast; the fixture binary is ~0.7 MB and runs in
  milliseconds.

## Branching

Feature work goes through `ai/*` branches and pull requests. Commits and PR
descriptions are plain technical text.
