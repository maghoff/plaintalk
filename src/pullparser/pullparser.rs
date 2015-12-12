use std::io::Read;

use super::error::*;
use super::message::*;

pub enum PullParserState {
	Initial,
	Done,
	Error(&'static str),
}

pub struct PullParser<R> {
	inner: R,
	state: PullParserState,
}

impl<R: Read> PullParser<R> {
	pub fn new(inner: R) -> PullParser<R> {
		PullParser {
			inner: inner,
			state: PullParserState::Initial,
		}
	}

	pub fn get_message<'x, 'y: 'x+'y>(&'y mut self) -> Result<Option<Message<'x, R>>, &'static str> {
		match self.state {
			PullParserState::Initial => Ok(Some(Message::new(&mut self.inner, &mut self.state))),
			PullParserState::Done => Ok(None),
			PullParserState::Error(err) => Err(err),
		}
	}

	pub fn read_message(&mut self) -> Result<Option<Vec<Vec<u8>>>, Error> {
		if let Some(mut message) = try!{self.get_message()} {
			let mut buffered_message = Vec::new();
			loop {
				let mut buffered_field = Vec::new();
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
