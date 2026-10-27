//! A `.snxlib` holding keys this build does not model survives parse -> edit
//! -> write with its unknown keys, key order and untouched rows as they were
//! (#1435).

use super::*;
use crate::manifest::{LibraryMode, UserEntry};

/// Not the canonical layout on purpose: keys out of order, a comment, unknown
/// keys at every level the reader knows about.
const DOC: &str = r#"library_id = "0190a000-0000-7000-8000-000000000001"
format = "snxlib/1"
future_root = "keep"   # a comment

[library]
name = "L"
future_library_key = true

[future_tree.inner]
deeper = [1, 2, 3]

[users."a@b.c"]
display_name = "A"
roles = ["Designer"]
future_user_key = 1

[[classes]]
label = "Resistor"
key = "res"
future_class_key = "x"

[[classes]]
key = "cap"
label = "Capacitor"

[tables.res]
future_table_scalar = 7
tsv = '''
row_id	mpn
a	1
b	1
'''

[tables.res.pricing.a.DigiKey.RC1]
sku = "RC1"
currency = "USD"
fetched_at = "2026-01-02T03:04:05Z"
breaks = [{ qty = 1, unit_price = 0.1, future_break_key = 2 }]
future_record_key = "r"

[tables.res.future_table_tree]
note = "hi"
"#;

fn edit_row(file: &mut LibraryFile, row: usize, mpn: &str) {
    let t = file.tables.get_mut("res").unwrap();
    t.rows[row].cells.insert("mpn".into(), mpn.into());
}

fn round(file: &LibraryFile) -> LibraryFile {
    LibraryFile::parse(&file.write().expect("write")).expect("reparse")
}

#[test]
fn a_row_edit_changes_only_that_row() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    edit_row(&mut file, 1, "2");
    assert_eq!(file.write().unwrap(), DOC.replace("b\t1", "b\t2"));
}

#[test]
fn writing_unedited_content_returns_the_read_bytes() {
    assert_eq!(LibraryFile::parse(DOC).unwrap().write().unwrap(), DOC);
}

#[test]
fn a_header_edit_keeps_the_other_keys_where_they_were() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    file.manifest.library.name = "M".into();
    file.manifest.workflow.review_required = true;
    let out = file.write().unwrap();
    let want = DOC.replace("name = \"L\"", "name = \"M\"");
    assert!(out.starts_with(&want), "{out}");
    assert!(out.contains("review_required = true"), "{out}");
}

#[test]
fn a_price_refresh_keeps_the_unknown_keys_of_the_record() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    let t = file.tables.get_mut("res").unwrap();
    t.pricing
        .get_mut("a")
        .unwrap()
        .get_mut("DigiKey")
        .unwrap()
        .get_mut("RC1")
        .unwrap()
        .currency = "EUR".into();
    let out = file.write().unwrap();
    assert_eq!(out, DOC.replace("currency = \"USD\"", "currency = \"EUR\""));
}

#[test]
fn removing_a_class_never_moves_its_unknown_keys_onto_another() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    file.manifest.classes.retain(|c| c.key != "res");
    let out = file.write().unwrap();
    assert!(!out.contains("future_class_key"), "{out}");
    assert_eq!(LibraryFile::parse(&out).unwrap().manifest.classes.len(), 1);

    let mut reordered = LibraryFile::parse(DOC).unwrap();
    reordered.manifest.classes.reverse();
    let back = round(&reordered);
    assert_eq!(back.manifest.classes, reordered.manifest.classes);
    let out = reordered.write().unwrap();
    let res = out.find("key = \"res\"").unwrap();
    assert!(out[res..].contains("future_class_key = \"x\""), "{out}");
}

#[test]
fn renaming_a_class_keeps_its_unknown_keys() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    let entry = ClassEntry {
        key: "resistor".into(),
        label: "Resistor".into(),
    };
    file.rename_class("res", entry);
    assert_eq!(
        file.write().unwrap(),
        DOC.replace("key = \"res\"", "key = \"resistor\"")
    );
}

#[test]
fn a_class_removed_by_a_save_does_not_come_back_with_its_keys() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    let res = file.manifest.classes.remove(0);
    file.write_and_rebase().unwrap();
    file.manifest.classes.push(res);
    let out = file.write().unwrap();
    assert!(out.contains("key = \"res\""), "{out}");
    assert!(!out.contains("future_class_key"), "{out}");
}

#[test]
fn removing_a_user_removes_its_unknown_keys_and_later_saves_still_work() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    file.manifest.users.by_email.clear();
    let out = file.write_and_rebase().unwrap();
    assert!(!out.contains("a@b.c"), "{out}");
    file.manifest.users.by_email.insert(
        "d@e.f".into(),
        UserEntry {
            display_name: "D".into(),
            roles: vec![],
        },
    );
    assert_eq!(round(&file).manifest.users.by_email.len(), 1);
}

#[test]
fn a_known_key_takes_over_an_unknown_one_of_the_same_name() {
    let text = DOC.replace(
        "[library]",
        "[mode]\nkind = \"local_git\"\nurl = \"stray\"\n\n[library]",
    );
    let mut file = LibraryFile::parse(&text).unwrap();
    file.manifest.mode = LibraryMode::Database {
        url: "postgres://h/db".into(),
        auth: "a".into(),
    };
    assert_eq!(round(&file).manifest.mode, file.manifest.mode);
}

#[test]
fn the_committed_stock_library_writes_its_own_bytes() {
    let text = include_str!("../../../../assets/stock-library/Stock.snxlib");
    let file = LibraryFile::parse(text).unwrap();
    assert_eq!(format!("{:?}", file.layout), "SourceLayout(none)");
    assert_eq!(file.write().unwrap(), text);
}

#[test]
fn a_file_without_unknown_keys_writes_exactly_its_committed_bytes() {
    let file = LibraryFile::parse(CLEAN).unwrap();
    assert_eq!(format!("{:?}", file.layout), "SourceLayout(none)");
    assert_eq!(file.write().unwrap(), CLEAN);
}

const CLEAN: &str = "format = \"snxlib/1\"
library_id = \"0190a000-0000-7000-8000-000000000001\"
classes = []

[library]
name = \"L\"

[mode]
kind = \"local_git\"

[workflow]
mode = \"personal\"
review_required = false
reviewers_required = 1
auto_lifecycle_promote = \"Released\"

[users]
default_role = \"\"
allow_self_signup = false

[tables.res]
tsv = '''
row_id	mpn
a	1
'''
";
