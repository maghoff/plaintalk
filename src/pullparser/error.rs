use std::convert;
use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
	Io(io::Error),
	Unspecified(&'static str),
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Error::Io(ref err) => write!(f, "IO error: {}", err),
			Error::Unspecified(ref err) => write!(f, "Unspecified error: {}", err),
		}
	}
}

impl error::Error for Error {
	fn description(&self) -> &str {
		match *self {
			Error::Io(ref err) => err.description(),
			Error::Unspecified(ref err) => err,
		}
	}

	fn cause(&self) -> Option<&error::Error> {
		match *self {
			Error::Io(ref err) => Some(err),
			Error::Unspecified(_) => None,
		}
	}
}

impl convert::From<io::Error> for Error {
	fn from(err: io::Error) -> Error {
		Error::Io(err)
	}
}

impl convert::From<&'static str> for Error {
	fn from(err: &'static str) -> Error {
		Error::Unspecified(err)
	}
}
