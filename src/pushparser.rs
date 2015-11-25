pub trait PlainTalkParserListener {
	fn start_message(&self);
	fn end_message(&self);

	fn start_field(&self);
	fn field_data(&self, &[u8]);
	fn end_field(&self);
}

pub struct PlainTalkParser<'a> {
	pub listener: &'a PlainTalkParserListener,
	state: for<'b> fn(&mut PlainTalkParser<'a>, &'b [u8]) -> &'b [u8]
}

impl<'a> PlainTalkParser<'a> {
	pub fn new(listener: &'a PlainTalkParserListener) -> PlainTalkParser<'a> {
		PlainTalkParser {
			listener: listener,
			state: PlainTalkParser::expect_message
		}
	}

	pub fn eat_this(&mut self, s: &[u8]) {
		println!("eat_this {:?}", s);
		let mut rest = s;
		while rest.len() != 0 {
			rest = (self.state)(self, rest);
		}
	}

	fn expect_message<'b>(&mut self, s: &'b [u8]) -> &'b [u8] {
		self.listener.start_message();
		self.state = PlainTalkParser::expect_field;
		s
	}

	fn expect_field<'b>(&mut self, s: &'b [u8]) -> &'b [u8] {
		self.listener.start_field();
		self.state = PlainTalkParser::expect_field_data_or_end_of_field;
		s
	}

	fn expect_field_data_or_end_of_field<'b>(&mut self, s: &'b [u8]) -> &'b [u8] {
		match s.iter().position(|&x| x == b' ' || x == b'\n' || x == b'\r') {
			Some(0) => {
				match s[0] {
					b' ' => {
						self.listener.end_field();
						self.listener.start_field();
					},
					b'\n' => {
						self.listener.end_field();
						self.listener.end_message();
						self.state = PlainTalkParser::expect_message;
					},
					b'\r' => {
						self.listener.end_field();
						self.listener.end_message();
						self.state = PlainTalkParser::expect_line_feed;
					},
					_ => panic!("A specific match should be found based on the search above")
				}
				&s[1..]
			},
			Some(n) => {
				self.listener.field_data(&s[0..n]);
				&s[n..]
			},
			None => {
				self.listener.field_data(s);
				&[]
			}
		}
	}

	fn expect_line_feed<'b>(&mut self, _s: &'b [u8]) -> &'b [u8] {
		panic!();
	}
}

#[cfg(test)]
mod test {
	use pushparser::*;

	struct TestPlainTalkParserListener;

	impl PlainTalkParserListener for TestPlainTalkParserListener {
		fn start_message(&self) {
			println!("start_message");
		}

		fn end_message(&self) {
			println!("end_message");
		}

		fn start_field(&self) {
			println!("start_field");
		}

		fn field_data(&self, data:&[u8]) {
			println!("field_data {:?}", data);
		}

		fn end_field(&self) {
			println!("end_field");
		}
	}

	#[test]
	fn it_works() {
		let listener = TestPlainTalkParserListener;
		let mut parser = PlainTalkParser::new(&listener);
		parser.eat_this(b"OMG POP\n");
		parser.eat_this(b"korn");
		parser.eat_this(b"flaeks\n");
	}
}
