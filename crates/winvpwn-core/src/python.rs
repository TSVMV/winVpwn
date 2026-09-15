//! PyO3 bindings exposing the engine to Python.
//!
//! Exposes `run_elf` (load, map and execute an ELF image) and `parse_elf`
//! (inspect a loadable image without executing it).

use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

use crate::cpu::unicorn_engine::{Vcpu, STACK_BASE, STACK_SIZE};
use crate::elf::load_elf;
use crate::syscall::dispatch::Dispatch;
use crate::syscall::dispatch_impl;
use crate::trace::recorder::TraceSink;
use crate::vkernel::ExitReason;

/// Load, map and execute an ELF image.
///
/// Returns a dict with keys: `exit` (str), `code` (int, exit-only), `rip`
/// (int, falloff-only), `output` (bytes), `trace` (list of dicts).
#[pyfunction]
#[pyo3(signature = (image, timeout_ms=0))]
fn run_elf<'py>(py: Python<'py>, image: &[u8], timeout_ms: u64) -> PyResult<Bound<'py, PyDict>> {
    let elf = load_elf(image).map_err(|e| PyValueError::new_err(e.to_string()))?;

    let trace = TraceSink::new();
    let mut dispatch = Dispatch::new();
    dispatch_impl::register_all(&mut dispatch);
    dispatch.set_trace(trace.clone());

    let mut vcpu = Vcpu::new(Arc::new(dispatch)).map_err(to_err)?;
    vcpu.map_elf(&elf).map_err(to_err)?;
    vcpu.map_stack(STACK_BASE, STACK_SIZE).map_err(to_err)?;
    vcpu.setup_initial_state(elf.entry, STACK_BASE + STACK_SIZE)
        .map_err(to_err)?;

    let reason = vcpu.run(timeout_ms).map_err(to_err)?;
    let output = vcpu.output();

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
