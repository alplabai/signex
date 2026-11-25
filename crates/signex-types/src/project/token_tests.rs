//! DECISIONS 2026-11-25: a project with variant settings writes
//! `snxprj/1.1`, one without them stays `snxprj/1` byte for byte, and a
//! newer minor opens read-only with every save refused.

use super::*;
use crate::format_version::{FormatVersion, MissingToken, NewerMinor};
use serde_json::{Value, json};

fn bare() -> Value {
    json!({"name": "Proj", "dir": "", "schematic_root": null, "pcb_file": null})
}

fn write_raw(dir: &Path, body: &Value) -> PathBuf {
    let path = dir.join("Proj.snxprj");
    std::fs::write(&path, body.to_string()).unwrap();
    path
}

fn token_in(path: &Path) -> Value {
    let saved: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    saved["format"].clone()
}

/// The policy of a build that stops at `snxprj/1` but opens a newer minor
/// read-only.
fn one_zero() -> FormatVersion {
    FormatVersion {
        current: SNXPRJ_FORMAT_V1,
        readable: &[SNXPRJ_FORMAT_V1],
        newer_minor: NewerMinor::ReadOnly,
        missing: MissingToken::ReadAs(SNXPRJ_FORMAT_V1),
    }
}

#[test]
fn no_variant_settings_write_snxprj_1_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let data = parse_project(&path).unwrap();
    assert!(data.variant_settings.is_empty());
    assert_eq!(snxprj_token(&data), SNXPRJ_FORMAT_V1);
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    assert_eq!(token_in(&path), json!(SNXPRJ_FORMAT_V1));
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first);
}

#[test]
fn variant_settings_write_snxprj_1_1_and_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let mut data = parse_project(&path).unwrap();
    data.add_variant("Lite").unwrap();
    data.set_default_variant(Some("Lite")).unwrap();
    assert_eq!(snxprj_token(&data), SNXPRJ_FORMAT_V1_1);
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    assert_eq!(token_in(&path), json!(SNXPRJ_FORMAT_V1_1));

    let back = parse_project(&path).unwrap();
    assert!(!back.is_from_newer_minor(), "this build reads 1.1");
    assert_eq!(back.variant_settings, data.variant_settings);
    write_project(&path, &back).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first, "byte-identical");

    // Clearing the last setting goes back to `snxprj/1`.
    let mut cleared = back;
    cleared.set_default_variant(None).unwrap();
    write_project(&path, &cleared).unwrap();
    assert_eq!(token_in(&path), json!(SNXPRJ_FORMAT_V1));
}

/// An older build reading `snxprj/1.1` opens it read-only and refuses its
/// saves; this build reads it in full.
#[test]
fn snxprj_1_1_opens_read_only_in_a_build_that_knows_only_1() {
    let older = one_zero();
    assert_eq!(
        older.read(Some(SNXPRJ_FORMAT_V1_1)).unwrap(),
        SNXPRJ_FORMAT_V1_1
    );
    assert!(older.is_newer_minor(SNXPRJ_FORMAT_V1_1));
    assert!(older.refuse_save(SNXPRJ_FORMAT_V1_1).is_err());
    assert!(!SNXPRJ_VERSION.is_newer_minor(SNXPRJ_FORMAT_V1_1));
    assert!(SNXPRJ_VERSION.refuse_save(SNXPRJ_FORMAT_V1_1).is_ok());
    assert!(!snxprj_opens_read_only(SNXPRJ_FORMAT_V1));
}

/// A still-newer minor opens here read-only: it loads, every save is
/// refused and the file is left as it was.
#[test]
fn a_newer_minor_opens_read_only_and_refuses_saves() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = bare();
    body["format"] = json!("snxprj/1.2");
    body["future"] = json!(1);
    let path = write_raw(dir.path(), &body);
    let before = std::fs::read(&path).unwrap();
    assert!(snxprj_opens_read_only("snxprj/1.2"));
    let data = parse_project(&path).expect("a newer minor opens");
    assert!(data.is_from_newer_minor());
    assert!(data.refuse_save().is_err());
    assert!(encode_project(&data).is_err(), "no bytes for a save");
    match write_project(&path, &data) {
        Err(ProjectError::ReadOnly { source, .. }) => {
            assert!(source.to_string().contains("snxprj/1.2"), "{source}");
        }
        other => panic!("expected ReadOnly, got {other:?}"),
    }
    assert_eq!(std::fs::read(&path).unwrap(), before, "untouched");
}

/// A missing token still reads as `snxprj/1`, editable; a newer major is
/// still refused.
#[test]
fn a_missing_token_reads_as_1_and_a_newer_major_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let data = parse_project(&path).unwrap();
    assert!(!data.is_from_newer_minor());
    assert!(data.refuse_save().is_ok());
    assert_eq!(SNXPRJ_VERSION.read(None).unwrap(), SNXPRJ_FORMAT_V1);

    let mut body = bare();
    body["format"] = json!("snxprj/2");
    let path = write_raw(dir.path(), &body);
    assert!(matches!(
        parse_project(&path),
        Err(ProjectError::UnsupportedFormat { .. })
    ));
}
