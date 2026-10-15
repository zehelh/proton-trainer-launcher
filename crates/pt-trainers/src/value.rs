//! Typed trainer values and their byte encodings.

use crate::pattern::parse_hex_bytes;

/// The value a trainer entry writes, in native endianness.
///
/// TOML form: `value = { type = "i32", data = 99 }` — integers, floats,
/// or raw bytes as hex: `{ type = "bytes", data = "DE AD BE EF" }`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
pub enum ValueSpec {
    /// A 32-bit signed integer.
    I32(i32),
    /// A 64-bit signed integer.
    I64(i64),
    /// A 32-bit float.
    F32(f32),
    /// A 64-bit float.
    F64(f64),
    /// Raw bytes as a hex string (no wildcards), e.g. `"DE AD BE EF"`.
    Bytes(String),
}

impl ValueSpec {
    /// The number of bytes this value occupies in the target's memory.
    ///
    /// # Errors
    ///
    /// Returns an error for a `bytes` value whose hex payload is
    /// invalid.
    pub fn size_bytes(&self) -> Result<usize, String> {
        match self {
            Self::I32(_) | Self::F32(_) => Ok(4),
            Self::I64(_) | Self::F64(_) => Ok(8),
            Self::Bytes(data) => Ok(parse_hex_bytes(data)?.len()),
        }
    }

    /// Encodes the value as bytes in the target's native endianness.
    ///
    /// # Errors
    ///
    /// Returns an error for a `bytes` value whose hex payload is
    /// invalid.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::I32(value) => Ok(value.to_ne_bytes().to_vec()),
            Self::I64(value) => Ok(value.to_ne_bytes().to_vec()),
            Self::F32(value) => Ok(value.to_ne_bytes().to_vec()),
            Self::F64(value) => Ok(value.to_ne_bytes().to_vec()),
            Self::Bytes(data) => parse_hex_bytes(data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_sizes_and_encodings() {
        assert_eq!(ValueSpec::I32(99).size_bytes().unwrap(), 4);
        assert_eq!(
            ValueSpec::I32(99).to_bytes().unwrap(),
            99i32.to_ne_bytes().to_vec()
        );
        assert_eq!(ValueSpec::I64(-1).size_bytes().unwrap(), 8);
        assert_eq!(
            ValueSpec::I64(-1).to_bytes().unwrap(),
            (-1i64).to_ne_bytes().to_vec()
        );
    }

    #[test]
    fn float_sizes_and_encodings() {
        assert_eq!(ValueSpec::F32(1.5).size_bytes().unwrap(), 4);
        assert_eq!(
            ValueSpec::F32(1.5).to_bytes().unwrap(),
            1.5f32.to_ne_bytes().to_vec()
        );
        assert_eq!(ValueSpec::F64(1.5).size_bytes().unwrap(), 8);
        assert_eq!(
            ValueSpec::F64(1.5).to_bytes().unwrap(),
            1.5f64.to_ne_bytes().to_vec()
        );
    }

    #[test]
    fn bytes_values() {
        assert_eq!(
            ValueSpec::Bytes("DE AD".to_string()).to_bytes().unwrap(),
            vec![0xde, 0xad]
        );
        assert!(ValueSpec::Bytes("DE ??".to_string()).to_bytes().is_err());
        assert!(ValueSpec::Bytes("XYZ".to_string()).to_bytes().is_err());
    }

    #[test]
    fn toml_roundtrip() {
        let toml_text = "value = { type = \"i32\", data = 99 }";
        #[derive(serde::Deserialize)]
        struct Wrapper {
            value: ValueSpec,
        }
        let wrapper: Wrapper = toml::from_str(toml_text).unwrap();
        assert_eq!(wrapper.value, ValueSpec::I32(99));

        let toml_text = "value = { type = \"bytes\", data = \"DE AD\" }";
        let wrapper: Wrapper = toml::from_str(toml_text).unwrap();
        assert_eq!(wrapper.value.to_bytes().unwrap(), vec![0xde, 0xad]);
    }
}
