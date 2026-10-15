//! Minimal parser for Valve's VDF (a.k.a. KeyValues) text format.
//!
//! Only the subset Steam writes to `libraryfolders.vdf` and
//! `appmanifest_*.acf` is supported: quoted keys and values, quoted
//! strings with backslash escapes, `{ ... }` nested maps, and `//` line
//! comments. Unquoted bare tokens are accepted and read as strings,
//! because some Steam files omit quotes on simple values.
//!
//! # Example
//!
//! ```
//! use pt_discover::vdf::{self, VdfValue};
//!
//! let value = vdf::parse("\"root\" { \"key\" \"value\" }")?;
//! assert_eq!(
//!     value.get("root").and_then(|v| v.get("key")).and_then(VdfValue::as_str),
//!     Some("value")
//! );
//! # Ok::<(), pt_discover::DiscoverError>(())
//! ```

use std::collections::BTreeMap;

use crate::error::DiscoverError;

/// A parsed VDF node: either a plain string or a nested map.
///
/// The parser is lenient about duplicated keys: the last occurrence
/// wins. Steam never writes duplicate keys to the files parsed here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VdfValue {
    /// A plain string value.
    Str(String),
    /// A nested map of key-value pairs.
    Map(BTreeMap<String, VdfValue>),
}

impl VdfValue {
    /// Returns the node as a string, or `None` if it is a map.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s.as_str()),
            Self::Map(_) => None,
        }
    }

    /// Returns the node as a map, or `None` if it is a string.
    pub fn as_map(&self) -> Option<&BTreeMap<String, VdfValue>> {
        match self {
            Self::Str(_) => None,
            Self::Map(m) => Some(m),
        }
    }

    /// Looks up `key` in a map node; returns `None` on misses and on
    /// string nodes.
    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.as_map().and_then(|m| m.get(key))
    }
}

