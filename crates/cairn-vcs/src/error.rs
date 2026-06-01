//! I/O failures while reading an otherwise-present git metadata layout.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that prevent completing a [`RepoEpoch`](cairn_types::RepoEpoch) capture.
///
/// A missing or unreadable repository is **not** an error: [`capture`](crate::capture)
/// returns `Ok` with [`OperationState::UnknownVcs`](cairn_types::OperationState::UnknownVcs).
#[derive(Debug, Error)]
pub enum VcsError {
    #[error("failed to read git metadata at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl VcsError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
