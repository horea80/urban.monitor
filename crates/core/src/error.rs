#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("HTTP {url}: status {status}")]
    Status { url: String, status: u16 },
    #[error("parsare {url}: {msg}")]
    Parse { url: String, msg: String },
    #[error("PDF: {0}")]
    Pdf(String),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Pool(#[from] r2d2::Error),
    #[error(transparent)]
    Migration(#[from] rusqlite_migration::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn parse(url: &str, msg: impl Into<String>) -> Self {
        Error::Parse {
            url: url.to_owned(),
            msg: msg.into(),
        }
    }
}
