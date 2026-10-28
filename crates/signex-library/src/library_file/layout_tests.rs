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
    std::sync::Arc::make_mut(&mut t.rows[row])
        .cells
        .insert("mpn".into(), mpn.into());
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
        auth: "@signex-keychain:a".into(),
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

#[test]
fn renaming_a_table_keeps_its_unknown_keys() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    assert!(file.rename_table("res", "resistors"));
    assert_eq!(
        file.write().unwrap(),
        DOC.replace("[tables.res", "[tables.resistors")
    );
}

#[test]
fn adding_a_table_after_removing_the_only_one_writes_no_empty_tables_header() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    let res = file.tables.remove("res").unwrap();
    file.tables.insert("other".into(), res);
    let out = file.write().unwrap();
    assert!(!out.contains("[tables]"), "{out}");
    assert!(out.contains("[tables.other]"), "{out}");
}

#[test]
fn an_inline_user_keeps_its_unknown_keys_when_edited() {
    let text = DOC.replace(
        "[users.\"a@b.c\"]\ndisplay_name = \"A\"\nroles = [\"Designer\"]\nfuture_user_key = 1\n",
        "[users]\n\"a@b.c\" = { display_name = \"A\", roles = [\"Designer\"], future_user_key = 1 }\n",
    );
    assert_ne!(text, DOC);
    let mut file = LibraryFile::parse(&text).unwrap();
    let user = file.manifest.users.by_email.get_mut("a@b.c").unwrap();
    user.roles.push("Librarian".into());
    let out = file.write().unwrap();
    assert_eq!(
        out,
        text.replace("[\"Designer\"]", "[\"Designer\", \"Librarian\"]")
    );
}

#[test]
fn inline_classes_keep_their_unknown_keys_when_another_class_is_edited() {
    let text = DOC
        .replace(
            "[[classes]]\nlabel = \"Resistor\"\nkey = \"res\"\nfuture_class_key = \"x\"\n\n[[classes]]\nkey = \"cap\"\nlabel = \"Capacitor\"\n\n",
            "",
        )
        .replace(
            "future_root = \"keep\"   # a comment\n",
            "future_root = \"keep\"   # a comment\nclasses = [{ key = \"res\", label = \"Resistor\", future_class_key = \"x\" }, { key = \"cap\", label = \"Capacitor\" }]\n",
        );
    assert!(text.contains("classes = [{"));
    assert!(!text.contains("[[classes]]"));
    let mut file = LibraryFile::parse(&text).unwrap();
    file.manifest.classes[1].label = "Caps".into();
    assert_eq!(file.write().unwrap(), text.replace("Capacitor", "Caps"));
}

#[test]
fn a_refetched_price_drops_the_unknown_keys_of_the_old_fetch() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    let t = file.tables.get_mut("res").unwrap();
    let rec = t.pricing.get_mut("a").unwrap().get_mut("DigiKey").unwrap();
    let rec = rec.get_mut("RC1").unwrap();
    rec.fetched_at += chrono::Duration::days(1);
    let out = file.write().unwrap();
    assert!(!out.contains("future_record_key"), "{out}");
    assert!(!out.contains("future_break_key"), "{out}");
    assert!(out.contains("future_table_scalar"), "{out}");
}

#[test]
fn a_written_text_that_lost_an_unknown_key_is_refused() {
    let read: toml::Table = toml::from_str(DOC).unwrap();
    let file = LibraryFile::parse(DOC).unwrap();
    let base: toml::Table = toml::from_str(&file.write_canonical().unwrap()).unwrap();
    assert_eq!(layout::lost_unknown_key(&read, &base, &base, &read), None);
    let mut lossy = read.clone();
    lossy["library"]
        .as_table_mut()
        .unwrap()
        .remove("future_library_key");
    assert_eq!(
        layout::lost_unknown_key(&read, &base, &base, &lossy).as_deref(),
        Some("library.future_library_key")
    );
    let mut lossy = read.clone();
    lossy["classes"].as_array_mut().unwrap()[0]
        .as_table_mut()
        .unwrap()
        .remove("future_class_key");
    assert_eq!(
        layout::lost_unknown_key(&read, &base, &base, &lossy).as_deref(),
        Some("classes.future_class_key")
    );
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

#[test]
fn renaming_an_inline_class_keeps_its_unknown_keys() {
    let text = CLEAN.replace(
        "classes = []",
        "classes = [{ key = \"a\", label = \"A\", fa = 1 }]",
    );
    let mut file = LibraryFile::parse(&text).unwrap();
    let entry = ClassEntry {
        key: "z".into(),
        label: "Z".into(),
    };
    file.rename_class("a", entry);
    let out = file.write().unwrap();
    assert!(out.contains("fa = 1"), "{out}");
    assert!(out.contains("key = \"z\""), "{out}");
}

#[test]
fn a_breaks_edit_that_would_drop_an_unknown_break_key_is_refused() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    let t = file.tables.get_mut("res").unwrap();
    let rec = t.pricing.get_mut("a").unwrap().get_mut("DigiKey").unwrap();
    rec.get_mut("RC1").unwrap().breaks[0].unit_price = 0.2;
    let err = file.write().unwrap_err();
    assert!(
        matches!(err, LibraryFileError::WrittenDocumentDiffers { .. }),
        "{err:?}"
    );
}

#[test]
fn duplicate_class_keys_still_load_but_a_save_is_refused() {
    let text = CLEAN.replace(
        "classes = []",
        "classes = [{ key = \"a\", label = \"A\" }, { key = \"a\", label = \"B\" }]",
    );
    let file = LibraryFile::parse(&text).expect("an older file with duplicates still opens");
    assert_eq!(file.manifest.classes.len(), 2);
    let err = file.write().unwrap_err();
    assert!(
        matches!(&err, LibraryFileError::DuplicateClassKey { key } if key == "a"),
        "{err:?}"
    );
}

