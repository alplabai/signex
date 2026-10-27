//! Unknown keys in a `.snxlib` survive parse -> edit -> write (#1435).

use super::*;

const DOC: &str = r#"format = "snxlib/1"
library_id = "0190a000-0000-7000-8000-000000000001"
future_root = "keep"

[future_tree]
depth = 1

[future_tree.inner]
deeper = [1, 2, 3]

[library]
name = "L"
future_library_key = true

[[classes]]
key = "res"
label = "Resistor"
future_class_key = "x"

[tables.res]
tsv = '''
row_id	mpn
a	1
'''
future_table_scalar = 7

[tables.res.future_table_tree]
note = "hi"
"#;

fn edited_write(text: &str) -> (LibraryFile, String) {
    let mut file = LibraryFile::parse(text).expect("parse");
    let t = file.tables.get_mut("res").unwrap();
    t.rows[0].cells.insert("mpn".into(), "2".into());
    let out = file.write().expect("write");
    (file, out)
}

#[test]
fn unknown_keys_survive_a_row_edit() {
    let (_, out) = edited_write(DOC);
    let back = LibraryFile::parse(&out).unwrap();
    let m = &back.manifest.extra;
    assert_eq!(m["future_root"].as_str(), Some("keep"));
    assert_eq!(
        m["future_tree"]["inner"]["deeper"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(m["library"]["future_library_key"].as_bool(), Some(true));
    assert_eq!(m["classes"]["res"]["future_class_key"].as_str(), Some("x"));
    let t = &back.tables["res"].extra;
    assert_eq!(t["future_table_scalar"].as_integer(), Some(7));
    assert_eq!(t["future_table_tree"]["note"].as_str(), Some("hi"));
    assert_eq!(back.tables["res"].rows[0].cells["mpn"], "2");
}

#[test]
fn rewriting_unchanged_content_is_a_fixed_point() {
    let (_, once) = edited_write(DOC);
    let twice = LibraryFile::parse(&once).unwrap().write().unwrap();
    assert_eq!(once, twice);
}

#[test]
fn removing_a_class_never_moves_its_unknown_keys_onto_another() {
    let text = DOC.replace(
        "[tables.res]",
        "[[classes]]
key = \"cap\"
label = \"Capacitor\"

[tables.res]",
    );
    let mut file = LibraryFile::parse(&text).unwrap();
    file.manifest.classes.retain(|c| c.key != "res");
    let back = LibraryFile::parse(&file.write().unwrap()).unwrap();
    assert_eq!(back.manifest.classes.len(), 1);
    assert_eq!(back.manifest.classes[0].key, "cap");
    assert!(back.manifest.extra.get("classes").is_none());
    let mut reordered = LibraryFile::parse(&text).unwrap();
    reordered.manifest.classes.reverse();
    let back = LibraryFile::parse(&reordered.write().unwrap()).unwrap();
    assert_eq!(
        back.manifest.extra["classes"]["res"]["future_class_key"].as_str(),
        Some("x")
    );
}

#[test]
fn a_file_without_unknown_keys_writes_exactly_its_committed_bytes() {
    let text = CLEAN;
    let file = LibraryFile::parse(text).unwrap();
    assert!(file.manifest.extra.is_empty());
    assert_eq!(file.write().unwrap(), text);
}

#[test]
fn first_save_of_a_file_with_unknown_keys_writes_canonical_bytes() {
    let (_, out) = edited_write(DOC);
    assert_eq!(out, EXPECTED_AFTER_FIRST_SAVE);
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

const EXPECTED_AFTER_FIRST_SAVE: &str = "format = \"snxlib/1\"
future_root = \"keep\"
library_id = \"0190a000-0000-7000-8000-000000000001\"

[[classes]]
future_class_key = \"x\"
key = \"res\"
label = \"Resistor\"

[future_tree]
depth = 1

[future_tree.inner]
deeper = [
    1,
    2,
    3,
]

[library]
future_library_key = true
name = \"L\"

[mode]
kind = \"local_git\"

[users]
allow_self_signup = false
default_role = \"\"

[workflow]
auto_lifecycle_promote = \"Released\"
mode = \"personal\"
review_required = false
reviewers_required = 1

[tables.res]
tsv = '''
row_id	mpn
a	2
'''
future_table_scalar = 7

[tables.res.future_table_tree]
note = \"hi\"
";
