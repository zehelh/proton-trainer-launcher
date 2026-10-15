//! Property tests for pattern, offset, and value math.

use proptest::prelude::*;

use pt_core::maps::{validate_writable_range, Region};
use pt_trainers::pattern::parse_pattern;
use pt_trainers::trainer::Entry;
use pt_trainers::value::ValueSpec;
use pt_trainers::TrainerError;

// A byte pattern serialized by the test, parsed back, must be
// identical: bytes and wildcards survive a roundtrip for arbitrary
// content.
proptest! {
    #[test]
    fn pattern_roundtrip(bytes in prop::collection::vec(any::<u8>(), 1..=32), wildcards in prop::collection::vec(any::<bool>(), 1..=32)) {
        let tokens: Vec<String> = bytes
            .iter()
            .zip(wildcards.iter().chain(std::iter::repeat(&false)))
            .map(|(byte, wildcard)| {
                if *wildcard { "??".to_string() } else { format!("{byte:02X}") }
            })
            .collect();
        let text = tokens.join(" ");
        let (parsed, mask) = parse_pattern(&text)
            .map_err(|_| TestCaseError::fail("valid pattern failed to parse"))?;
        prop_assert_eq!(parsed.len(), bytes.len());
        for (index, byte) in bytes.iter().enumerate() {
            if mask[index] {
                prop_assert_eq!(parsed[index], *byte);
            }
        }
        for (index, expected) in wildcards.iter().take(bytes.len()).enumerate() {
            prop_assert_eq!(!mask[index], *expected);
        }
    }
}

// Integer values must survive a bytes roundtrip at the documented
// size.
proptest! {
    #[test]
    fn integer_value_roundtrip(value in any::<i32>()) {
        let spec = ValueSpec::I32(value);
        let bytes = spec.to_bytes().map_err(|_| TestCaseError::fail("I32 encode failed"))?;
        prop_assert_eq!(bytes.len(), 4);
        prop_assert_eq!(i32::from_ne_bytes(bytes.try_into().expect("4 bytes")), value);
    }
}

// Float values must survive a bytes roundtrip at the documented size.
proptest! {
    #[test]
    fn float_value_roundtrip(mantissa in -1_000_000i32..1_000_000i32) {
        let value = f64::from(mantissa) / 7.3;
        let spec = ValueSpec::F64(value);
        let bytes = spec.to_bytes().map_err(|_| TestCaseError::fail("F64 encode failed"))?;
        prop_assert_eq!(bytes.len(), 8);
        let decoded = f64::from_ne_bytes(bytes.try_into().expect("8 bytes"));
        prop_assert!(decoded == value || (decoded - value).abs() < f64::EPSILON);
    }
}

// The trainer math must line up with the engine region validation:
// an entry's value range `[value_address, value_address + size)` is
// accepted exactly when it fits inside the region containing the
// pattern match.
proptest! {
    #[test]
    fn entry_value_range_matches_region_validation(
        region_start in 0u64..0x1000,
        region_size in 16u64..0x2000,
        match_offset in 0u64..0x2000,
        entry_offset in 0u32..0x100,
        value_size in 1usize..=8,
    ) {
        let region = Region {
            start: region_start,
            end: region_start + region_size,
            readable: true,
            writable: true,
            executable: false,
            private: true,
            pathname: None,
        };
        let entry = Entry {
            id: "test".to_string(),
            label: "test".to_string(),
            search: pt_trainers::trainer::SearchSpec {
                pattern: "90".to_string(),
            },
            offset: entry_offset,
            value: ValueSpec::Bytes(vec!["AB"; value_size].join(" ")),
            freeze: false,
        };
        let match_address = region.start + match_offset;
        let address = entry.value_address(match_address);
        let end = address + value_size as u64;
        let accepted = validate_writable_range(std::slice::from_ref(&region), 42, address, end).is_ok();
        let expected = address >= region.start && end <= region.end;
        prop_assert_eq!(accepted, expected);
    }
}

// Loading an invalid trainer must fail (never panic) for arbitrary
// byte soup - the parser has no panic paths on hostile input.
proptest! {
    #[test]
    fn parser_never_panics_on_arbitrary_input(text in prop::collection::vec(any::<char>(), 0..=256)) {
        let text: String = text.into_iter().collect();
        let result: Result<pt_trainers::trainer::Trainer, TrainerError> =
            pt_trainers::trainer::parse_trainer(std::path::Path::new("prop.toml"), &text);
        prop_assert!(result.is_err() || result.unwrap().id.chars().count() > 0);
    }
}
