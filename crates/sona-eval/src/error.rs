//! Harness errors. A small typed enum so the "fail loudly if the corpus is
//! absent" requirement (ADR-0011, ticket 01) reads as a specific, actionable
//! message rather than a bare string.

use std::fmt;
use std::path::PathBuf;

/// Anything that can go wrong loading evaluation data.
#[derive(Debug)]
pub enum EvalError {
    /// An I/O failure, with the path that caused it.
    Io { path: PathBuf, source: std::io::Error },
    /// A malformed annotation/CSV/JSON line, with a human-readable reason.
    Parse { path: PathBuf, reason: String },
    /// The Vocadito corpus directory is missing or not configured. Carries a
    /// hint on how to populate it, since this is the expected first-run state.
    CorpusMissing(PathBuf),
    /// A WAV fixture could not be decoded.
    Wav { path: PathBuf, reason: String },
}

impl EvalError {
    /// An I/O error tagged with the offending path.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        EvalError::Io { path: path.into(), source }
    }

    /// A parse error tagged with the offending path.
    pub fn parse(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        EvalError::Parse { path: path.into(), reason: reason.into() }
    }

    /// A WAV decode/encode error tagged with the offending path.
    pub fn wav(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        EvalError::Wav { path: path.into(), reason: reason.into() }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            EvalError::Parse { path, reason } => write!(f, "{}: {reason}", path.display()),
            EvalError::CorpusMissing(path) => write!(
                f,
                "Vocadito corpus not found at {}. Set SONA_VOCADITO_DIR or run \
                 crates/sona-eval/scripts/fetch-vocadito.sh to download it.",
                path.display()
            ),
            EvalError::Wav { path, reason } => {
                write!(f, "{}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for EvalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EvalError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
