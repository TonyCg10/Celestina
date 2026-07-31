//! Typed engine failures.
//!
//! Every variant says what the engine was doing and keeps the backend's own
//! error as its source, because "playback failed" without the cause is what
//! turns a missing codec into an unfixable bug report.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum EngineError {
    /// The backend could not be created at all — no libmpv, or a rejected
    /// initial option.
    BackendUnavailable { source: libmpv2::Error },
    /// A path that is not absolute, or not the kind the operation accepts.
    UnusableSource { path: PathBuf, reason: &'static str },
    /// The backend refused an option, command or property.
    Backend {
        operation: &'static str,
        source: libmpv2::Error,
    },
    /// The file was loaded but nothing usable came out of it.
    Undecodable { path: PathBuf, detail: String },
    /// The operation ran out of its time budget.
    TimedOut {
        operation: &'static str,
        after: std::time::Duration,
    },
    /// The job was cancelled before it produced anything.
    Cancelled,
    /// Filesystem failure while publishing a derived resource.
    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    /// A derived resource came out larger than the budget it was made for, so
    /// it was discarded instead of published.
    OverBudget {
        what: &'static str,
        limit: u64,
        actual: u64,
    },
    /// The worker thread is gone, so no further job can be accepted.
    WorkerStopped,
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable { .. } => {
                formatter.write_str("the media backend could not be started")
            }
            Self::UnusableSource { path, reason } => {
                write!(formatter, "unusable source {}: {reason}", path.display())
            }
            Self::Backend { operation, .. } => {
                write!(formatter, "the media backend rejected {operation}")
            }
            Self::Undecodable { path, detail } => {
                write!(formatter, "cannot decode {}: {detail}", path.display())
            }
            Self::TimedOut { operation, after } => {
                write!(formatter, "{operation} exceeded its budget of {after:?}")
            }
            Self::Cancelled => formatter.write_str("the job was cancelled"),
            Self::Io {
                operation, path, ..
            } => write!(formatter, "{operation} failed on {}", path.display()),
            Self::OverBudget {
                what,
                limit,
                actual,
            } => write!(formatter, "{what} exceeded its budget: {actual} > {limit}"),
            Self::WorkerStopped => formatter.write_str("the engine worker is no longer running"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BackendUnavailable { source } | Self::Backend { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl EngineError {
    /// Whether retrying the same request could plausibly succeed. A cancelled
    /// or timed-out job may; an undecodable file will not.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Cancelled | Self::TimedOut { .. })
    }

    /// The message a host may show. It never includes a raw path, which the UI
    /// already knows and may not be able to render losslessly.
    #[must_use]
    pub fn user_message(&self) -> String {
        match self {
            Self::BackendUnavailable { .. } => "No se pudo iniciar el motor multimedia".to_owned(),
            Self::UnusableSource { .. } => "Este archivo no se puede abrir".to_owned(),
            Self::Backend { .. } => "El motor multimedia rechazó la operación".to_owned(),
            Self::Undecodable { .. } => "No hay decodificador para este archivo".to_owned(),
            Self::TimedOut { .. } => "La operación tardó demasiado".to_owned(),
            Self::Cancelled => "Operación cancelada".to_owned(),
            Self::OverBudget { .. } => "La vista previa excedió su presupuesto".to_owned(),
            Self::Io { .. } => "No se pudo escribir el resultado".to_owned(),
            Self::WorkerStopped => "El motor multimedia se detuvo".to_owned(),
        }
    }
}

pub type EngineResult<T> = Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::EngineError;
    use std::error::Error;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn an_io_failure_keeps_its_cause() {
        let error = EngineError::Io {
            operation: "publish artwork",
            path: PathBuf::from("/tmp/x.png"),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };

        assert!(error.to_string().contains("/tmp/x.png"));
        assert!(error.source().is_some());
        assert!(!error.is_retryable());
    }

    #[test]
    fn only_transient_failures_invite_a_retry() {
        assert!(EngineError::Cancelled.is_retryable());
        assert!(EngineError::TimedOut {
            operation: "probe",
            after: Duration::from_secs(5),
        }
        .is_retryable());
        assert!(!EngineError::Undecodable {
            path: PathBuf::from("/tmp/x.mkv"),
            detail: "no video track".to_owned(),
        }
        .is_retryable());
    }

    #[test]
    fn the_user_message_never_leaks_a_path() {
        let error = EngineError::Undecodable {
            path: PathBuf::from("/home/toni/privado/secreto.mkv"),
            detail: "no decoder".to_owned(),
        };

        assert!(!error.user_message().contains("secreto"));
        assert!(error.to_string().contains("secreto"), "el log sí lo dice");
    }
}
