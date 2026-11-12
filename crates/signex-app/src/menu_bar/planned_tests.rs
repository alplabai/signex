//! Ratchets (#1251) over the menu bar's inert rows.
//!
//! - `stub` rows are features that do not exist yet, greyed with a
//!   "planned" tooltip. The set may only shrink.
//! - `unavailable` called with a literal label is a row that is greyed
//!   for good with NO explanation. Only the informational placeholder
//!   below is allowed; anything else must be wired or become a `stub`.
//!   (`item_if` rows that are only greyed while unavailable are live rows,
//!   not literal call sites of this kind.)
//!
//! A new stub row fails here: wire a real action instead, or — if it must
//! ship planned — add its key to the list and justify it in the PR. A
//! removed row must be removed from the list too.

/// The first string literal after each `stub(` call in `model/menus.rs`: the
/// row's label, or the command id the label is read from.
const EXPECTED_STUBS: &[&str] = &[];

/// `unavailable` rows (by label id) that are allowed to stay unexplained.
/// The label ids are pinned to their English by `disabled_row_label_is_pinned`.
const EXPECTED_DISABLED: &[&str] = &["MENU_FILE_NO_RECENT"];

/// The label key of each `<callee>(` call in `source`: the string literal or
/// `&cmd_label("id", ..)` it opens with, or the name of the `ids::NAME`
/// message it labels the row with (after the leading `ctx,`).
fn call_keys(source: &str, callee: &str) -> Vec<String> {
    let needle = format!("{callee}(");
    let mut keys = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(&needle) {
        let before = &rest[..at];
        let line_start = before.rfind('\n').map_or(0, |n| n + 1);
        let line_before = &before[line_start..];
        let is_comment = line_before.trim_start().starts_with("//");
        let is_ident_tail = line_before.ends_with(|c: char| c.is_alphanumeric() || c == '_');
        rest = &rest[at + needle.len()..];
        if is_comment || is_ident_tail {
            continue;
        }
        let arg = rest.trim_start();
        let arg = arg.strip_prefix("ctx,").map_or(arg, str::trim_start);
        if let Some(named) = arg.strip_prefix("ids::") {
            let end = named
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(named.len());
            keys.push(named[..end].to_string());
            continue;
        }
        let literal_start = if arg.starts_with('"') {
            Some(1)
        } else if arg.starts_with("&cmd_label(") {
            arg.find('"').map(|n| n + 1)
        } else {
            None
        };
        let Some(start) = literal_start else { continue };
        let Some(len) = arg[start..].find('"') else {
            break;
        };
        keys.push(arg[start..start + len].to_string());
    }
    keys
}

fn assert_ratchet(callee: &str, found: &[String], expected: &[&str]) {
    for key in found {
        assert!(
            expected.contains(&key.as_str()),
            "`{key}` is a new `{callee}` menu row. Wire a real action instead of \
             an inert row; if it must ship planned, add it to the list in \
             menu_bar/planned_tests.rs and justify it in the PR."
        );
    }
    assert_eq!(
        found.len(),
        expected.len(),
        "a `{callee}` row was removed: drop it from the list so the ratchet \
         tightens (found {found:?})"
    );
}

#[test]
fn menu_stub_rows_can_only_shrink() {
    let keys = call_keys(include_str!("model/menus.rs"), "stub");
    assert_ratchet("stub", &keys, EXPECTED_STUBS);
}

#[test]
fn unexplained_disabled_rows_can_only_shrink() {
    let keys = call_keys(include_str!("model/menus.rs"), "unavailable");
    assert_ratchet("unavailable", &keys, EXPECTED_DISABLED);
}

#[test]
fn the_scanner_catches_a_new_inert_row() {
    let fake = r#"stub("Brand New Row"),"#;
    assert_eq!(call_keys(fake, "stub"), vec!["Brand New Row"]);
    let fake = "unavailable(\n    \"Greyed\",";
    assert_eq!(call_keys(fake, "unavailable"), vec!["Greyed"]);
    let fake = "unavailable(\n    ctx,\n    ids::MENU_GREYED,";
    assert_eq!(call_keys(fake, "unavailable"), vec!["MENU_GREYED"]);
}

/// The one allowed unexplained row keeps its English: the ratchet above keys
/// it by id, so its text is pinned here.
#[test]
fn disabled_row_label_is_pinned() {
    let i18n = signex_i18n::I18n::en_us_ref();
    assert_eq!(
        i18n.message(signex_i18n::ids::MENU_FILE_NO_RECENT, &[])
            .to_string(),
        "No recent projects"
    );
}

#[test]
fn bill_of_materials_is_not_a_stub() {
    let keys = call_keys(include_str!("model/menus.rs"), "stub");
    assert!(!keys.iter().any(|k| k == "Bill of Materials…"));
}

/// Rows whose feature already works are wired: a row with a command id runs
/// that command (what each one does is pinned by `tests/`), the rest send
/// their own message.
#[test]
fn working_rows_are_wired() {
    let source: String = include_str!("model/menus.rs").split_whitespace().collect();
    for call in [
        "cmd_item_gated(ctx,cmd(\"zoom_in_at_cursor\",\"Ctrl+=\"))",
        "cmd_item_gated(ctx,cmd(\"zoom_out_at_cursor\",\"Ctrl+-\"))",
        "cmd_item_gated(ctx,cmd(\"place_power_symbol\",\"P\"))",
        "item_if(ctx,ids::MENU_PLACE_TEXT,MenuMessage::PlaceText,",
        "item_if(ctx,ids::MENU_PLACE_NO_CONNECT,MenuMessage::PlaceNoConnect,",
        "item_if(ctx,ids::MENU_REPORTS_BOM,MenuMessage::ExportBom,",
    ] {
        assert!(source.contains(call), "the row is not wired as `{call}`");
    }
}

/// The labels of those wired rows, now catalog text instead of literals in
/// the model source, keep their English.
#[test]
fn working_rows_keep_their_labels() {
    use signex_i18n::{I18n, ids};
    let i18n = I18n::en_us_ref();
    for (command, label) in [
        ("zoom_in_at_cursor", "Zoom In"),
        ("zoom_out_at_cursor", "Zoom Out"),
        ("place_power_symbol", "Power Port"),
    ] {
        assert_eq!(super::cmd_label(i18n, command, "<missing>"), label);
    }
    for (id, label) in [
        (ids::MENU_PLACE_TEXT, "Text"),
        (ids::MENU_PLACE_NO_CONNECT, "No Connect"),
        (ids::MENU_REPORTS_BOM, "Bill of Materials…"),
    ] {
        assert_eq!(i18n.message(id, &[]).to_string(), label);
    }
}
