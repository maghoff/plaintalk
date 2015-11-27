use std::io::{self, Write};

#[derive(Debug, Clone)]
pub enum Error {
	Unspecified(&'static str),
}

enum PushGeneratorState {
	Initial,
	GeneratingMessage,
	Error(Error),
}

pub struct PushGenerator<'a> {
	target: &'a mut Write,
	state: PushGeneratorState,
	auto_flush: bool,
}

impl<'a> PushGenerator<'a> {
	pub fn new<'x>(target: &'x mut Write) -> PushGenerator<'x> {
		PushGenerator {
			target: target,
			state: PushGeneratorState::Initial,
			auto_flush: true,
		}
	}

	pub fn next_message<'x, 'y: 'x+'y>(&'y mut self) -> Result<Message<'x, 'a>, Error> {
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
}

enum MessageState {
	BeforeFirstField,
	AfterFirstField,
	GeneratingField,
}

pub struct Message<'a, 'b: 'a+'b> {
	target: &'a mut PushGenerator<'b>,
	state: MessageState,
}

impl<'a, 'b> Message<'a, 'b> {
	fn new<'x, 'y: 'x+'y>(target: &'x mut PushGenerator<'y>) -> Message<'x, 'y> {
		Message {
			target: target,
			state: MessageState::BeforeFirstField,
		}
	}

	pub fn next_field<'x, 'y: 'x+'y>(&'y mut self) -> Result<Field<'x, 'a, 'b>, Error> {
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
}

impl<'a, 'b> Drop for Message<'a, 'b> {
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

pub struct Field<'a, 'b: 'a + 'b, 'c: 'b + 'c> {
	target: &'a mut Message<'b, 'c>,
}

impl<'a, 'b, 'c> Field<'a, 'b, 'c> {
	fn new<'x, 'y, 'z>(target: &'x mut Message<'y, 'z>) -> Field<'x, 'y, 'z> {
		Field {
			target: target,
		}
	}
}

impl<'a, 'b, 'c> Drop for Field<'a, 'b, 'c> {
	fn drop(&mut self) {
		self.target.state = MessageState::AfterFirstField;
	}
}

impl<'a, 'b, 'c> Write for Field<'a, 'b, 'c> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.target.target.target.write(buf)
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
		let mut buffer = Vec::<u8>::new();

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
}
