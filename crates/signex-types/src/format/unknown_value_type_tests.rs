//! Foreign keys keep their TOML value type through load -> save (#1875): a
//! datetime stays a datetime (not the string a newer build then refuses),
//! and integers, floats, booleans, arrays and inline tables stay what they
//! were. Every bag is covered, sheet and board.

use super::tests::{empty_board, empty_sheet};
use super::*;

/// One key of every TOML value type, as it is written inside a table.
const VALUES: &str = r#"when = 1979-05-27T07:32:00Z
when_frac = 1979-05-27T00:32:00.999999-07:00
day = 1979-05-27
clock = 07:32:00
count = 42
negative = -7
ratio = 1.5
whole_float = 3.0
flag = true
name = "text"
list = [1, 2, 3]
dates = [1979-05-27T07:32:00Z, 1980-01-01T00:00:00Z]
point = { x = 1, y = 2.5, at = 1979-05-27T07:32:00Z }
"#;

/// The values `VALUES` parses to, as a TOML table.
fn expected() -> toml::Table {
    toml::from_str(VALUES).unwrap()
}

/// The table at the dotted `path` of `text`.
fn table_at(text: &str, path: &[&str]) -> toml::Table {
    let mut value: toml::Value = toml::from_str(text).unwrap();
    for key in path {
        value = value
            .get(*key)
            .unwrap_or_else(|| panic!("no [{}] in:\n{text}", path.join(".")))
            .clone();
    }
    match value {
        toml::Value::Table(table) => table,
        other => panic!("{path:?} is {other:?}, not a table"),
    }
}

/// Assert `path` of `text` holds exactly `VALUES`, types included.
fn assert_values_at(text: &str, path: &[&str]) {
    // The table also holds this build's own keys; only the foreign ones are compared.
    let found = table_at(text, path);
    for (key, value) in expected() {
        assert_eq!(found.get(&key), Some(&value), "[{}] {key}", path.join("."));
    }
    assert!(
        found["when"].is_datetime(),
        "[{}] lost the datetime type",
        path.join(".")
    );
}

fn sheet_file() -> String {
    let base = SnxSchematic::new(empty_sheet()).write_string().unwrap();
    // The manifest keys go before the first table; the rest are appended.
    let (head, tables) = base
        .split_once("\n[")
        .map_or((base.as_str(), String::new()), |(h, t)| {
            (h, format!("\n[{t}"))
        });
    format!(
        "{head}\n{VALUES}{tables}\n[sheets.widgets]\n{VALUES}\n[extras.sheet]\n{VALUES}\n\
         [extras.sheet.frame]\n{VALUES}\n[extras.mystery]\n{VALUES}"
    )
}

#[test]
fn sheet_bags_keep_every_toml_value_type() {
    let sheet = SnxSchematic::parse(&sheet_file()).unwrap().sheet;
    let out = SnxSchematic::new(sheet.clone()).write_string().unwrap();
    for path in [
        &["sheets", "widgets"][..],
        &["extras", "sheet"],
        &["extras", "sheet", "frame"],
        &["extras", "mystery"],
    ] {
        assert_values_at(&out, path);
    }
    let manifest = table_at(&out, &[]);
    for (key, value) in expected() {
        assert_eq!(manifest[&key], value, "manifest key {key}");
    }
    assert!(manifest["when"].is_datetime());
    // The datetime is written bare, not quoted.
    assert!(out.contains("when = 1979-05-27T07:32:00Z"), "{out}");
    // And the bytes are stable on a second pass.
    let again = SnxSchematic::parse(&out).unwrap().sheet;
    assert_eq!(SnxSchematic::new(again).write_string().unwrap(), out);
}

fn board_file() -> String {
    let base = SnxPcb::new(empty_board()).write_string().unwrap();
    format!(
        "{VALUES}{base}\n[stackup]\n{VALUES}\n[nets]\n{VALUES}\n[extras.board]\n{VALUES}\n\
         [extras.mystery]\n{VALUES}"
    )
}

#[test]
fn board_bags_keep_every_toml_value_type() {
    let board = SnxPcb::parse(&board_file()).unwrap().board;
    let out = SnxPcb::new(board).write_string().unwrap();
    for path in [
        &["stackup"][..],
        &["nets"],
        &["extras", "board"],
        &["extras", "mystery"],
    ] {
        assert_values_at(&out, path);
    }
    let manifest = table_at(&out, &[]);
    assert!(manifest["when"].is_datetime());
    assert_eq!(manifest["point"], expected()["point"]);
}
