use std::io;
use std::convert;

#[derive(Debug)]
pub enum Error {
	Io(io::Error),
	Unspecified(&'static str),
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
