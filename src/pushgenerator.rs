use std::convert;
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub enum Error {
// 	Io(io::Error),
	Unspecified(&'static str),
}

impl convert::From<io::Error> for Error {
	fn from(_err: io::Error) -> Error {
// 		Error::Io(err)
		Error::Unspecified("IO error")
	}
}

enum PushGeneratorState {
	Initial,
	GeneratingMessage,
	Error(Error),
}

pub struct PushGenerator<W: Write> {
	target: W,
	state: PushGeneratorState,
	auto_flush: bool,
}

impl<W: Write> PushGenerator<W> {
	pub fn new(target: W) -> PushGenerator<W> {
		PushGenerator {
			target: target,
			state: PushGeneratorState::Initial,
			auto_flush: true,
		}
	}

	pub fn next_message<'x, 'y: 'x+'y>(&'y mut self) -> Result<Message<'x, W>, Error> {
		match self.state {
			PushGeneratorState::Initial => {
				self.state = PushGeneratorState::GeneratingMessage;
				Ok(Message::new(self))
			},
			PushGeneratorState::GeneratingMessage => {
				Err(Error::Unspecified("Finish message before starting a new one"))
			},
			PushGeneratorState::Error(ref err) => Err(err.clone())
		}
	}

	pub fn flush(&mut self) -> io::Result<()> {
		self.target.flush()
	}

	fn auto_flush(&self) -> bool {
		self.auto_flush
	}

	pub fn write_message(&mut self, msg: &[&[u8]]) -> Result<(), Error> {
		let mut message = try!{self.next_message()};
		for &fieldbuf in msg {
			try!{message.write_field(&fieldbuf)};
		}
		Ok(())
	}
}

enum MessageState {
	BeforeFirstField,
	AfterFirstField,
	GeneratingField,
}

pub struct Message<'a, W: 'a + Write> {
	target: &'a mut PushGenerator<W>,
	state: MessageState,
}

impl<'a, W: Write> Message<'a, W> {
	fn new(target: &'a mut PushGenerator<W>) -> Message<'a, W> {
		Message {
			target: target,
			state: MessageState::BeforeFirstField,
		}
	}

	pub fn next_field<'x, 'y: 'x+'y>(&'y mut self) -> Result<Field<'x, 'a, W>, Error> {
		match self.state {
			MessageState::BeforeFirstField => {
				self.state = MessageState::GeneratingField;
				Ok(Field::new(self))
			},
			MessageState::AfterFirstField => {
				// TODO Handle failure. Should the generator get into a failed
				// state? Or are we able to try the same operation again?
				if let Err(_err) = self.target.target.write_all(b" ") { return Err(Error::Unspecified("Nested error")); }
				self.state = MessageState::GeneratingField;
				Ok(Field::new(self))
			},
			MessageState::GeneratingField =>
				Err(Error::Unspecified("You must close the previous field before starting a new one"))
		}
	}

	pub fn flush(&mut self) -> io::Result<()> {
		self.target.target.flush()
	}

	pub fn write_field(&mut self, buf: &[u8]) -> Result<(), Error> {
		let mut field = try!{self.next_field()};
		try!{field.write(buf)};
		Ok(())
	}
}

impl<'a, W: Write> Drop for Message<'a, W> {
	fn drop(&mut self) {
		self.target.state = match self.target.target.write_all(&['\n' as u8]) {
			Ok(()) => PushGeneratorState::Initial,
			Err(_err) => PushGeneratorState::Error(Error::Unspecified("Nested error")),
		};
		if self.target.auto_flush() {
			if let Err(_err) = self.target.target.flush() {
				self.target.state = PushGeneratorState::Error(Error::Unspecified("Autoflush failed"));
			}
		}
	}
}

pub struct Field<'a, 'b: 'a + 'b, W: 'b + Write> {
	target: &'a mut Message<'b, W>,
}

impl<'a, 'b, W: Write> Field<'a, 'b, W> {
	fn new(target: &'a mut Message<'b, W>) -> Field<'a, 'b, W> {
		Field {
			target: target,
		}
	}
}

impl<'a, 'b, W: Write> Drop for Field<'a, 'b, W> {
	fn drop(&mut self) {
		self.target.state = MessageState::AfterFirstField;
	}
}

const CURLY_L: u8 = '{' as u8;
const SP: u8 = ' ' as u8;
const CR: u8 = '\r' as u8;
const LF: u8 = '\n' as u8;

fn should_escape(buf: &[u8]) -> bool {
	if buf.len() > 100 {
		true
	} else {
		buf.iter().position(|&x| x == CURLY_L || x == SP || x == CR || x == LF).is_some()
	}
}

impl<'a, 'b, W: Write> Write for Field<'a, 'b, W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		// TODO Handle errors. Should an error put the generator into a failed state?

		let inner_stream = &mut self.target.target.target;
		if should_escape(buf) {
			try!{write!(inner_stream, "{{{}}}", buf.len())}
		}
		try!{inner_stream.write_all(buf)}
		Ok(buf.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		self.target.target.target.flush()
	}
}

#[cfg(test)]
mod test {
	use std::io::Write;
	use pushgenerator::*;

	#[test]
	fn it_works() {
		let mut buffer = Vec::new();

		{
			let mut generator = PushGenerator::new(&mut buffer);

			{
				let mut message = generator.next_message().unwrap();

				{
					let mut field = message.next_field().unwrap();
					field.write_all(b"0").unwrap();
				}

				{
					let mut field = message.next_field().unwrap();
					field.write_all(b"lol").unwrap();
				}
			}
		}

		assert_eq!(String::from("0 lol\n").into_bytes(), buffer);
	}

	#[test]
	fn it_escapes_control_characters() {
		let mut buffer = Vec::new();

		{
			let mut generator = PushGenerator::new(&mut buffer);

			{
				let mut message = generator.next_message().unwrap();

				{
					let mut field = message.next_field().unwrap();
					field.write_all(b" ").unwrap();
					field.write_all(b"\r").unwrap();
					field.write_all(b"\n").unwrap();
					field.write_all(b"{").unwrap();
				}
			}
		}

		assert_eq!(String::from("{1} {1}\r{1}\n{1}{\n").into_bytes(), buffer);
	}

	#[test]
	fn it_has_convenience_functions() {
		let mut buffer = Vec::new();

		{
			let mut generator = PushGenerator::new(&mut buffer);

			{
				let mut message = generator.next_message().unwrap();
				message.write_field(b"apekatt").unwrap();
				message.write_field(b"katter ape").unwrap();
			}

			generator.write_message(&[b"0", b"error", b"success"]).unwrap();
			generator.write_message(&[b"1"]).unwrap();
		}

		assert_eq!(String::from("apekatt {10}katter ape\n0 error success\n1\n").into_bytes(), buffer);
	}
}