#[test]
fn writing_duplicate_class_keys_names_the_key() {
    let mut file = LibraryFile::parse(CLEAN).unwrap();
    let a = ClassEntry {
        key: "a".into(),
        label: "A".into(),
    };
    file.manifest.classes = vec![a.clone(), a];
    let err = file.write().unwrap_err();
    assert!(
        matches!(&err, LibraryFileError::DuplicateClassKey { key } if key == "a"),
        "{err:?}"
    );
}

#[test]
fn an_unknown_plain_value_under_users_is_kept() {
    let text = CLEAN.replace(
        "allow_self_signup = false
",
        "allow_self_signup = false
future_flag = true
",
    );
    let mut file = LibraryFile::parse(&text).unwrap();
    file.manifest.users.allow_self_signup = true;
    assert_eq!(
        file.write().unwrap(),
        text.replace("allow_self_signup = false", "allow_self_signup = true")
    );
}

#[test]
fn deleting_a_row_with_an_unknown_key_price_record_writes_and_drops_the_record() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    let t = file.tables.get_mut("res").unwrap();
    t.rows.remove(0);
    t.pricing.remove("a");
    let out = file.write().unwrap();
    assert!(!out.contains("future_record_key"), "{out}");
    assert!(out.contains("future_table_scalar"), "{out}");
    assert_eq!(round(&file).tables["res"].rows.len(), 1);
}

#[test]
fn deleting_a_table_with_unknown_keys_drops_them_with_it() {
    let mut file = LibraryFile::parse(DOC).unwrap();
    file.tables.remove("res");
    let out = file.write().unwrap();
    assert!(!out.contains("future_table"), "{out}");
    assert!(out.contains("future_root"), "{out}");
    assert!(round(&file).tables.is_empty());
}

#[test]
fn reordering_breaks_that_hold_an_unknown_key_is_refused() {
    let text = DOC.replace(
        "breaks = [{ qty = 1, unit_price = 0.1, future_break_key = 2 }]",
        "breaks = [{ qty = 1, unit_price = 0.1, future_break_key = 2 }, { qty = 10, unit_price = 0.05 }]",
    );
    assert_ne!(text, DOC);
    let mut file = LibraryFile::parse(&text).unwrap();
    let t = file.tables.get_mut("res").unwrap();
    let rec = t.pricing.get_mut("a").unwrap().get_mut("DigiKey").unwrap();
    rec.get_mut("RC1").unwrap().breaks.reverse();
    let err = file.write().unwrap_err();
    assert!(
        matches!(err, LibraryFileError::WrittenDocumentDiffers { .. }),
        "{err:?}"
    );
}

#[test]
fn a_layout_is_kept_through_the_adapters_row_repair_parse() {
    let mut file = LibraryFile::parse_with(DOC, &crate::tables::repair_short_component_row)
        .expect("parse_with");
    edit_row(&mut file, 1, "2");
    assert_eq!(file.write().unwrap(), DOC.replace("b\t1", "b\t2"));
}

#[test]
fn unknown_keys_in_mode_and_workflow_survive_an_edit() {
    let text = CLEAN
        .replace(
            "kind = \"local_git\"\n",
            "kind = \"local_git\"\nfuture_mode_key = 1\n",
        )
        .replace(
            "reviewers_required = 1\n",
            "reviewers_required = 1\nfuture_workflow_key = \"w\"\n",
        );
    assert!(text.contains("future_mode_key") && text.contains("future_workflow_key"));
    let mut file = LibraryFile::parse(&text).unwrap();
    file.manifest.workflow.review_required = true;
    let out = file.write().unwrap();
    assert_eq!(
        out,
        text.replace("review_required = false", "review_required = true")
    );
}

/// Save cost of a kept layout against the canonical writer on a mid-size
/// library. Run: `cargo test -p signex-library --release -- --ignored --nocapture save_cost`.
#[test]
#[ignore = "timing note, not a gate"]
fn save_cost_of_a_kept_layout() {
    let mut text = String::from(CLEAN.split("[tables.res]").next().unwrap());
    text.push_str("future_root = 1\n");
    for t in 0..5 {
        text.push_str(&format!(
            "\n[tables.t{t}]\nfuture_table_key = 1\ntsv = '''\nrow_id\tmpn\n"
        ));
        for r in 0..2000 {
            text.push_str(&format!("{t}-{r}\tMPN{r}\n"));
        }
        text.push_str("'''\n");
    }
    let mut file = LibraryFile::parse(&text).unwrap();
    std::sync::Arc::make_mut(&mut file.tables.get_mut("t0").unwrap().rows[0])
        .cells
        .insert("mpn".into(), "X".into());
    let time = |f: &dyn Fn()| {
        let start = std::time::Instant::now();
        (0..5).for_each(|_| f());
        start.elapsed() / 5
    };
    let kept = time(&|| drop(file.write().unwrap()));
    // The same content with no unknown keys is what trunk's `write` does.
    let plain = text
        .replace(
            "future_root = 1
",
            "",
        )
        .replace(
            "future_table_key = 1
",
            "",
        );
    let mut plain = LibraryFile::parse(&plain).unwrap();
    std::sync::Arc::make_mut(&mut plain.tables.get_mut("t0").unwrap().rows[0])
        .cells
        .insert("mpn".into(), "X".into());
    let canonical = time(&|| drop(plain.write().unwrap()));
    println!("10000 rows: kept layout {kept:?}, trunk-style write {canonical:?}");
}
