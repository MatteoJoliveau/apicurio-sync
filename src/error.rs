use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum Error {
    Http(reqwest::Error),
    Io(std::io::Error),
    Parse(Box<dyn std::error::Error>),
    Setup(String),
}

impl Error {
    pub fn setup(msg: impl ToString) -> Self {
        Self::Setup(msg.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Http(err) => err.fmt(f),
            Error::Io(err) => err.fmt(f),
            Error::Setup(msg) => msg.fmt(f),
            Error::Parse(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Http(err) => Some(err),
            Error::Io(err) => Some(err),
            Error::Parse(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Http(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Parse(Box::new(err))
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Parse(Box::new(err))
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Self::Parse(Box::new(err))
    }
}

