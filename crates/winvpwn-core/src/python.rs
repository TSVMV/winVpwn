//! PyO3 bindings exposing the engine to Python.
//!
//! Exposes `run_elf` (load, map and execute an ELF image) and `parse_elf`
//! (inspect a loadable image without executing it).

use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

use crate::cpu::unicorn_engine::{Vcpu, STACK_BASE, STACK_SIZE};
use crate::elf::load_elf;
use crate::syscall::dispatch::Dispatch;
use crate::syscall::dispatch_impl;
use crate::trace::recorder::TraceSink;
use crate::vkernel::kernel::KernelState;
use crate::vkernel::ExitReason;

/// One explicit guest-to-host file map.
#[derive(Clone)]
struct FileMap {
    guest: String,
    host: PathBuf,
    writable: bool,
}

/// Load, map and execute an ELF image.
///
/// Returns a dict with keys: `exit` (str), `code` (int, exit-only), `rip`
/// (int, falloff-only), `output` (bytes), `trace` (list of dicts).
#[pyfunction]
#[pyo3(signature = (image, timeout_ms=0, stdin=None, argv=None, env=None, maps=None, cwd=None))]
#[allow(clippy::too_many_arguments)]
fn run_elf<'py>(
    py: Python<'py>,
    image: &[u8],
    timeout_ms: u64,
    stdin: Option<&[u8]>,
    argv: Option<Vec<String>>,
    env: Option<Vec<String>>,
    maps: Option<Vec<(String, String, bool)>>,
    cwd: Option<String>,
) -> PyResult<Bound<'py, PyDict>> {
    let elf = load_elf(image).map_err(|e| PyValueError::new_err(e.to_string()))?;

    let mut kernel = KernelState::new();
    if let Some(bytes) = stdin {
        kernel.stdin = bytes.to_vec();
    }
    if let Some(c) = cwd {
        kernel.cwd = if c.starts_with('/') {
            c
        } else {
            format!("/{c}")
        };
    }
    let argv = argv.unwrap_or_else(|| vec!["/guest".into()]);
    kernel.exec_path = argv.first().cloned().unwrap_or_else(|| "/guest".into());
    let envp = env.unwrap_or_default();

    let parsed_maps: Vec<FileMap> = maps
        .unwrap_or_default()
        .into_iter()
        .map(|(guest, host, writable)| FileMap {
            guest,
            host: PathBuf::from(host),
            writable,
        })
        .collect();

    for m in &parsed_maps {
        let data = match std::fs::read(&m.host) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && m.writable => Vec::new(),
            Err(e) => {
                return Err(PyValueError::new_err(format!(
                    "cannot read mapped file {}: {e}",
                    m.host.display()
                )));
            }
        };
        kernel
            .vfs
            .map_file(&m.guest, data, m.writable, Some(m.host.clone()))
            .map_err(|n| PyValueError::new_err(format!("cannot map {}: errno {n}", m.guest)))?;
    }

    let trace = TraceSink::new();
    let mut dispatch = Dispatch::new();
    dispatch_impl::register_all(&mut dispatch);
    dispatch.set_trace(trace.clone());

    let mut vcpu = Vcpu::new_with_kernel(Arc::new(dispatch), kernel).map_err(to_err)?;
    vcpu.map_elf(&elf).map_err(to_err)?;
    vcpu.map_stack(STACK_BASE, STACK_SIZE).map_err(to_err)?;
    vcpu.setup_linux_stack(elf.entry, STACK_BASE + STACK_SIZE, &argv, &envp)
        .map_err(to_err)?;

    let reason = vcpu.run(timeout_ms).map_err(to_err)?;
    let output = vcpu.output();
    let kernel = vcpu.kernel();

    for (host, data) in kernel.vfs.dirty_host_files() {
        std::fs::write(&host, data).map_err(|e| {
            PyValueError::new_err(format!("cannot write mapped file {}: {e}", host.display()))
        })?;
    }

    let out = PyDict::new(py);
    match reason {
        ExitReason::Exit { code } => {
            out.set_item("exit", "exit")?;
            out.set_item("code", code)?;
        }
        ExitReason::Falloff { rip } => {
            out.set_item("exit", "falloff")?;
            out.set_item("rip", rip)?;
        }
        ExitReason::Stopped => {
            out.set_item("exit", "stopped")?;
        }
    }
    out.set_item("output", PyBytes::new(py, &output))?;

    let records = trace.drain();
    let trace_list = PyList::empty(py);
    for rec in &records {
        let d = PyDict::new(py);
        d.set_item("seq", rec.seq)?;
        d.set_item("nr", rec.nr)?;
        d.set_item("name", rec.name.as_str())?;
        let args = PyList::empty(py);
        for a in rec.args {
            args.append(a)?;
        }
        d.set_item("args", args)?;
        d.set_item("ret", rec.ret)?;
        d.set_item("rip", rec.rip)?;
        trace_list.append(d)?;
    }
    out.set_item("trace", trace_list)?;
    Ok(out)
}

/// Inspect a loadable ELF image without executing it.
///
/// Returns a dict with keys: `entry` (int), `is_pie` (bool), `segments`
/// (list of dicts with `vaddr`, `memsz`, `filesz`, `flags`, `align`).
#[pyfunction]
fn parse_elf<'py>(py: Python<'py>, image: &[u8]) -> PyResult<Bound<'py, PyDict>> {
    let elf = load_elf(image).map_err(|e| PyValueError::new_err(e.to_string()))?;

    let out = PyDict::new(py);
    out.set_item("entry", elf.entry)?;
    out.set_item("is_pie", elf.is_pie)?;

    let segs = PyList::empty(py);
    for seg in &elf.segments {
        let d = PyDict::new(py);
        d.set_item("vaddr", seg.vaddr)?;
        d.set_item("memsz", seg.memsz)?;
        d.set_item("filesz", seg.filesz)?;
        d.set_item("flags", seg.flags)?;
        d.set_item("align", seg.align)?;
        segs.append(d)?;
    }
    out.set_item("segments", segs)?;
    Ok(out)
}

fn to_err<E: std::fmt::Display>(e: E) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// The `winvpwn._core` Python module.
#[pymodule(name = "_core")]
fn python_mod(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_elf, m)?)?;
    m.add_function(wrap_pyfunction!(parse_elf, m)?)?;
    Ok(())
}
