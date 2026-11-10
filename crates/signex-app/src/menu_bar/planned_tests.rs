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
const EXPECTED_STUBS: &[&str] = &[
    "Back Annotate Schematics...",
    "Number Schematic Sheets...",
    "Tile Horizontally",
    "Tile Vertically",
];

/// Literal `unavailable` rows that are allowed to stay unexplained.
const EXPECTED_DISABLED: &[&str] = &["No recent projects"];

/// The label literal of each `<callee>(` call in `source` whose first
/// argument is a string literal or `&cmd_label("id", ..)`.
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
        "cmd_item_gated(ctx,cmd(\"zoom_in_at_cursor\",\"ZoomIn\",\"Ctrl+=\"))",
        "cmd_item_gated(ctx,cmd(\"zoom_out_at_cursor\",\"ZoomOut\",\"Ctrl+-\"))",
        "cmd_item_gated(ctx,cmd(\"place_power_symbol\",\"PowerPort\",\"P\"))",
        "item_if(\"Text\",MenuMessage::PlaceText,",
        "item_if(\"NoConnect\",MenuMessage::PlaceNoConnect,",
        "item_if(\"BillofMaterials…\",MenuMessage::ExportBom,",
    ] {
        assert!(source.contains(call), "the row is not wired as `{call}`");
    }
}
