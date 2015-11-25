extern crate num;

pub mod pull {
	use std;
	use std::io::{Read, Result, Error, ErrorKind};
	use pull::num::traits::{CheckedAdd, CheckedMul};

	const CURLY_L: u8 = '{' as u8;
	const CURLY_R: u8 = '}' as u8;
	const SP: u8 = ' ' as u8;
	const CR: u8 = '\r' as u8;
	const LF: u8 = '\n' as u8;
	const NUM_0: u8 = '0' as u8;
	const NUM_9: u8 = '9' as u8;

	pub struct PullParser<'a> {
		source: &'a mut Read,
	}

	impl<'a> PullParser<'a> {
		pub fn new<'b>(source: &'b mut Read) -> PullParser<'b> {
			PullParser {
				source: source,
			}
		}

		pub fn get_message<'x, 'y: 'x+'y>(&'y mut self) -> Message<'x, 'a> {
			Message::new(self)
		}
	}

	enum MessageParserState {
		ExpectingField,
		ReadingField,
		Done,
	}

	pub struct Message<'a, 'b: 'a + 'b> {
		source: &'a mut PullParser<'b>,
		state: MessageParserState,
	}

	impl<'a, 'b> Message<'a, 'b> {
		pub fn new(source: &'a mut PullParser<'b>) -> Message<'a, 'b> {
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
				MessageParserState::Done => Ok(None)
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
		pub fn new(source: &'a mut Message<'b, 'c>) -> Field<'a, 'b, 'c> {
			Field {
				source: source,
				state: FieldParserState::Initial,
			}
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
		fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
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
											// TODO: Set error state in parser and message objects, there is no way to recover.
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
											// TODO: Set error state in parser and message objects, there is no way to recover.
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
								None => break
							}
						}
					},
					FieldParserState::ReadingEscapedBytes(size_left) => {
						let try_to_read = std::cmp::min(buf.len()-cursor, size_left);
						let read_bytes = try!{reader.read(&mut buf[cursor..cursor+try_to_read])};
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
							return Err(Error::new(kind, data.clone()));
						}
					},
				}
			}
			Ok(cursor)
		}
	}
}

#[cfg(test)]
mod test {
	use std::io::{Read, Cursor};

	use pull::*;

	fn buffer_message(message: &mut pull::Message) -> Vec<String> {
		let mut parsed_message = Vec::<String>::new();
		while let Ok(Some(mut field)) = message.get_field() {
			let mut buffer = String::new();
			field.read_to_string(&mut buffer).unwrap();
			parsed_message.push(buffer);
		}
		parsed_message
	}

	#[test]
	fn it_works() {
		let mut data = Cursor::new(String::from("0 ape katt lol").into_bytes());
		let mut parser = pull::PullParser::new(&mut data);

		let mut message = parser.get_message();

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
		let mut data = Cursor::new(String::from("0 ape katt\n1 tam ape\n2 lol\n").into_bytes());
		let mut parser = pull::PullParser::new(&mut data);

		let mut parsed_messages = Vec::<Vec<String>>::new();
		for _ in 0..3 {
			parsed_messages.push(buffer_message(&mut parser.get_message()));
		}

		assert_eq!(
			vec![
				vec!["0", "ape", "katt"],
				vec!["1", "tam", "ape"],
				vec!["2", "lol"],
			],
			parsed_messages
		);
	}

	#[test]
	fn it_can_parse_escape_sequences() {
		let mut data = Cursor::new(String::from("{6}0{1} a{10}pe katt\nlol fie{3}ld 2\n").into_bytes());
		let mut parser = pull::PullParser::new(&mut data);

		assert_eq!(vec!["0{1} ape katt\nlol", "field 2"], buffer_message(&mut parser.get_message()));
	}

	#[test]
	fn it_handles_escape_overflow() {
		let mut data = Cursor::new(String::from("{9000000000000000000000}blahblah\n").into_bytes());
		let mut parser = pull::PullParser::new(&mut data);
		let mut buffer = String::new();
		let result = parser.get_message().get_field().unwrap().unwrap().read_to_string(&mut buffer);
		assert!(result.is_err());
	}

	#[test]
	fn it_understands_crlf() {
		let mut data = Cursor::new(String::from("0 ape\r\n1 katt\r\n").into_bytes());
		let mut parser = pull::PullParser::new(&mut data);

		let mut parsed_messages = Vec::<Vec<String>>::new();
		for _ in 0..2 {
			parsed_messages.push(buffer_message(&mut parser.get_message()));
		}

		assert_eq!(
			vec![
				vec!["0", "ape"],
				vec!["1", "katt"],
			],
			parsed_messages
		);
	}
}
