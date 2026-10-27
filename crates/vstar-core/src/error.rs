use thiserror::Error;

/// Errors that can occur while loading or parsing observations.
#[derive(Debug, Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(String),

    #[error("star \"{0}\" was not found")]
    StarNotFound(String),

    #[error("could not read the data: {0}")]
    Parse(String),

    #[error("file error: {0}")]
    Io(#[from] std::io::Error),

    #[error("AI request failed: {0}")]
    Ai(String),

    #[error("couldn't use the system credential store: {0}")]
    Secret(String),

    #[error("loading was cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<csv::Error> for Error {
    fn from(e: csv::Error) -> Self {
        Error::Parse(e.to_string())
    }
}

impl From<roxmltree::Error> for Error {
    fn from(e: roxmltree::Error) -> Self {
        Error::Parse(e.to_string())
    }
}
