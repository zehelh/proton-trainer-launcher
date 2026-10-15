//! Fixture-based tests: the trainer examples shipped in the repo's
//! `trainers/` directory must load and validate.

use std::path::PathBuf;

use pt_trainers::loader::load_dir;

#[test]
fn shipped_example_trainers_load() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../trainers")
        .canonicalize()
        .expect("the repo trainers/ directory must exist");

    let trainers = load_dir(&dir).expect("the shipped trainers must be valid");
    let ids: Vec<&str> = trainers.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"pt-dummy"), "missing pt-dummy, got {ids:?}");
    assert!(
        ids.contains(&"hollow-knight"),
        "missing hollow-knight, got {ids:?}"
    );
    assert!(
        ids.contains(&"stardew-valley"),
        "missing stardew-valley, got {ids:?}"
    );

    // The dummy trainer's pattern must convert to a pt-core pattern of
    // the expected length ("PTDUMMY-PATTERN-v1" is 18 bytes).
    let dummy = trainers
        .iter()
        .find(|t| t.id == "pt-dummy")
        .expect("pt-dummy");
    let pattern = dummy.entries[0].search.to_pattern().expect("valid pattern");
    assert_eq!(pattern.len(), 18);
    assert_eq!(dummy.entries[0].offset, 18);
    assert_eq!(dummy.entries[0].value, pt_trainers::ValueSpec::I32(99));
}
