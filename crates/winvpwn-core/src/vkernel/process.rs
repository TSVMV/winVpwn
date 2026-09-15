//! Process lifecycle state.

/// Lifecycle state of a simulated process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessState {
    /// The process is executing (or ready to execute).
    Running,
    /// The process has terminated via `exit`/`exit_group`.
    Exited { code: u8 },
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessState::Running => write!(f, "running"),
            ProcessState::Exited { code } => write!(f, "exited(code={})", code),
        }
    }
}
