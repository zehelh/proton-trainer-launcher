//! Trainer definitions, validation, and the anti-cheat denylist.
//!
//! Milestone P3 will add: TOML trainer definitions (search pattern, mask,
//! value, freeze), validation with clear English errors, the anti-cheat
//! denylist (attach refusal, never bypass), and a loader that refuses
//! duplicate ids.

#![forbid(unsafe_code)]

/// Returns the crate version, for diagnostics and `--version` output.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
