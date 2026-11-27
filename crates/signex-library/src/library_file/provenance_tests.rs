//! The optional `[library]` provenance keys (#3112): `license`, `source`,
//! `converted_by`, `requires`.
use super::*;

const HEAD: &str = r#"format = "snxlib/2"
library_id = "0190a000-0000-7000-8000-000000000001"

[library]
name = "Passives"
"#;

const TAIL: &str = r#"
[mode]
kind = "local_git"

[workflow]
mode = "personal"
review_required = false
reviewers_required = 1
auto_lifecycle_promote = "Released"

[users]
default_role = "Designer"
allow_self_signup = false

[[classes]]
key = "resistor"
label = "Resistor"
"#;

const KEYS: &str = r#"license = "CC-BY-SA-4.0"
source = "https://example.com/parts v2.1"
converted_by = "example-import 0.16.0"
requires = [
    "0190a000-0000-7000-8000-0000000000f0",
    "0190a000-0000-7000-8000-0000000000f1",
]
"#;

fn doc(library_keys: &str) -> String {
    format!("{HEAD}{library_keys}{TAIL}")
}

#[test]
fn a_manifest_without_the_keys_writes_none_of_them() {
    let text = doc("");
    let file = LibraryFile::parse(&text).unwrap();
    assert_eq!(file.manifest.library.license, None);
    assert!(file.manifest.library.requires.is_empty());
    assert_eq!(file.write().unwrap(), text);
}

#[test]
fn the_keys_read_typed_and_round_trip_byte_for_byte() {
    let text = doc(KEYS);
    let file = LibraryFile::parse(&text).unwrap();
    let lib = &file.manifest.library;
    assert_eq!(lib.license.as_deref(), Some("CC-BY-SA-4.0"));
    assert_eq!(
        lib.source.as_deref(),
        Some("https://example.com/parts v2.1")
    );
    assert_eq!(lib.converted_by.as_deref(), Some("example-import 0.16.0"));
    assert_eq!(
        lib.required_libraries(),
        [
            Uuid::parse_str("0190a000-0000-7000-8000-0000000000f0").unwrap(),
            Uuid::parse_str("0190a000-0000-7000-8000-0000000000f1").unwrap(),
        ]
    );
    assert!(!file.layout.keeps_text(), "the keys are modelled now");
    assert_eq!(file.write().unwrap(), text);
}

#[test]
fn set_keys_are_written_in_the_documented_order() {
    let mut file = LibraryFile::parse(&doc("")).unwrap();
    let lib = &mut file.manifest.library;
    lib.requires = vec![
        "0190a000-0000-7000-8000-0000000000f0".into(),
        "0190a000-0000-7000-8000-0000000000f1".into(),
    ];
    lib.converted_by = Some("example-import 0.16.0".into());
    lib.source = Some("https://example.com/parts v2.1".into());
    lib.license = Some("CC-BY-SA-4.0".into());
    assert_eq!(file.write().unwrap(), doc(KEYS));
}

/// One required library is written inline, like a one-element `roles`.
#[test]
fn a_single_required_library_is_written_inline() {
    let text = doc("requires = [\"0190a000-0000-7000-8000-0000000000f0\"]\n");
    let file = LibraryFile::parse(&text).unwrap();
    assert_eq!(file.manifest.library.required_libraries().len(), 1);
    assert_eq!(file.write().unwrap(), text);
}

/// A file that carried the keys before this build modelled them, next to a
/// key it still does not model, keeps its bytes.
#[test]
fn the_keys_next_to_an_unknown_key_keep_the_file_bytes() {
    let text = doc(&format!("{KEYS}homepage = \"https://example.com\"\n"));
    let file = LibraryFile::parse(&text).unwrap();
    assert!(file.layout.keeps_text());
    assert_eq!(
        file.manifest.library.license.as_deref(),
        Some("CC-BY-SA-4.0")
    );
    assert_eq!(file.write().unwrap(), text);
}

/// A malformed `requires` entry does not stop the library opening; it is
/// kept as written and reported by the accessor.
#[test]
fn a_malformed_requires_entry_still_opens_and_is_kept() {
    let text = doc("requires = [\"not-a-uuid\"]\n");
    let file = LibraryFile::parse(&text).unwrap();
    let lib = &file.manifest.library;
    assert_eq!(lib.malformed_requires(), ["not-a-uuid"]);
    assert!(lib.required_libraries().is_empty());
    assert_eq!(file.write().unwrap(), text);
}
