//! Explicit serialization of compound application work across background services.

use std::sync::Mutex;

/// Application-owned gate shared by commands, snapshots, and deadline mutations.
///
/// Use only on blocking workers. It is not a database connection lock and must
/// never be held across an await. Callers must not reenter the same gate.
#[derive(Debug, Default)]
pub struct ExecutionGate(Mutex<()>);

/// A prior panic made application-wide coherent execution unavailable.
#[derive(Debug, thiserror::Error)]
#[error("application execution gate is poisoned")]
pub struct ExecutionGateError;

impl ExecutionGate {
    /// Runs one complete use case or snapshot under exclusive application ownership.
    ///
    /// # Errors
    /// Fails closed if a prior operation panicked while owning the gate.
    pub fn run<T>(&self, work: impl FnOnce() -> T) -> Result<T, ExecutionGateError> {
        let _guard = self.0.lock().map_err(|_| ExecutionGateError)?;
        Ok(work())
    }
}
