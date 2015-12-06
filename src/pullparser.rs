extern crate num;

use std;
use std::convert;
use std::result::Result;
use std::io::{self, Read, ErrorKind};
use self::num::traits::{CheckedAdd, CheckedMul};

const CURLY_L: u8 = '{' as u8;
const CURLY_R: u8 = '}' as u8;
const SP: u8 = ' ' as u8;
const CR: u8 = '\r' as u8;
const LF: u8 = '\n' as u8;
const NUM_0: u8 = '0' as u8;
const NUM_9: u8 = '9' as u8;

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

enum PullParserState {
	Initial,
	Done,
	Error(&'static str),
}

pub struct PullParser<'a> {
	source: &'a mut Read,
	state: PullParserState,
}

impl<'a> PullParser<'a> {
	pub fn new<'b>(source: &'b mut Read) -> PullParser<'b> {
		PullParser {
			source: source,
			state: PullParserState::Initial,
		}
	}

	pub fn get_message<'x, 'y: 'x+'y>(&'y mut self) -> Result<Option<Message<'x, 'a>>, &'static str> {
		match self.state {
			PullParserState::Initial => Ok(Some(Message::new(self))),
			PullParserState::Done => Ok(None),
			PullParserState::Error(err) => Err(err),
		}
	}

	pub fn read_message(&mut self) -> Result<Option<Vec<Vec<u8>>>, Error> {
		if let Some(mut message) = try!{self.get_message()} {
			let mut buffered_message = Vec::<Vec<u8>>::new();
			loop {
				let mut buffered_field = Vec::<u8>::new();
				match try!{message.read_field_to_end(&mut buffered_field)} {
					Some(_) => buffered_message.push(buffered_field),
					None => break
				}
			}
			Ok(Some(buffered_message))
		} else {
			Ok(None)
		}
	}
}

enum MessageParserState {
	ExpectingField,
	ReadingField,
	Done,
	Error(&'static str),
}

pub struct Message<'a, 'b: 'a + 'b> {
	source: &'a mut PullParser<'b>,
	state: MessageParserState,
}

impl<'a, 'b> Message<'a, 'b> {
	fn new(source: &'a mut PullParser<'b>) -> Message<'a, 'b> {
		Message {
			source: source,
			state: MessageParserState::ExpectingField,
		}
	}

	pub fn get_field<'x, 'y: 'x+'y>(&'y mut self) -> std::result::Result<Option<Field<'x, 'a, 'b>>, &'static str> {
		match self.state {
			MessageParserState::ExpectingField => {
				self.state = MessageParserState::ReadingField;
				Ok(Some(Field::new(self)))
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

enum FieldParserState {
	Initial,
	ReadingEscapedBytes(usize),
	Done,
	Error(ErrorKind, &'static str)
}

pub struct Field<'a, 'b: 'a + 'b, 'c: 'b + 'c> {
	source: &'a mut Message<'b, 'c>,
	state: FieldParserState,
}

impl<'a, 'b, 'c> Field<'a, 'b, 'c> {
	fn new(source: &'a mut Message<'b, 'c>) -> Field<'a, 'b, 'c> {
		Field {
			source: source,
			state: FieldParserState::Initial,
		}
	}

	pub fn ignore_rest(&mut self) -> Result<(), Error> {
		let mut buf = [0u8; 256];
		while try!{self.read(&mut buf)} > 0 {}
		Ok(())
	}
}

fn parse_escape_header<T:Read>(bytes: &mut std::io::Bytes<T>) -> std::result::Result<usize, (ErrorKind, &'static str)> {
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

impl<'a, 'b, 'c> Read for Field<'a, 'b, 'c> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let reader = &mut self.source.source.source;
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
										self.source.source.state = PullParserState::Error(err.1);
										self.source.state = MessageParserState::Error(err.1);
										self.state = FieldParserState::Error(err.0, err.1);
									}
								}
							},
							Some(Ok(SP)) => {
								self.source.state = MessageParserState::ExpectingField;
								self.state = FieldParserState::Done;
							},
							Some(Ok(LF)) => {
								self.source.state = MessageParserState::Done;
								self.state = FieldParserState::Done;
							},
							Some(Ok(CR)) => {
								match bytes.next() {
									Some(Ok(LF)) => {
										self.source.state = MessageParserState::Done;
										self.state = FieldParserState::Done;
									},
									_ => {
										self.source.source.state = PullParserState::Error("Invalid byte after CR");
										self.source.state = MessageParserState::Error("Invalid byte after CR");
										self.state = FieldParserState::Error(ErrorKind::InvalidData, "Invalid byte after CR");
									}
								}
							},
							Some(Ok(ch)) => {
								buf[cursor] = ch;
								cursor += 1;
							},
							Some(Err(err)) => {
								return Err(err)
							},
							None => {
								// TODO It should be considered an error to reach EOF unless
								// we are at the very beginning of a message
								self.source.source.state = PullParserState::Done;
								self.source.state = MessageParserState::Done;
								self.state = FieldParserState::Done;
							}
						}
					}
				},
				FieldParserState::ReadingEscapedBytes(size_left) => {
					// TODO It is possible to reach EOF here. It should be considered an
					// error, and the parser should stop
					let try_to_read = std::cmp::min(buf.len()-cursor, size_left);
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
		Ok(cursor)
	}
}

