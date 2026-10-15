//! Fuzz-style property tests: the VDF and app-manifest parsers must
//! never panic on arbitrary input — malformed data produces errors,
//! never crashes.

use proptest::prelude::*;

use pt_discover::library::parse_library_folders;
use pt_discover::manifest::parse_app_manifest;
use pt_discover::vdf;

proptest! {
    #[test]
    fn vdf_parser_never_panics(text in prop::collection::vec(any::<char>(), 0..=2048)) {
        let text: String = text.into_iter().collect();
        let _ = vdf::parse(&text);
    }
}

proptest! {
    #[test]
    fn library_folders_parser_never_panics(text in prop::collection::vec(any::<char>(), 0..=2048)) {
        let text: String = text.into_iter().collect();
        let _ = parse_library_folders(&text);
    }
}

proptest! {
    #[test]
    fn app_manifest_parser_never_panics(text in prop::collection::vec(any::<char>(), 0..=2048)) {
        let text: String = text.into_iter().collect();
        let _ = parse_app_manifest(&text);
    }
}

proptest! {
    #[test]
    fn vdf_parser_survives_fuzz_shaped_input(
        keys in prop::collection::vec(any::<u8>(), 0..=64),
        braces in prop::collection::vec(any::<bool>(), 0..=64),
    ) {
        // Byte soup interleaved with braces and quotes: a worst case for
        // a tokenizer, built from simple strategies.
        let mut text = String::new();
        for (byte, open) in keys.iter().zip(braces.iter()) {
            text.push('"');
            text.push_str(&format!("{byte:02x}"));
            text.push('"');
            text.push(if *open { '{' } else { '}' });
        }
        let _ = vdf::parse(&text);
    }
}
