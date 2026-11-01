use crate::bencode::ParserError::{EndOfInput, UnexpectedByte};
use crate::bencode::Value::Int;

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

    fn take(&mut self, len: usize) -> Result<&'a [u8], ParserError> {
        let end = self.pos.checked_add(len).ok_or(EndOfInput)?;
        let bytes = self.buf.get(self.pos..end).ok_or(EndOfInput)?;
        self.pos = end;

        Ok(bytes)
    }

    pub fn parse(&mut self) -> Result<Value, ParserError> {
        if let Ok(byte) = self.peek() {
            return match byte {
                b'i' => self.parse_int(),
                _ if byte.is_ascii_digit() => self.parse_string(),
                _ => Err(UnexpectedByte),
            };
        }
        Err(EndOfInput)
    }

    fn parse_string(&mut self) -> Result<Value, ParserError> {
        let mut digits: Vec<u8> = Vec::new();

        loop {
            let byte = self.peek()?;
            self.pos += 1;

            match byte {
                b':' => break,
                _ if byte.is_ascii_digit() => digits.push(byte),
                _ => return Err(UnexpectedByte),
            }
        }

        if digits.starts_with(b"0") && digits.len() > 1 {
            return Err(UnexpectedByte);
        }

        let strlen: usize = str::from_utf8(&digits)
            .map_err(|_| UnexpectedByte)?
            .parse()
            .map_err(|_| UnexpectedByte)?;

        if let Ok(bytes) = self.take(strlen) {
            return Ok(Value::Bytes(bytes.to_vec()));
        }

        Err(EndOfInput)
    }

    fn parse_int(&mut self) -> Result<Value, ParserError> {
        let mut digits: Vec<u8> = Vec::new();

        loop {
            self.pos += 1;
            let byte = self.peek()?;

            match byte {
                b'e' => {
                    self.pos += 1;
                    break;
                }
                b'-' if digits.is_empty() => digits.push(byte),
                b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                    digits.push(byte)
                }
                _ => return Err(UnexpectedByte),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &[u8]) -> Result<Value, ParserError> {
        Parser::new(input).parse()
    }

    #[test]
    fn parses_positive_int() {
        assert!(matches!(parse(b"i42e"), Ok(Int(42))));
    }

    #[test]
    fn parses_zero() {
        assert!(matches!(parse(b"i0e"), Ok(Int(0))));
    }

    #[test]
    fn parses_negative_int() {
        assert!(matches!(parse(b"i-3e"), Ok(Int(-3))));
    }

    #[test]
    fn parses_i64_bounds() {
        assert!(matches!(parse(b"i9223372036854775807e"), Ok(Int(i64::MAX))));
        assert!(matches!(
            parse(b"i-9223372036854775808e"),
            Ok(Int(i64::MIN))
        ));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(matches!(parse(b""), Err(EndOfInput)));
    }

    #[test]
    fn rejects_unknown_prefix() {
        assert!(matches!(parse(b"x"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_non_digit() {
        assert!(matches!(parse(b"i4a2e"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_double_minus() {
        assert!(matches!(parse(b"i--3e"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_minus_after_digits() {
        assert!(matches!(parse(b"i3-e"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_unterminated_int() {
        assert!(matches!(parse(b"i42"), Err(EndOfInput)));
    }

    #[test]
    fn rejects_empty_int() {
        assert!(parse(b"ie").is_err());
    }

    #[test]
    fn rejects_lone_minus() {
        assert!(parse(b"i-e").is_err());
    }

    #[test]
    fn rejects_negative_zero() {
        assert!(parse(b"i-0e").is_err());
    }

    #[test]
    fn rejects_leading_zero() {
        assert!(parse(b"i03e").is_err());
    }

    #[test]
    fn rejects_overflow() {
        assert!(parse(b"i9223372036854775808e").is_err());
    }

    #[test]
    fn parses_consecutive_ints() {
        let mut parser = Parser::new(b"i1ei2e");
        assert!(matches!(parser.parse(), Ok(Int(1))));
        assert!(matches!(parser.parse(), Ok(Int(2))));
        assert!(matches!(parser.parse(), Err(EndOfInput)));
    }

    #[test]
    fn parses_string() {
        assert!(matches!(parse(b"4:spam"), Ok(Value::Bytes(b)) if b == b"spam"));
    }

    #[test]
    fn parses_multi_digit_length() {
        assert!(matches!(parse(b"11:hello world"), Ok(Value::Bytes(b)) if b == b"hello world"));
    }

    #[test]
    fn parses_empty_string() {
        assert!(matches!(parse(b"0:"), Ok(Value::Bytes(b)) if b.is_empty()));
    }

    #[test]
    fn parses_binary_string() {
        assert!(matches!(parse(b"3:\xff\x00\x80"), Ok(Value::Bytes(b)) if b == [0xff, 0x00, 0x80]));
    }

    #[test]
    fn rejects_short_string() {
        assert!(matches!(parse(b"5:spam"), Err(EndOfInput)));
    }

    #[test]
    fn rejects_missing_colon() {
        assert!(matches!(parse(b"4"), Err(EndOfInput)));
    }

    #[test]
    fn rejects_non_digit_length() {
        assert!(matches!(parse(b"4x:spam"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_string_leading_zero() {
        assert!(parse(b"03:abc").is_err());
    }

    #[test]
    fn rejects_huge_length() {
        assert!(parse(b"18446744073709551615:x").is_err());
    }

    #[test]
    fn parses_consecutive_values() {
        let mut parser = Parser::new(b"4:spami42e0:");
        assert!(matches!(parser.parse(), Ok(Value::Bytes(b)) if b == b"spam"));
        assert!(matches!(parser.parse(), Ok(Int(42))));
        assert!(matches!(parser.parse(), Ok(Value::Bytes(b)) if b.is_empty()));
        assert!(matches!(parser.parse(), Err(EndOfInput)));
    }
}
