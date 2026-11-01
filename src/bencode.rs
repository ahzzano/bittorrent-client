use crate::bencode::{
    ParserError::{EndOfInput, UnexpectedByte},
    Value::Int,
};

pub enum Value {
    Int(i64),
    Bytes(Vec<u8>),
}

#[non_exhaustive]
#[derive(Debug, PartialEq)]
pub enum ParserError {
    UnexpectedByte,
    EndOfInput,
}

pub struct Parser<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    // Use this when peeking. Avoid using direct look ups
    pub fn peek(&self) -> Result<u8, ParserError> {
        self.buf
            .get(self.pos)
            .copied()
            .ok_or(ParserError::EndOfInput)
    }

    pub fn parse(&mut self) -> Result<Value, ParserError> {
        if let Ok(byte) = self.peek() {
            return match byte {
                b'i' => self.parse_int(),
                _ => Err(UnexpectedByte),
            };
        }
        Err(EndOfInput)
    }

    fn parse_int(&mut self) -> Result<Value, ParserError> {
        let mut digits: Vec<u8> = Vec::new();

        loop {
            self.pos += 1;
            let char = self.peek();

            if let Ok(byte) = char {
                match byte {
                    b'e' => break,
                    b'-' if digits.is_empty() => digits.push(byte),
                    b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                        digits.push(byte)
                    }
                    _ => return Err(UnexpectedByte),
                }
            } else {
                return Err(UnexpectedByte);
            }
        }

        // Spec forbids leading zeros and negative zero; only "0" itself may start with 0.
        if digits.starts_with(b"-0") || (digits.starts_with(b"0") && digits.len() > 1) {
            return Err(UnexpectedByte);
        }

        let num: i64 = str::from_utf8(&digits)
            .map_err(|_| UnexpectedByte)?
            .parse()
            .map_err(|_| UnexpectedByte)?;

        Ok(Int(num))
    }
}
