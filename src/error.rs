use std::{error::Error as StdError, fmt};

/// Main error type of this crate.
/// When possible we return a `fjall::Error` directly, but if we have to encode or decode a value we have to return the associated error.
#[derive(Debug)]
pub enum Error<KeyError, ValueError> {
    /// An original fjall error. See [`fjall::Error`] for more info.
    Fjall(fjall::Error),
    /// An encoding or decoding error happened while working with the key codec.
    Key(KeyError),
    /// An encoding or decoding error happened while working with the value codec.
    Value(ValueError),
}

impl<KeyError, ValueError> Error<KeyError, ValueError> {
    /// Unwrap the [`fjall::Error`] and panic if it was another variant.
    pub fn unwrap_fjall(self) -> fjall::Error {
        match self {
            Error::Fjall(error) => error,
            Error::Key(_) => panic!("Unwrapped a non fjall error"),
            Error::Value(_) => panic!("Unwrapped a non fjall error"),
        }
    }
}

impl<KeyError: StdError, ValueError: StdError> fmt::Display for Error<KeyError, ValueError> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Fjall(error) => write!(f, "{error}"),
            Error::Key(error) => write!(f, "{error}"),
            Error::Value(error) => write!(f, "{error}"),
        }
    }
}
impl<KeyError: StdError, ValueError: StdError> StdError for Error<KeyError, ValueError> {}
