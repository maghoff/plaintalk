pub mod plaintalk {
	pub trait PlainTalkParserListener {
		fn start_message(&self);
		fn end_message(&self);
	}

	pub struct PlainTalkParser<'a> {
		pub listener: &'a PlainTalkParserListener
	}

	impl<'a> PlainTalkParser<'a> {
		pub fn eat_this(&self, s: &str) {
			println!("eat_this {}", s);
			self.listener.start_message();
			self.listener.end_message();
		}
	}
}


#[cfg(test)]
mod test {
	use plaintalk;

	struct TestPlainTalkParserListener;

	impl plaintalk::PlainTalkParserListener for TestPlainTalkParserListener {
		fn start_message(&self) {
			println!("start_message");
		}

		fn end_message(&self) {
			println!("end_message");
		}
	}


	#[test]
	fn it_works() {
		let listener = TestPlainTalkParserListener;
		let parser = plaintalk::PlainTalkParser { listener: &listener };
		parser.eat_this("OMG POP");
	}
}
