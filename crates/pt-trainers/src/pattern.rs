//! Hex byte-pattern parsing shared by trainer definitions.
//!
//! Patterns are whitespace-separated two-digit hex bytes where `??`
//! marks a wildcard: `"89 45 ?? 8B 06"`.

use pt_core::Pattern;

/// Parses a pattern string into (bytes, mask) where `mask[i] == false`
/// marks a wildcard.
///
/// # Errors
///
/// Returns an error naming the offending token for empty input,
/// malformed tokens, or tokens that are not exactly two hex digits or
/// `??`.
pub fn parse_pattern(text: &str) -> Result<(Vec<u8>, Vec<bool>), String> {
    let mut bytes = Vec::new();
    let mut mask = Vec::new();
    for token in text.split_whitespace() {
        if token == "??" {
            bytes.push(0);
            mask.push(false);
            continue;
        }
        if token.len() != 2 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "invalid token {token:?}: expected two hex digits or \"??\""
            ));
        }
        let byte = u8::from_str_radix(token, 16)
            .map_err(|error| format!("invalid token {token:?}: {error}"))?;
        bytes.push(byte);
        mask.push(true);
    }
    if bytes.is_empty() {
        return Err("the pattern is empty".to_string());
    }
    Ok((bytes, mask))
}

/// Parses a plain hex byte string (no wildcards), e.g. `"DE AD BE EF"`.
///
/// # Errors
///
/// Returns an error naming the offending token.
pub fn parse_hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    let (bytes, mask) = parse_pattern(text)?;
    if mask.iter().any(|must_match| !must_match) {
        return Err("wildcards (\"??\") are not allowed in byte values".to_string());
    }
    Ok(bytes)
}

/// Converts a pattern string into a [`pt_core::Pattern`].
///
/// # Errors
///
/// Returns an error naming the offending token (see
/// [`parse_pattern`]) or a [`TrainerError::Schema`] wrapped by the
/// caller through its message.
pub fn to_core_pattern(text: &str) -> Result<Pattern, String> {
    let (bytes, mask) = parse_pattern(text)?;
    Pattern::new(bytes, mask).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bytes_and_wildcards() {
        let (bytes, mask) = parse_pattern("89 45 ?? 8B 06").unwrap();
        assert_eq!(bytes, vec![0x89, 0x45, 0x00, 0x8b, 0x06]);
        assert_eq!(mask, vec![true, true, false, true, true]);
    }

    #[test]
    fn lower_case_hex_is_accepted() {
        let (bytes, _) = parse_pattern("ab cd ef").unwrap();
        assert_eq!(bytes, vec![0xab, 0xcd, 0xef]);
    }

    #[test]
    fn rejects_empty_and_malformed_patterns() {
        assert!(parse_pattern("").is_err());
        assert!(parse_pattern("   ").is_err());
        let error = parse_pattern("89 4 ??").unwrap_err();
        assert!(error.contains("4"));
        let error = parse_pattern("89 zz").unwrap_err();
        assert!(error.contains("zz"));
    }

    #[test]
    fn hex_values_reject_wildcards() {
        assert!(parse_hex_bytes("DE AD").is_ok());
        assert!(parse_hex_bytes("DE ??").is_err());
    }

    #[test]
    fn converts_to_core_pattern() {
        let pattern = to_core_pattern("89 ??").unwrap();
        assert_eq!(pattern.len(), 2);
        assert_eq!(pattern.mask, vec![true, false]);
    }
}
