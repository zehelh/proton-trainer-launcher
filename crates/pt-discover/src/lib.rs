//! Steam library discovery for proton-trainer.
//!
//! Milestone P1 will add: `libraryfolders.vdf` parsing, `appmanifest_*.acf`
//! parsing, compatdata prefix detection, and native plus Flatpak Steam
//! root discovery, with a scan-result cache in the XDG config directory.

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