/// Parses a VDF document into a root map.
///
/// The document must consist of key-value pairs at the top level, as in
/// `"AppState" { ... }` or `"libraryfolders" { ... }`.
pub fn parse(text: &str) -> Result<VdfValue, DiscoverError> {
    let mut parser = Parser::new(text.as_bytes());
    let body = parser.parse_body()?;
    parser.skip_trivia()?;
    if parser.peek().is_some() {
        return Err(parser.error("unexpected trailing content"));
    }
    Ok(VdfValue::Map(body))
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn error(&self, message: &str) -> DiscoverError {
        DiscoverError::Vdf {
            line: self.line,
            column: self.column,
            message: message.to_string(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(byte)
    }

    /// Skips whitespace and `//` line comments.
    fn skip_trivia(&mut self) -> Result<(), DiscoverError> {
        loop {
            match self.peek() {
                Some(byte) if byte.is_ascii_whitespace() => {
                    self.bump();
                }
                Some(b'/') if self.bytes.get(self.pos + 1).copied() == Some(b'/') => {
                    while let Some(byte) = self.peek() {
                        if byte == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn parse_body(&mut self) -> Result<BTreeMap<String, VdfValue>, DiscoverError> {
        let mut map = BTreeMap::new();
        loop {
            self.skip_trivia()?;
            match self.peek() {
                None => return Ok(map),
                Some(b'}') => return Err(self.error("unexpected '}' at top level")),
                _ => {}
            }
            let key = self.parse_token()?;
            self.skip_trivia()?;
            let value = self.parse_value()?;
            map.insert(key, value);
        }
    }

    fn parse_map(&mut self) -> Result<BTreeMap<String, VdfValue>, DiscoverError> {
        let mut map = BTreeMap::new();
        loop {
            self.skip_trivia()?;
            match self.peek() {
                Some(b'}') => {
                    self.bump();
                    return Ok(map);
                }
                None => return Err(self.error("unterminated map: missing '}'")),
                _ => {}
            }
            let key = self.parse_token()?;
            self.skip_trivia()?;
            let value = self.parse_value()?;
            map.insert(key, value);
        }
    }

    /// Parses a key: a quoted string or a bare token.
    fn parse_token(&mut self) -> Result<String, DiscoverError> {
        match self.peek() {
            Some(b'"') => self.parse_quoted(),
            Some(b'{') | Some(b'}') => Err(self.error("expected a key, found a brace")),
            Some(_) => self.parse_bare(),
            None => Err(self.error("expected a key, found end of input")),
        }
    }

    fn parse_value(&mut self) -> Result<VdfValue, DiscoverError> {
        match self.peek() {
            Some(b'{') => {
                self.bump();
                Ok(VdfValue::Map(self.parse_map()?))
            }
            Some(b'"') => Ok(VdfValue::Str(self.parse_quoted()?)),
            Some(b'}') => Err(self.error("expected a value, found '}'")),
            Some(_) => Ok(VdfValue::Str(self.parse_bare()?)),
            None => Err(self.error("expected a value, found end of input")),
        }
    }

    fn parse_quoted(&mut self) -> Result<String, DiscoverError> {
        // Consume the opening quote.
        self.bump();
        let mut out = Vec::new();
        loop {
            let byte = self
                .bump()
                .ok_or_else(|| self.error("unterminated string"))?;
            match byte {
                b'"' => break,
                b'\\' => {
                    let escape = self
                        .bump()
                        .ok_or_else(|| self.error("unterminated escape sequence"))?;
                    match escape {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        other => {
                            return Err(self.error(&format!("unknown escape '\\{}'", other as char)))
                        }
                    }
                }
                other => out.push(other),
            }
        }
        String::from_utf8(out).map_err(|_| self.error("string is not valid UTF-8"))
    }

    fn parse_bare(&mut self) -> Result<String, DiscoverError> {
        let mut out = Vec::new();
        while let Some(byte) = self.peek() {
            if byte.is_ascii_whitespace() || matches!(byte, b'"' | b'{' | b'}') {
                break;
            }
            out.push(byte);
            self.bump();
        }
        if out.is_empty() {
            return Err(self.error("expected a token, found whitespace"));
        }
        String::from_utf8(out).map_err(|_| self.error("token is not valid UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_maps_and_strings() {
        let text = "\"root\" { \"outer\" { \"key\" \"value\" } \"plain\" \"42\" }";
        let value = parse(text).unwrap();
        assert_eq!(
            value
                .get("root")
                .and_then(|v| v.get("outer"))
                .and_then(|v| v.get("key"))
                .and_then(VdfValue::as_str),
            Some("value")
        );
        assert_eq!(
            value
                .get("root")
                .and_then(|v| v.get("plain"))
                .and_then(VdfValue::as_str),
            Some("42")
        );
    }

    #[test]
    fn parses_escapes_and_comments() {
        let text = "// leading comment\n\"key\" \"a\\\"b\\\\c\"\n// trailing comment";
        let value = parse(text).unwrap();
        assert_eq!(value.get("key").and_then(VdfValue::as_str), Some("a\"b\\c"));
    }

    #[test]
    fn parses_bare_tokens() {
        let value = parse("key value").unwrap();
        assert_eq!(value.get("key").and_then(VdfValue::as_str), Some("value"));
    }

    #[test]
    fn reports_line_and_column_on_error() {
        let error = parse("\"key\" \"value\"\ngarbage {").unwrap_err();
        match error {
            DiscoverError::Vdf { line, column, .. } => {
                assert_eq!(line, 2);
                assert!(column > 0);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_unterminated_strings() {
        assert!(parse("\"key\" \"unterminated").is_err());
    }

    #[test]
    fn rejects_unterminated_maps() {
        assert!(parse("\"key\" { \"inner\" \"value\"").is_err());
    }

    #[test]
    fn rejects_trailing_content() {
        assert!(parse("\"key\" \"value\" }").is_err());
    }
}
