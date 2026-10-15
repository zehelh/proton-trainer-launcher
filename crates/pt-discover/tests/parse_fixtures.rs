//! Fixture-based tests for the `libraryfolders.vdf` and app manifest
//! parsers, using realistic file contents checked into the repo.

use std::path::PathBuf;

use pt_discover::library::parse_library_folders;
use pt_discover::manifest::{parse_app_manifest, AppManifest};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture(name))
        .unwrap_or_else(|error| panic!("failed to read fixture {name}: {error}"))
}

#[test]
fn libraryfolders_fixture_lists_both_libraries_in_order() {
    let text = read_fixture("libraryfolders.vdf");
    let paths = parse_library_folders(&text).unwrap();
    assert_eq!(
        paths,
        vec![
            PathBuf::from("/home/tester/.local/share/Steam"),
            PathBuf::from("/mnt/MP600/SteamLibrary"),
        ]
    );
}

fn manifest_fixture(name: &str) -> AppManifest {
    parse_app_manifest(&read_fixture(name))
        .unwrap_or_else(|error| panic!("failed to parse fixture {name}: {error}"))
}

#[test]
fn hollow_knight_manifest_parses() {
    let manifest = manifest_fixture("appmanifest_367520.acf");
    assert_eq!(manifest.app_id, 367520);
    assert_eq!(manifest.name, "Hollow Knight");
    assert_eq!(manifest.installdir, "Hollow Knight");
    assert!(manifest.is_fully_installed());
}

#[test]
fn terraria_manifest_parses() {
    let manifest = manifest_fixture("appmanifest_105600.acf");
    assert_eq!(manifest.app_id, 105600);
    assert_eq!(manifest.name, "Terraria");
    assert!(manifest.is_fully_installed());
}

#[test]
fn stardew_valley_manifest_parses() {
    let manifest = manifest_fixture("appmanifest_413150.acf");
    assert_eq!(manifest.app_id, 413150);
    assert!(manifest.is_fully_installed());
}

#[test]
fn witcher_3_manifest_is_not_fully_installed() {
    // StateFlags "2": needs update, must be filtered out of scans.
    let manifest = manifest_fixture("appmanifest_292030.acf");
    assert_eq!(manifest.app_id, 292030);
    assert!(!manifest.is_fully_installed());
}
