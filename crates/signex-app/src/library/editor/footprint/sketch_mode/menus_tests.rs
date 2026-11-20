//! Modify ▾ rows of mockup row 187 / mockup row 191 on both sketch hosts.

use super::*;
use signex_library::Footprint;

fn en_us() -> I18n {
    I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()))
}

fn footprint_state() -> FootprintEditorState {
    FootprintEditorState::from_footprint(&Footprint::empty("t"))
}

fn board_state() -> FootprintEditorState {
    let mut state = footprint_state();
    let board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    state.board_backdrop = Some(std::sync::Arc::new(
        crate::library::editor::footprint::BoardBackdrop::from_board(&board),
    ));
    state
}

fn strip(label: &str) -> String {
    label.replace(['\u{2068}', '\u{2069}'], "")
}

/// Every Modify row: (label, disabled), headers and separators left out.
fn rows(state: &FootprintEditorState) -> Vec<(String, bool)> {
    modify_entries(state, PathBuf::from("t"), ThemeId::DEFAULT, &en_us())
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownEntry::Item(item) => Some((strip(&item.label), item.disabled)),
            _ => None,
        })
        .collect()
}

fn labels(state: &FootprintEditorState) -> Vec<String> {
    rows(state).into_iter().map(|(label, _)| label).collect()
}

const MODIFY_ROWS: [&str; 10] = [
    "Fillet",
    "Chamfer",
    "Trim",
    "Extend",
    "Mirror",
    "Offset",
    "Linear Pattern",
    "Rectangular Pattern",
    "Circular Pattern",
    "Make Pad from Profile",
];

#[test]
fn modify_offers_chamfer_extend_and_linear_pattern_in_mockup_order() {
    assert_eq!(labels(&footprint_state()), MODIFY_ROWS);
}

#[test]
fn a_board_sketch_has_the_same_modify_rows() {
    assert_eq!(labels(&board_state()), MODIFY_ROWS);
}

#[test]
fn modify_keeps_its_groups() {
    let entries = modify_entries(
        &footprint_state(),
        PathBuf::from("t"),
        ThemeId::DEFAULT,
        &en_us(),
    );
    let shape: Vec<String> = entries
        .iter()
        .map(|entry| match entry {
            DropdownEntry::Header(text) => format!("# {}", strip(text)),
            DropdownEntry::Separator => "--".to_string(),
            DropdownEntry::Item(item) => strip(&item.label),
            _ => "?".to_string(),
        })
        .collect();
    assert_eq!(
        shape,
        [
            "# Modify",
            "Fillet",
            "Chamfer",
            "Trim",
            "Extend",
            "--",
            "# Needs a selection",
            "Mirror",
            "Offset",
            "Linear Pattern",
            "Rectangular Pattern",
            "Circular Pattern",
            "--",
            "Make Pad from Profile",
        ]
    );
}

#[test]
fn chamfer_and_extend_need_no_selection_and_linear_pattern_does() {
    let state = footprint_state();
    let disabled = |name: &str| {
        rows(&state)
            .into_iter()
            .find(|(label, _)| label == name)
            .map(|(_, disabled)| disabled)
            .expect(name)
    };
    assert!(!disabled("Chamfer"));
    assert!(!disabled("Extend"));
    assert!(disabled("Linear Pattern"));

    let mut selected = footprint_state();
    selected.selected_sketch = Some(signex_sketch::id::SketchEntityId::new());
    assert!(
        rows(&selected)
            .into_iter()
            .filter(|(label, _)| label != "Make Pad from Profile")
            .all(|(_, disabled)| !disabled),
        "with a selection every tool row is live"
    );
}

#[test]
fn the_new_rows_arm_their_tools_and_show_the_armed_one() {
    let mut state = footprint_state();
    state.active_tool = SketchTool::Extend;
    let entries = modify_entries(&state, PathBuf::from("t"), ThemeId::DEFAULT, &en_us());
    let armed_by = |name: &str| {
        entries.iter().find_map(|entry| match entry {
            DropdownEntry::Item(item) if strip(&item.label) == name => Some(item),
            _ => None,
        })
    };
    for (name, tool) in [
        ("Chamfer", SketchTool::Chamfer),
        ("Extend", SketchTool::Extend),
        ("Linear Pattern", SketchTool::LinearPattern),
    ] {
        let item = armed_by(name).expect(name);
        assert!(
            matches!(
                &item.on_press,
                Some(LibraryMessage::PrimitiveEditorEvent {
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarSetSketchTool(t)),
                    ..
                }) if *t == tool
            ),
            "{name} arms its tool"
        );
        assert_eq!(item.checked, tool == SketchTool::Extend, "{name}");
    }
}

/// Every Create row's label, headers left out.
fn create_labels(state: &FootprintEditorState) -> Vec<String> {
    create_entries(state, PathBuf::from("t"), ThemeId::DEFAULT, &en_us())
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownEntry::Item(item) => Some(strip(&item.label)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_board_sketch_has_the_same_create_rows_with_slot_and_polygon_after_point() {
    let rows = create_labels(&footprint_state());
    assert_eq!(rows, create_labels(&board_state()));
    let point = rows.iter().position(|r| r == "Point").expect("Point");
    assert_eq!(rows[point + 1..], ["Slot", "Polygon"]);
}

#[test]
fn the_slot_and_polygon_rows_arm_their_tools() {
    let mut state = footprint_state();
    state.active_tool = SketchTool::Polygon;
    let entries = create_entries(&state, PathBuf::from("t"), ThemeId::DEFAULT, &en_us());
    for (name, tool) in [("Slot", SketchTool::Slot), ("Polygon", SketchTool::Polygon)] {
        let item = entries
            .iter()
            .find_map(|entry| match entry {
                DropdownEntry::Item(item) if strip(&item.label) == name => Some(item),
                _ => None,
            })
            .expect(name);
        assert!(
            matches!(
                &item.on_press,
                Some(LibraryMessage::PrimitiveEditorEvent {
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarSetSketchTool(t)),
                    ..
                }) if *t == tool
            ),
            "{name} arms its tool"
        );
        assert_eq!(item.checked, tool == SketchTool::Polygon, "{name}");
    }
}
