/*!
PlainTalk
=========
This is a library for parsing and generating [PlainTalk][plaintalk].

PlainTalk is a message and field framing protocol. Fields are separated by an
ASCII space, messages are terminated by a newline (CR LF or LF). For a more
complete description of PlainTalk, see [PlainTalk &ndash; Introduction and
Definition][plaintalk].

[plaintalk]: http://magnushoff.com/plaintalk/introduction-and-definition.html
*/

pub mod pushparser;
pub mod pullparser;

pub mod pushgenerator;
