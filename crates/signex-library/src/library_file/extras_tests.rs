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
    assert_eq!(m["classes"][0]["future_class_key"].as_str(), Some("x"));
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
fn a_file_without_unknown_keys_has_no_extras_and_writes_as_before() {
    let file = LibraryFile::parse(&edited_write(DOC).1).unwrap();
    let mut clean = file.clone();
    clean.manifest.extra.clear();
    clean.tables.get_mut("res").unwrap().extra.clear();
    let out = clean.write().unwrap();
    let back = LibraryFile::parse(&out).unwrap();
    assert!(back.manifest.extra.is_empty());
    assert!(back.tables["res"].extra.is_empty());
    assert_eq!(out, toml_header_then_tables(&clean));
}

/// The pre-fix writer: header via `to_string_pretty` on the manifest alone.
fn toml_header_then_tables(file: &LibraryFile) -> String {
    let head = toml::to_string_pretty(&file.manifest).unwrap();
    assert!(
        file.write()
            .unwrap()
            .starts_with(head.trim_end_matches('\n'))
    );
    file.write().unwrap()
}
