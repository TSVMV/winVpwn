//! Collecting a per-run syscall trace.

use std::sync::{Arc, Mutex};

use crate::syscall::dispatch::SyscallOutcome;

/// One recorded syscall invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRecord {
    /// Monotonic invocation index.
    pub seq: u64,
    /// Linux x86_64 syscall number.
    pub nr: i64,
    /// Handler name, or `"<unimplemented>"`.
    pub name: String,
    /// The six ABI argument registers.
    pub args: [u64; 6],
    /// The guest-visible return value (possibly negative errno).
    pub ret: i64,
    /// Guest instruction pointer at invocation time.
    pub rip: u64,
}

/// A handle that syscalls are recorded into. Cloning shares the same buffer.
#[derive(Debug, Clone, Default)]
pub struct TraceSink(Arc<Mutex<Vec<TraceRecord>>>);

impl TraceSink {
    /// Create a fresh sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record.
    pub fn record(&self, name: &str, nr: i64, args: [u64; 6], rip: u64, outcome: &SyscallOutcome) {
        let ret = match outcome {
            SyscallOutcome::Return { ret } => *ret,
            SyscallOutcome::Exit(_) => 0,
        };
        let mut buf = self.0.lock().expect("trace mutex poisoned");
        let seq = buf.len() as u64;
        buf.push(TraceRecord {
            seq,
            nr,
            name: name.to_string(),
            args,
            ret,
            rip,
        });
    }

    /// Take all records collected so far, leaving the sink empty.
    pub fn drain(&self) -> Vec<TraceRecord> {
        let mut buf = self.0.lock().expect("trace mutex poisoned");
        std::mem::take(&mut *buf)
    }

    /// Number of records collected so far.
    pub fn len(&self) -> usize {
        self.0.lock().expect("trace mutex poisoned").len()
    }

    /// True when no records have been collected.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::dispatch::SyscallOutcome;

    #[test]
    fn drain_roundtrip() {
        let sink = TraceSink::new();
        sink.record(
            "write",
            1,
            [1, 0, 5, 0, 0, 0],
            0x401000,
            &SyscallOutcome::Return { ret: 5 },
        );
        let recs = sink.drain();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].name, "write");
        assert_eq!(recs[0].nr, 1);
        assert_eq!(recs[0].ret, 5);
        assert!(sink.is_empty());
    }

    #[test]
    fn clones_share_buffer() {
        let a = TraceSink::new();
        let b = a.clone();
        a.record(
            "exit",
            60,
            [0, 0, 0, 0, 0, 0],
            0,
            &SyscallOutcome::Exit(crate::vkernel::ExitReason::Exit { code: 0 }),
        );
        assert_eq!(b.drain().len(), 1);
    }
}
