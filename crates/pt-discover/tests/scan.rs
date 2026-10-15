//! Integration tests for a full scan over a synthetic Steam
//! installation built in a temporary directory.

use std::path::{Path, PathBuf};

use pt_discover::library::{SteamFlavor, SteamRoot};
use pt_discover::scan::{scan, ScanOptions};

const MANIFEST_TERRARIA: &str = include_str!("fixtures/appmanifest_105600.acf");
const MANIFEST_HOLLOW_KNIGHT: &str = include_str!("fixtures/appmanifest_367520.acf");
const MANIFEST_WITCHER_3: &str = include_str!("fixtures/appmanifest_292030.acf");

/// Builds a synthetic Steam root:
///
/// - `root/steamapps/` holds the Terraria manifest, its compatdata, and
///   a `libraryfolders.vdf` declaring a second library in `lib2/`.
/// - `lib2/steamapps/` holds the Hollow Knight manifest with compatdata
///   and the not-fully-installed Witcher 3 manifest.
/// - `empty/steamapps/` is a root with a malformed manifest to verify
///   that a corrupt file only produces a warning.
fn build_root(base: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let root = base.join("root");
    let lib2 = base.join("lib2");
    let empty = base.join("empty");

    let steamapps = root.join("steamapps");
    std::fs::create_dir_all(steamapps.join("compatdata/105600/pfx")).unwrap();
    std::fs::write(steamapps.join("appmanifest_105600.acf"), MANIFEST_TERRARIA).unwrap();
    std::fs::write(
        steamapps.join("libraryfolders.vdf"),
        format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" {root:?} }} \"1\" {{ \"path\" {lib2:?} }} }}",
            root = root.display().to_string(),
            lib2 = lib2.display().to_string()
        ),
    )
    .unwrap();

    let lib2_steamapps = lib2.join("steamapps");
    std::fs::create_dir_all(lib2_steamapps.join("compatdata/367520/pfx")).unwrap();
    std::fs::write(
        lib2_steamapps.join("appmanifest_367520.acf"),
        MANIFEST_HOLLOW_KNIGHT,
    )
    .unwrap();
    std::fs::write(
        lib2_steamapps.join("appmanifest_292030.acf"),
        MANIFEST_WITCHER_3,
    )
    .unwrap();

    let empty_steamapps = empty.join("steamapps");
    std::fs::create_dir_all(&empty_steamapps).unwrap();
    std::fs::write(
        empty_steamapps.join("appmanifest_broken.acf"),
        "\"AppState\" { \"appid\"",
    )
    .unwrap();

    (root, lib2, empty)
}

fn options(roots: &[PathBuf], flavor: SteamFlavor, include_uninstalled: bool) -> ScanOptions {
    ScanOptions {
        roots: roots
            .iter()
            .map(|path| SteamRoot {
                path: path.clone(),
                flavor,
            })
            .collect(),
        include_uninstalled,
    }
}

#[test]
fn scan_finds_games_across_libraries_with_compatdata() {
    let base = tempfile::tempdir().unwrap();
    let (root, _lib2, empty) = build_root(base.path());

    let result = scan(&options(&[root.clone(), empty], SteamFlavor::Native, false)).unwrap();

    // The broken manifest in `empty` must be skipped without failing.
    let app_ids: Vec<u32> = result.games.iter().map(|g| g.app_id).collect();
    assert_eq!(app_ids, vec![105600, 367520]);

    let terraria = &result.games[0];
    assert_eq!(terraria.name, "Terraria");
    assert_eq!(
        terraria.compatdata.as_ref().map(|c| c.prefix.clone()),
        Some(root.join("steamapps/compatdata/105600"))
    );
    assert_eq!(terraria.install_dir, root.join("steamapps/common/Terraria"));

    let hollow_knight = &result.games[1];
    assert_eq!(hollow_knight.name, "Hollow Knight");
    assert!(hollow_knight.compatdata.is_some());
    // The Witcher 3 (StateFlags "2") must be filtered out.
    assert!(!result.games.iter().any(|g| g.app_id == 292030));
}

#[test]
fn scan_deduplicates_app_ids_across_roots() {
    let base = tempfile::tempdir().unwrap();
    let (root, _lib2, _empty) = build_root(base.path());

    // Same root listed twice: games must not appear twice.
    let result = scan(&options(&[root.clone(), root], SteamFlavor::Native, false)).unwrap();
    assert_eq!(result.games.len(), 2);
}

#[test]
fn scan_can_include_uninstalled_games() {
    let base = tempfile::tempdir().unwrap();
    let (root, _lib2, _empty) = build_root(base.path());

    let result = scan(&options(&[root], SteamFlavor::Native, true)).unwrap();
    let app_ids: Vec<u32> = result.games.iter().map(|g| g.app_id).collect();
    assert_eq!(app_ids, vec![105600, 292030, 367520]);
}

#[test]
fn scan_reports_flatpak_flavor() {
    let base = tempfile::tempdir().unwrap();
    let (root, _lib2, _empty) = build_root(base.path());

    let result = scan(&options(&[root], SteamFlavor::Flatpak, false)).unwrap();
    assert!(result
        .games
        .iter()
        .all(|g| g.flavor == SteamFlavor::Flatpak));
}

#[test]
fn scan_skips_roots_without_steamapps() {
    let base = tempfile::tempdir().unwrap();
    let missing = base.path().join("does-not-exist");

    let result = scan(&options(&[missing], SteamFlavor::Native, false)).unwrap();
    assert!(result.games.is_empty());
    // scanned_at_unix is set to "now", so just sanity-check it.
    assert!(result.scanned_at_unix > 1_500_000_000);
}
