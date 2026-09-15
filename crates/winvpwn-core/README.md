# winvpwn-core

Core engine of winvpwn: an ELF loader, a Unicorn-based x86-64 execution loop,
and a syscall dispatcher, exposed to Python through PyO3.

Stage 1 scope: static `ET_EXEC` x86_64 images; `write`, `exit`, `exit_group`.

```rust
use winvpwn_core::elf::load_elf;
use winvpwn_core::cpu::unicorn_engine::{Vcpu, STACK_BASE, STACK_SIZE};
use winvpwn_core::syscall::dispatch::Dispatch;
use winvpwn_core::syscall::dispatch_impl;
use std::sync::Arc;

let image = std::fs::read("hello_static")?;
let elf = load_elf(&image)?;

let mut dispatch = Dispatch::new();
dispatch_impl::register_all(&mut dispatch);
let mut vcpu = Vcpu::new(Arc::new(dispatch))?;
vcpu.map_elf(&elf)?;
vcpu.map_stack(STACK_BASE, STACK_SIZE)?;
vcpu.setup_initial_state(elf.entry, STACK_BASE + STACK_SIZE)?;

let reason = vcpu.run(0)?;
assert_eq!(vcpu.output(), b"hello, winVpwn\n");
```

Build without Python bindings:

```console
$ cargo test --no-default-features
```
