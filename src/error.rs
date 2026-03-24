use thiserror::Error;

#[derive(Debug, Error)]
pub enum SzalError {
    #[error("step failed: {step} — {reason}")]
    StepFailed { step: String, reason: String },
    #[error("step timeout: {step} after {timeout_ms}ms")]
    StepTimeout { step: String, timeout_ms: u64 },
    #[error("flow invalid: {0}")]
    InvalidFlow(String),
    #[error("retry exhausted: {step} after {attempts} attempts")]
    RetryExhausted { step: String, attempts: u32 },
    #[error("rollback failed: {step} — {reason}")]
    RollbackFailed { step: String, reason: String },
    #[error("cycle detected in DAG: {0}")]
    CycleDetected(String),
    #[cfg(feature = "hardware")]
    #[error(
        "hardware unavailable: step '{step}' requires {requirement} but no matching device found"
    )]
    HardwareUnavailable { step: String, requirement: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
