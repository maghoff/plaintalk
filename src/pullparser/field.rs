use std::cmp;
use std::result::Result;
use std::io::{self, Read, ErrorKind};

use super::error::*;
use super::pullparser::*;
use super::message::*;

const CURLY_L: u8 = b'{';
const CURLY_R: u8 = b'}';
const SP: u8 = b' ';
const CR: u8 = b'\r';
const LF: u8 = b'\n';
const NUM_0: u8 = b'0';
const NUM_9: u8 = b'9';

enum FieldParserState {
	Initial,
	ReadingEscapedBytes(usize),
	Done,
	Error(ErrorKind, &'static str)
}

pub struct Field<'a, 'b: 'a + 'b> {
	inner: &'b mut Read,
	parser_state: &'b mut PullParserState,
	message_state: &'a mut MessageParserState,
	empty: &'a mut bool,
	state: FieldParserState,
}

#[doc(hidden)]
pub trait FieldInternal<'a, 'b> {
	fn new(
		inner: &'b mut Read,
		parser_state: &'b mut PullParserState,
		state: &'a mut MessageParserState,
		empty: &'a mut bool,
	) -> Field<'a, 'b>;
}

impl<'a, 'b> FieldInternal<'a, 'b> for Field<'a, 'b> {
	fn new(
		inner: &'b mut Read,
		parser_state: &'b mut PullParserState,
		message_state: &'a mut MessageParserState,
		empty: &'a mut bool,
	) -> Field<'a, 'b> {
		Field {
			inner: inner,
			parser_state: parser_state,
			message_state: message_state,
			empty: empty,
			state: FieldParserState::Initial,
		}
	}
}

impl<'a, 'b> Field<'a, 'b> {
	pub fn ignore_rest(&mut self) -> Result<(), Error> {
		let mut buf = [0u8; 256];
		while try!{self.read(&mut buf)} > 0 {}
		Ok(())
	}
}

fn parse_escape_header<T: Read>(bytes: &mut io::Bytes<T>) -> Result<usize, (ErrorKind, &'static str)> {
	let mut escaped_bytes: usize = 0;
	loop {
		match bytes.next() {
			Some(Ok(CURLY_R)) => {
				return Ok(escaped_bytes);
			},
			Some(Ok(x)) if NUM_0 <= x && x <= NUM_9 => {
				match escaped_bytes.checked_mul(10).and_then(|y| y.checked_add((x - NUM_0) as usize)) {
					Some(y) => escaped_bytes = y,
					None => return Err((ErrorKind::InvalidData, "Overflow in PlainTalk escape sequence")),
				}
			},
			_ => return Err((ErrorKind::InvalidData, "Invalid symbol in PlainTalk escape sequence")),
		}
	}
}

impl<'a, 'b> Read for Field<'a, 'b> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let reader = &mut self.inner;
		let mut cursor:usize = 0;

		while cursor < buf.len() {
			match self.state {
				FieldParserState::Initial => {
					let mut bytes = reader.bytes();

					while match self.state { FieldParserState::Initial => true, _ => false } && cursor < buf.len() {
						match bytes.next() {
							Some(Ok(CURLY_L)) => {
								match parse_escape_header(&mut bytes) {
									Ok(escaped_bytes) => self.state = FieldParserState::ReadingEscapedBytes(escaped_bytes),
									Err(err) => {
										*self.parser_state = PullParserState::Error(err.1);
										*self.message_state = MessageParserState::Error(err.1);
										self.state = FieldParserState::Error(err.0, err.1);
									}
								}
							},
							Some(Ok(SP)) => {
								*self.message_state = MessageParserState::ExpectingField;
								self.state = FieldParserState::Done;
							},
							Some(Ok(LF)) => {
								if !(*self.empty && cursor == 0) {
									*self.message_state = MessageParserState::Done;
									self.state = FieldParserState::Done;
								}
							},
							Some(Ok(CR)) => {
								match bytes.next() {
									Some(Ok(LF)) => {
										if !(*self.empty && cursor == 0) {
											*self.message_state = MessageParserState::Done;
											self.state = FieldParserState::Done;
										}
									},
									_ => {
										*self.parser_state = PullParserState::Error("Invalid byte after CR");
										*self.message_state = MessageParserState::Error("Invalid byte after CR");
										self.state = FieldParserState::Error(ErrorKind::InvalidData, "Invalid byte after CR");
									}
								}
							},
							Some(Ok(ch)) => {
								buf[cursor] = ch;
								cursor += 1;
							},
							Some(Err(err)) => {
								// TODO Maybe put the whole parser in an error state?
								return Err(err)
							},
							None => {
								if *self.empty && (cursor == 0) {
									*self.parser_state = PullParserState::Done;
									*self.message_state = MessageParserState::Done;
									self.state = FieldParserState::Done;
								} else {
									*self.parser_state = PullParserState::Error("Unexpected EOF");
									*self.message_state = MessageParserState::Error("Unexpected EOF");
									self.state = FieldParserState::Error(ErrorKind::InvalidData, "Unexpected EOF");
								}
							}
						}
					}
				},
				FieldParserState::ReadingEscapedBytes(size_left) => {
					// TODO It is possible to reach EOF here. It should be considered an
					// error, and the parser should stop
					let try_to_read = cmp::min(buf.len()-cursor, size_left);
					let read_bytes = try!{reader.read(&mut buf[cursor..cursor+try_to_read])};
					// TODO An error ^^here should probably terminate the parser
					cursor += read_bytes;
					self.state = match size_left - read_bytes {
						0 => FieldParserState::Initial,
						x => FieldParserState::ReadingEscapedBytes(x),
					};
				},
				FieldParserState::Done => break,
				FieldParserState::Error(kind, data) => {
					if cursor > 0 {
						break;
					} else {
						return Err(io::Error::new(kind, data.clone()));
					}
				},
			}
		}
		*self.empty = *self.empty && (cursor == 0);
		Ok(cursor)
	}
}
