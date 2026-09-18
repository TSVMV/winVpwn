# winvpwn-core

Core engine of winvpwn: an ELF loader, a Unicorn-based x86-64 execution loop,
and a virtual kernel, exposed to Python through PyO3.

0.2 scope: static `ET_EXEC` x86_64 images; virtual FDs; explicit VFS maps;
`brk`/`mmap`; libc bootstrap syscalls.

```rust
use winvpwn_core::elf::load_elf;
use winvpwn_core::cpu::unicorn_engine::{Vcpu, STACK_BASE, STACK_SIZE};
use winvpwn_core::syscall::dispatch::Dispatch;
use winvpwn_core::syscall::dispatch_impl;
use winvpwn_core::vkernel::kernel::KernelState;
use std::sync::Arc;

let image = std::fs::read("hello_static")?;
let elf = load_elf(&image)?;

let mut kernel = KernelState::new();
kernel.stdin = b"hello\n".to_vec();

let mut dispatch = Dispatch::new();
dispatch_impl::register_all(&mut dispatch);
let mut vcpu = Vcpu::new_with_kernel(Arc::new(dispatch), kernel)?;
vcpu.map_elf(&elf)?;
vcpu.map_stack(STACK_BASE, STACK_SIZE)?;
vcpu.setup_linux_stack(elf.entry, STACK_BASE + STACK_SIZE, &["/guest".into()], &[])?;

let reason = vcpu.run(0)?;
let _ = reason;
let _ = vcpu.output();
```

Build without Python bindings:

```console
$ cargo test --no-default-features
```
