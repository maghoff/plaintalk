use std::result::Result;
use std::io::Read;

use super::error::*;
use super::pullparser::*;
use super::field::*;

pub enum MessageParserState {
	ExpectingField,
	ReadingField,
	Done,
	Error(&'static str),
}

pub struct Message<'a> {
	inner: &'a mut Read,
	parser_state: &'a mut PullParserState,
	state: MessageParserState,
	empty: bool,
}

#[doc(hidden)]
pub trait MessageInternal<'a> {
	fn new(inner: &'a mut Read, parser_state: &'a mut PullParserState) -> Message<'a>;
}

impl<'a> MessageInternal<'a> for Message<'a> {
	fn new(inner: &'a mut Read, parser_state: &'a mut PullParserState) -> Message<'a> {
		Message {
			inner: inner,
			parser_state: parser_state,
			state: MessageParserState::ExpectingField,
			empty: true,
		}
	}
}

impl<'a> Message<'a> {
	pub fn get_field<'x, 'y: 'x+'y>(&'y mut self) -> Result<Option<Field<'x, 'y>>, &'static str> {
		match self.state {
			MessageParserState::ExpectingField => {
				self.state = MessageParserState::ReadingField;
				Ok(Some(Field::new(self.inner, self.parser_state, &mut self.state, &mut self.empty)))
			},
			MessageParserState::ReadingField => Err("You need to finish reading the field"),
			MessageParserState::Done => Ok(None),
			MessageParserState::Error(err) => Err(err),
		}
	}

	pub fn ignore_rest(&mut self) -> Result<(), Error> {
		while let Some(mut field) = try!{self.get_field()} {
			try!{field.ignore_rest()};
		}
		Ok(())
	}

	pub fn read_field(&mut self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
		match try!{self.get_field()} {
			Some(mut field) => {
				let mut cursor = 0;
				while cursor < buf.len() {
					let count = try!{field.read(&mut buf[cursor..])};
					cursor += count;
					if count == 0 { break; }
				}
				if cursor == buf.len() {
					// At this point we may or may not have reached EOF
					let lookahead = &mut [0u8];
					match try!{field.read(lookahead)} {
						0 => {},
						_ => {
							try!{field.ignore_rest()};
							return Err(Error::Unspecified("Overflow"))
						},
					}
				}
				Ok(Some(cursor))
			},
			None => {
				Ok(None)
			}
		}
	}

	pub fn read_field_to_end(&mut self, buf: &mut Vec<u8>) -> Result<Option<usize>, Error> {
		match try!{self.get_field()} {
			Some(mut field) => {
				let len = try!{field.read_to_end(buf)};
				Ok(Some(len))
			},
			None => {
				Ok(None)
			}
		}
	}

	pub fn read_field_as_string(&mut self) -> Result<Option<String>, Error> {
		let mut string = String::new();
		match try!{self.get_field()} {
			Some(mut field) => {
				try!{field.read_to_string(&mut string)};
				Ok(Some(string))
			},
			None => Ok(None)
		}
	}

	pub fn read_field_as_slice<'x, 'y: 'x+'y>(&mut self, buffer: &'y mut[u8]) -> Result<Option<&'x [u8]>, Error> {
		match try!{self.read_field(buffer)} {
			Some(len) => {
				Ok(Some(&buffer[0..len]))
			},
			None => Ok(None)
		}
	}

	pub fn at_end(&self) -> bool {
		match self.state {
			MessageParserState::Done => true,
			_ => false
		}
	}
}
