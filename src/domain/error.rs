use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MicoError {
    #[error("could not resolve the current user's home directory")]
    HomeDirectoryUnavailable,
    #[error("required dependency `{0}` is missing")]
    MissingDependency(String),
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error("this feature is not yet configured: {0}")]
    NotConfigured(String),
}
