use std::collections::BTreeMap;

use crate::bencode::ParserError::{EndOfInput, UnexpectedByte};
use crate::bencode::Value::{Int, List};

const MAX_DEPTH: usize = 64;

#[derive(Debug, PartialEq)]
pub enum Value<'a> {
    Int(i64),
    Bytes(&'a [u8]),
    List(Vec<Value<'a>>),
    Dict(BTreeMap<&'a [u8], Value<'a>>),
}

#[non_exhaustive]
#[derive(Debug, PartialEq)]
pub enum ParserError {
    UnexpectedByte,
    EndOfInput,
    DepthError,
}

pub struct Parser<'a> {
    buf: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            buf,
            pos: 0,
            depth: 0,
        }
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

    pub fn parse(&mut self) -> Result<Value<'a>, ParserError> {
        if let Ok(byte) = self.peek() {
            return match byte {
                b'i' => self.parse_int(),
                b'l' => self.parse_list(),
                b'd' => self.parse_dict(),
                _ if byte.is_ascii_digit() => self.parse_string(),
                _ => Err(UnexpectedByte),
            };
        }
        Err(EndOfInput)
    }

    fn parse_dict_items(&mut self) -> Result<BTreeMap<&'a [u8], Value<'a>>, ParserError> {
        let mut tree = BTreeMap::new();
        let mut last: Option<&'a [u8]> = None;

        self.pos += 1;
        loop {
            if self.peek()? == b'e' {
                self.pos += 1;
                return Ok(tree);
            }
            let Value::Bytes(key) = self.parse_string()? else {
                unreachable!()
            };

            if let Some(last_key) = last
                && last_key >= key
            {
                return Err(UnexpectedByte);
            }
            last = Some(key);
            let value = self.parse()?;
            tree.insert(key, value);
        }
    }

    fn parse_dict(&mut self) -> Result<Value<'a>, ParserError> {
        if self.depth >= MAX_DEPTH {
            return Err(ParserError::DepthError);
        }

        self.depth += 1;
        let result = self.parse_dict_items();

        self.depth -= 1;
        result.map(Value::Dict)
    }

    fn parse_list_items(&mut self) -> Result<Vec<Value<'a>>, ParserError> {
        let mut items: Vec<Value<'a>> = Vec::new();
        loop {
            if self.peek()? == b'e' {
                self.pos += 1;
                return Ok(items);
            }
            items.push(self.parse()?);
        }
    }

    fn parse_list(&mut self) -> Result<Value<'a>, ParserError> {
        if self.depth >= MAX_DEPTH {
            return Err(ParserError::DepthError);
        }
        self.depth += 1;
        self.pos += 1;
        let result = self.parse_list_items();

        self.depth -= 1;
        result.map(List)
    }

    fn parse_string(&mut self) -> Result<Value<'a>, ParserError> {
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
            return Ok(Value::Bytes(bytes));
        }

        Err(EndOfInput)
    }

    fn parse_int(&mut self) -> Result<Value<'a>, ParserError> {
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

    fn parse(input: &[u8]) -> Result<Value<'_>, ParserError> {
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

    fn nested(depth: usize) -> Vec<u8> {
        ["l".repeat(depth), "e".repeat(depth)].concat().into_bytes()
    }

    #[test]
    fn parses_empty_list() {
        assert!(matches!(parse(b"le"), Ok(List(items)) if items.is_empty()));
    }

    #[test]
    fn parses_mixed_list() {
        let result = parse(b"li1e4:spami-3ee");
        assert!(matches!(
            result.as_ref().map(|v| match v {
                List(items) => items.as_slice(),
                _ => &[],
            }),
            Ok([Int(1), Value::Bytes(b), Int(-3)]) if b == b"spam"
        ));
    }

    #[test]
    fn parses_nested_list() {
        let result = parse(b"lli1eee");
        let Ok(List(outer)) = result else {
            panic!("expected list");
        };
        assert!(matches!(outer.as_slice(), [List(inner)] if matches!(inner.as_slice(), [Int(1)])));
    }

    #[test]
    fn parses_list_of_lists() {
        let Ok(List(outer)) = parse(b"ll4:spamel3:eggee") else {
            panic!("expected list");
        };
        assert_eq!(outer.len(), 2);
        assert!(
            outer
                .iter()
                .all(|v| matches!(v, List(inner) if inner.len() == 1))
        );
    }

    #[test]
    fn parses_long_flat_list() {
        let input = format!("l{}e", "i1e".repeat(MAX_DEPTH * 2));
        assert!(matches!(parse(input.as_bytes()), Ok(List(items)) if items.len() == MAX_DEPTH * 2));
    }

    #[test]
    fn parses_many_sibling_lists() {
        let input = format!("l{}e", "le".repeat(MAX_DEPTH * 2));
        assert!(matches!(parse(input.as_bytes()), Ok(List(items)) if items.len() == MAX_DEPTH * 2));
    }

    #[test]
    fn parses_consecutive_lists() {
        let input = "li1ee".repeat(MAX_DEPTH * 2);
        let mut parser = Parser::new(input.as_bytes());
        for _ in 0..MAX_DEPTH * 2 {
            assert!(matches!(parser.parse(), Ok(List(_))));
        }
        assert!(matches!(parser.parse(), Err(EndOfInput)));
    }

    #[test]
    fn parses_list_at_max_depth() {
        assert!(parse(&nested(MAX_DEPTH)).is_ok());
    }

    #[test]
    fn rejects_list_past_max_depth() {
        assert!(matches!(
            parse(&nested(MAX_DEPTH + 1)),
            Err(ParserError::DepthError)
        ));
    }

    #[test]
    fn rejects_nesting_bomb() {
        let input = "l".repeat(100_000);
        assert!(matches!(
            parse(input.as_bytes()),
            Err(ParserError::DepthError)
        ));
    }

    #[test]
    fn rejects_unterminated_list() {
        assert!(matches!(parse(b"li1e"), Err(EndOfInput)));
        assert!(matches!(parse(b"l"), Err(EndOfInput)));
    }

    #[test]
    fn rejects_invalid_list_item() {
        assert!(matches!(parse(b"lxe"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_stray_end() {
        assert!(matches!(parse(b"e"), Err(UnexpectedByte)));
    }

    #[test]
    fn restores_depth_after_error() {
        let mut parser = Parser::new(b"llxee");
        assert!(parser.parse().is_err());
        assert_eq!(parser.depth, 0);

        let bomb = nested(MAX_DEPTH + 1);
        let mut parser = Parser::new(&bomb);
        assert!(matches!(parser.parse(), Err(ParserError::DepthError)));
        assert_eq!(parser.depth, 0);
    }

    fn dict<'a>(pairs: Vec<(&'a [u8], Value<'a>)>) -> Value<'a> {
        Value::Dict(pairs.into_iter().collect())
    }

    fn nested_dict(depth: usize) -> Vec<u8> {
        let opens = "d1:k".repeat(depth - 1);
        let closes = "e".repeat(depth - 1);
        [opens, "de".to_string(), closes].concat().into_bytes()
    }

    #[test]
    fn parses_empty_dict() {
        assert_eq!(parse(b"de"), Ok(dict(vec![])));
    }

    #[test]
    fn parses_single_pair_dict() {
        assert_eq!(
            parse(b"d3:cow3:mooe"),
            Ok(dict(vec![(b"cow", Value::Bytes(b"moo"))]))
        );
    }

    #[test]
    fn parses_mixed_value_dict() {
        assert_eq!(
            parse(b"d1:ai1e1:b4:spam1:cli2eee"),
            Ok(dict(vec![
                (b"a", Int(1)),
                (b"b", Value::Bytes(b"spam")),
                (b"c", List(vec![Int(2)])),
            ]))
        );
    }

    #[test]
    fn parses_dict_in_list() {
        assert_eq!(
            parse(b"ld1:ai1eee"),
            Ok(List(vec![dict(vec![(b"a", Int(1))])]))
        );
    }

    #[test]
    fn parses_nested_dict() {
        assert_eq!(
            parse(b"d4:infod6:lengthi7eee"),
            Ok(dict(vec![(b"info", dict(vec![(b"length", Int(7))]))]))
        );
    }

    #[test]
    fn parses_empty_key() {
        assert_eq!(parse(b"d0:i1ee"), Ok(dict(vec![(b"", Int(1))])));
    }

    #[test]
    fn parses_binary_key() {
        assert_eq!(
            parse(b"d2:\xff\x00i1ee"),
            Ok(dict(vec![(b"\xff\x00", Int(1))]))
        );
    }

    #[test]
    fn sorts_keys_by_raw_bytes() {
        // 'Z' (0x5a) sorts before 'a' (0x61), and a prefix sorts before its extension.
        assert!(parse(b"d1:Zi1e1:ai2e2:abi3ee").is_ok());
    }

    #[test]
    fn parses_consecutive_dicts() {
        let mut parser = Parser::new(b"dede");
        assert_eq!(parser.parse(), Ok(dict(vec![])));
        assert_eq!(parser.parse(), Ok(dict(vec![])));
        assert!(matches!(parser.parse(), Err(EndOfInput)));
    }

    #[test]
    fn rejects_unsorted_keys() {
        assert!(matches!(parse(b"d1:bi1e1:ai2ee"), Err(UnexpectedByte)));
        assert!(matches!(parse(b"d1:ai1e1:Zi2ee"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_duplicate_keys() {
        assert!(matches!(parse(b"d1:ai1e1:ai2ee"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_non_string_keys() {
        assert!(matches!(parse(b"di1ei2ee"), Err(UnexpectedByte)));
        assert!(matches!(parse(b"dlei1ee"), Err(UnexpectedByte)));
        assert!(matches!(parse(b"ddei1ee"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_missing_value() {
        assert!(matches!(parse(b"d3:fooe"), Err(UnexpectedByte)));
    }

    #[test]
    fn rejects_unterminated_dict() {
        assert!(matches!(parse(b"d"), Err(EndOfInput)));
        assert!(matches!(parse(b"d3:foo"), Err(EndOfInput)));
        assert!(matches!(parse(b"d3:fooi1e"), Err(EndOfInput)));
    }

    #[test]
    fn parses_dict_at_max_depth() {
        assert!(parse(&nested_dict(MAX_DEPTH)).is_ok());
    }

    #[test]
    fn rejects_dict_past_max_depth() {
        assert!(matches!(
            parse(&nested_dict(MAX_DEPTH + 1)),
            Err(ParserError::DepthError)
        ));
    }

    #[test]
    fn rejects_mixed_nesting_bomb() {
        let input = "ld1:k".repeat(50_000);
        assert!(matches!(
            parse(input.as_bytes()),
            Err(ParserError::DepthError)
        ));
    }

    #[test]
    fn restores_depth_after_dict_error() {
        let mut parser = Parser::new(b"d1:kd1:bi1e1:ai2eee");
        assert!(parser.parse().is_err());
        assert_eq!(parser.depth, 0);

        let bomb = nested_dict(MAX_DEPTH + 1);
        let mut parser = Parser::new(&bomb);
        assert!(matches!(parser.parse(), Err(ParserError::DepthError)));
        assert_eq!(parser.depth, 0);
    }
}