#[cfg(test)]
mod test {
	use std::io::{Read, Cursor};
	use pullparser::*;

	fn buffer_message(message: &mut Message) -> Vec<String> {
		let mut parsed_message = Vec::<String>::new();
		while let Ok(Some(mut field)) = message.get_field() {
			let mut buffer = String::new();
			field.read_to_string(&mut buffer).unwrap();
			parsed_message.push(buffer);
		}
		parsed_message
	}

	fn buffer_all_messages(parser: &mut PullParser) -> Vec<Vec<String>> {
		let mut parsed_messages = Vec::<Vec<String>>::new();
		while let Ok(Some(mut message)) = parser.get_message() {
			parsed_messages.push(buffer_message(&mut message));
		}
		parsed_messages
	}

	#[test]
	fn it_works() {
		let mut data = Cursor::new(String::from("0 ape katt lol").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();

		let mut buffer = String::new();

		message.get_field().unwrap().unwrap().read_to_string(&mut buffer).unwrap();
		assert_eq!("0", buffer);
		buffer.clear();

		message.get_field().unwrap().unwrap().read_to_string(&mut buffer).unwrap();
		assert_eq!("ape", buffer);
		buffer.clear();

		message.get_field().unwrap().unwrap().read_to_string(&mut buffer).unwrap();
		assert_eq!("katt", buffer);
		buffer.clear();
	}

	#[test]
	fn it_can_parse_several_messages() {
		let mut data = Cursor::new(String::from("0 ape katt\n1 tam ape\n2 lol").into_bytes());
		let mut parser = PullParser::new(&mut data);

		assert_eq!(
			vec![
				vec!["0", "ape", "katt"],
				vec!["1", "tam", "ape"],
				vec!["2", "lol"],
			],
			buffer_all_messages(&mut parser)
		);
	}

	#[test]
	fn it_can_parse_escape_sequences() {
		let mut data = Cursor::new(String::from("{6}0{1} a{10}pe katt\nlol fie{3}ld 2\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		assert_eq!(vec!["0{1} ape katt\nlol", "field 2"], buffer_message(&mut parser.get_message().unwrap().unwrap()));
	}

	#[test]
	fn it_handles_escape_overflow() {
		let mut data = Cursor::new(String::from("{9000000000000000000000}blahblah\n").into_bytes());
		let mut parser = PullParser::new(&mut data);
		let mut buffer = String::new();
		let result = parser.get_message().unwrap().unwrap().get_field().unwrap().unwrap().read_to_string(&mut buffer);
		assert!(result.is_err());
	}

	#[test]
	fn it_understands_crlf() {
		let mut data = Cursor::new(String::from("0 ape\r\n1 katt\r\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		assert_eq!(
			vec![
				vec!["0", "ape"],
				vec!["1", "katt"],
				vec![""],
			],
			buffer_all_messages(&mut parser)
		);
	}

	#[test]
	fn it_can_ignore_a_field() {
		let mut data = Cursor::new(String::from("field1 field2\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();

		message.get_field().unwrap().unwrap().ignore_rest().unwrap();

		let mut buffer = String::new();
		message.get_field().unwrap().unwrap().read_to_string(&mut buffer).unwrap();
		assert_eq!("field2", buffer);
	}

	#[test]
	fn it_can_ignore_a_message() {
		let mut data = Cursor::new(String::from("msg1 msg1field2\nmsg2 msg2field2\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut buffer = String::new();

		{
			let mut message = parser.get_message().unwrap().unwrap();
			message.get_field().unwrap().unwrap().read_to_string(&mut buffer).unwrap();
			assert_eq!("msg1", buffer);
			buffer.clear();
			message.ignore_rest().unwrap();
		}

		{
			let mut message = parser.get_message().unwrap().unwrap();
			message.get_field().unwrap().unwrap().read_to_string(&mut buffer).unwrap();
			assert_eq!("msg2", buffer);
			buffer.clear();
			message.ignore_rest().unwrap();
		}
	}

	#[test]
	fn message_can_buffer_a_field() {
		let mut data = Cursor::new(String::from("0 protocol lol\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();

		{
			let buffer = &mut [0u8; 8];
			let len = message.read_field(buffer).unwrap().unwrap();
			assert_eq!(b"0".to_vec(), buffer[0..len].to_vec());
		}

		{
			let buffer = &mut [0u8; 8];
			let len = message.read_field(buffer).unwrap().unwrap();
			assert_eq!(b"protocol".to_vec(), buffer[0..len].to_vec());
		}
	}

	#[test]
	fn message_can_detect_overflow_when_buffering_a_field() {
		let mut data = Cursor::new(String::from("protocol\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();

		{
			let buffer = &mut [0u8; 4];
			assert!(message.read_field(buffer).is_err());
		}
	}

	#[test]
	fn message_can_read_field_to_end() {
		let mut data = Cursor::new(String::from("0 protocol\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();

		{
			let mut buffer = Vec::<u8>::new();
			message.read_field_to_end(&mut buffer).unwrap().unwrap();
			assert_eq!(b"0".to_vec(), buffer);
		}

		{
			let mut buffer = Vec::<u8>::new();
			message.read_field_to_end(&mut buffer).unwrap().unwrap();
			assert_eq!(b"protocol".to_vec(), buffer);
		}

		{
			let mut buffer = Vec::<u8>::new();
			let result = message.read_field_to_end(&mut buffer).unwrap();
			assert!(result.is_none());
		}
	}

	#[test]
	fn message_can_read_field_as_string() {
		let mut data = Cursor::new(String::from("0 protocol\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();

		assert_eq!("0", message.read_field_as_string().unwrap().unwrap());
		assert_eq!("protocol", message.read_field_as_string().unwrap().unwrap());
	}

	#[test]
	fn message_can_read_field_as_slice() {
		let mut data = Cursor::new(String::from("0 protocol\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();
		let mut buffer = [0u8;10];

		assert_eq!(b"0", message.read_field_as_slice(&mut buffer).unwrap().unwrap());
		assert_eq!(b"protocol", message.read_field_as_slice(&mut buffer).unwrap().unwrap());
	}

	#[test]
	fn message_can_tell_if_it_is_at_the_end() {
		let mut data = Cursor::new(String::from("0 protocol\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		let mut message = parser.get_message().unwrap().unwrap();

		assert_eq!(false, message.at_end());
		message.read_field_as_string().unwrap();
		assert_eq!(false, message.at_end());
		message.read_field_as_string().unwrap();
		assert_eq!(true, message.at_end());
	}

	#[test]
	fn parser_can_read_a_message() {
		let mut data = Cursor::new(String::from("0 protocol lol\n2 lol\n").into_bytes());
		let mut parser = PullParser::new(&mut data);

		assert_eq!([b"0".to_vec(), b"protocol".to_vec(), b"lol".to_vec()].to_vec(), parser.read_message().unwrap().unwrap());
		assert_eq!([b"2".to_vec(), b"lol".to_vec()].to_vec(), parser.read_message().unwrap().unwrap());
	}
}
