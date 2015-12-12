use std::io::{Read, Cursor};
use super::*;

fn buffer_message<R: Read>(message: &mut Message<R>) -> Vec<String> {
	let mut parsed_message = Vec::new();
	while let Ok(Some(mut field)) = message.get_field() {
		let mut buffer = String::new();
		field.read_to_string(&mut buffer).unwrap();
		parsed_message.push(buffer);
	}
	parsed_message
}

fn buffer_all_messages<R: Read>(parser: &mut PullParser<R>) -> Vec<Vec<String>> {
	let mut parsed_messages = Vec::new();
	while let Ok(Some(mut message)) = parser.get_message() {
		parsed_messages.push(buffer_message(&mut message));
	}
	parsed_messages
}

#[test]
fn it_works() {
	let data = Cursor::new(b"0 ape katt lol" as &[u8]);
	let mut parser = PullParser::new(data);

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
	let data = Cursor::new(b"0 ape katt\n1 tam ape\n2 lol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	assert_eq!(
		vec![
			vec!["0", "ape", "katt"],
			vec!["1", "tam", "ape"],
			vec!["2", "lol"],
			vec![""],
		],
		buffer_all_messages(&mut parser)
	);
}

#[test]
fn it_can_parse_escape_sequences() {
	let data = Cursor::new(b"{6}0{1} a{10}pe katt\nlol fie{3}ld 2\n" as &[u8]);
	let mut parser = PullParser::new(data);

	assert_eq!(vec!["0{1} ape katt\nlol", "field 2"], buffer_message(&mut parser.get_message().unwrap().unwrap()));
}

#[test]
fn it_handles_escape_overflow() {
	let data = Cursor::new(b"{9000000000000000000000}blahblah\n" as &[u8]);
	let mut parser = PullParser::new(data);
	let mut buffer = String::new();
	let result = parser.get_message().unwrap().unwrap().get_field().unwrap().unwrap().read_to_string(&mut buffer);
	assert!(result.is_err());
}

#[test]
fn it_understands_crlf() {
	let data = Cursor::new(b"0 ape\r\n1 katt\r\n" as &[u8]);
	let mut parser = PullParser::new(data);

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
	let data = Cursor::new(b"field1 field2\n" as &[u8]);
	let mut parser = PullParser::new(data);

	let mut message = parser.get_message().unwrap().unwrap();

	message.get_field().unwrap().unwrap().ignore_rest().unwrap();

	let mut buffer = String::new();
	message.get_field().unwrap().unwrap().read_to_string(&mut buffer).unwrap();
	assert_eq!("field2", buffer);
}

#[test]
fn it_can_ignore_a_message() {
	let data = Cursor::new(b"msg1 msg1field2\nmsg2 msg2field2\n" as &[u8]);
	let mut parser = PullParser::new(data);

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
	let data = Cursor::new(b"0 protocol lol\n" as &[u8]);
	let mut parser = PullParser::new(data);

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
	let data = Cursor::new(b"protocol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	let mut message = parser.get_message().unwrap().unwrap();

	{
		let buffer = &mut [0u8; 4];
		assert!(message.read_field(buffer).is_err());
	}
}

#[test]
fn message_can_read_field_to_end() {
	let data = Cursor::new(b"0 protocol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	let mut message = parser.get_message().unwrap().unwrap();

	{
		let mut buffer = Vec::new();
		message.read_field_to_end(&mut buffer).unwrap().unwrap();
		assert_eq!(b"0".to_vec(), buffer);
	}

	{
		let mut buffer = Vec::new();
		message.read_field_to_end(&mut buffer).unwrap().unwrap();
		assert_eq!(b"protocol".to_vec(), buffer);
	}

	{
		let mut buffer = Vec::new();
		let result = message.read_field_to_end(&mut buffer).unwrap();
		assert!(result.is_none());
	}
}

#[test]
fn message_can_read_field_as_string() {
	let data = Cursor::new(b"0 protocol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	let mut message = parser.get_message().unwrap().unwrap();

	assert_eq!("0", message.read_field_as_string().unwrap().unwrap());
	assert_eq!("protocol", message.read_field_as_string().unwrap().unwrap());
}

#[test]
fn message_can_read_field_as_slice() {
	let data = Cursor::new(b"0 protocol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	let mut message = parser.get_message().unwrap().unwrap();
	let mut buffer = [0u8;10];

	assert_eq!(b"0", message.read_field_as_slice(&mut buffer).unwrap().unwrap());
	assert_eq!(b"protocol", message.read_field_as_slice(&mut buffer).unwrap().unwrap());
}

#[test]
fn message_can_tell_if_it_is_at_the_end() {
	let data = Cursor::new(b"0 protocol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	let mut message = parser.get_message().unwrap().unwrap();

	assert_eq!(false, message.at_end());
	message.read_field_as_string().unwrap();
	assert_eq!(false, message.at_end());
	message.read_field_as_string().unwrap();
	assert_eq!(true, message.at_end());
}

#[test]
fn parser_can_read_a_message() {
	let data = Cursor::new(b"0 protocol lol\n2 lol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	assert_eq!([b"0".to_vec(), b"protocol".to_vec(), b"lol".to_vec()].to_vec(), parser.read_message().unwrap().unwrap());
	assert_eq!([b"2".to_vec(), b"lol".to_vec()].to_vec(), parser.read_message().unwrap().unwrap());
}

#[test]
fn it_ignores_empty_lines() {
	let data = Cursor::new(b"0 protocol lol\n\n{}\n{0}{00}{000}\n2 lol\n" as &[u8]);
	let mut parser = PullParser::new(data);

	assert_eq!([b"0".to_vec(), b"protocol".to_vec(), b"lol".to_vec()].to_vec(), parser.read_message().unwrap().unwrap());
	assert_eq!([b"2".to_vec(), b"lol".to_vec()].to_vec(), parser.read_message().unwrap().unwrap());
}
