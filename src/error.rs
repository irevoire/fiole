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
    /// Returns the [`fjall::Error`], or `None` if it was a codec error.
    pub fn fjall_error(self) -> Option<fjall::Error> {
        match self {
            Error::Fjall(error) => Some(error),
            Error::Key(_) | Error::Value(_) => None,
        }
    }

    /// Unwrap the [`fjall::Error`] and panic if it was another variant.
    ///
    /// See [`Error::fjall_error`] for a non-panicking version.
    pub fn unwrap_fjall(self) -> fjall::Error {
        match self {
            Error::Fjall(error) => error,
            Error::Key(_) => panic!("called `Error::unwrap_fjall()` on a `Key` error"),
            Error::Value(_) => panic!("called `Error::unwrap_fjall()` on a `Value` error"),
        }
    }
}

impl<KeyError, ValueError> From<fjall::Error> for Error<KeyError, ValueError> {
    fn from(error: fjall::Error) -> Self {
        Error::Fjall(error)
    }
}

impl<KeyError: fmt::Display, ValueError: fmt::Display> fmt::Display
    for Error<KeyError, ValueError>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Fjall(error) => write!(f, "{error}"),
            Error::Key(error) => write!(f, "key codec error: {error}"),
            Error::Value(error) => write!(f, "value codec error: {error}"),
        }
    }
}

/// The message of the inner error is already part of [`fmt::Display`], so
/// [`StdError::source`] skips it and returns its own source.
impl<KeyError: StdError, ValueError: StdError> StdError for Error<KeyError, ValueError> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Fjall(error) => error.source(),
            Error::Key(error) => error.source(),
            Error::Value(error) => error.source(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{error::Error as StdError, fmt, io};

    use crate::Error;

    /// An error that only implements `Display`.
    #[derive(Debug)]
    struct DisplayOnly;

    impl fmt::Display for DisplayOnly {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("display only")
        }
    }

    /// An error caused by another one.
    #[derive(Debug)]
    struct Outer(io::Error);

    impl fmt::Display for Outer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("outer")
        }
    }

    impl StdError for Outer {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            Some(&self.0)
        }
    }

    type TestError = Error<Outer, Outer>;

    fn fjall_error() -> fjall::Error {
        fjall::Error::Io(io::Error::other("disk"))
    }

    #[test]
    fn from_fjall() {
        fn fails() -> Result<(), TestError> {
            Err(fjall_error())?
        }
        assert!(matches!(fails(), Err(Error::Fjall(fjall::Error::Io(_)))));
    }

    #[test]
    fn display_says_which_codec_failed() {
        let key: TestError = Error::Key(Outer(io::Error::other("inner")));
        assert_eq!(key.to_string(), "key codec error: outer");
        let value: TestError = Error::Value(Outer(io::Error::other("inner")));
        assert_eq!(value.to_string(), "value codec error: outer");
        let fjall: TestError = Error::Fjall(fjall_error());
        assert_eq!(fjall.to_string(), fjall_error().to_string());
    }

    #[test]
    fn display_only_needs_display() {
        let error: Error<DisplayOnly, DisplayOnly> = Error::Key(DisplayOnly);
        assert_eq!(error.to_string(), "key codec error: display only");
    }

    #[test]
    fn source_continues_the_chain() {
        // The inner message is already in `Display`, so the chain continues with its source
        let key: TestError = Error::Key(Outer(io::Error::other("inner")));
        assert_eq!(key.source().unwrap().to_string(), "inner");
        let value: TestError = Error::Value(Outer(io::Error::other("inner")));
        assert_eq!(value.source().unwrap().to_string(), "inner");
        let fjall: TestError = Error::Fjall(fjall_error());
        assert_eq!(fjall.source().unwrap().to_string(), "disk");

        let error: Error<io::Error, io::Error> = Error::Key(io::Error::other("no source"));
        assert!(error.source().is_none());
    }

    #[test]
    fn fjall_error_is_non_panicking() {
        let fjall: TestError = Error::Fjall(fjall_error());
        assert!(matches!(fjall.fjall_error(), Some(fjall::Error::Io(_))));
        let key: TestError = Error::Key(Outer(io::Error::other("inner")));
        assert!(key.fjall_error().is_none());
    }

    #[test]
    #[should_panic(expected = "called `Error::unwrap_fjall()` on a `Value` error")]
    fn unwrap_fjall_says_what_it_found() {
        let value: TestError = Error::Value(Outer(io::Error::other("inner")));
        value.unwrap_fjall();
    }
}
