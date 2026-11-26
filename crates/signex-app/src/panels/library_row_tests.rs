//! Mockup row 127: the Properties of a selected library row.

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};

const WINDOW: iced::Size = iced::Size::new(320.0, 720.0);

fn detail() -> LibraryRowDetail {
    let i18n = I18n::en_us();
    LibraryRowDetail {
        library_path: std::path::PathBuf::from("/libs/passives/passives.snxlib"),
        library: "passives".into(),
        table: "Resistors".into(),
        row_id: uuid::Uuid::nil(),
        position: Some(18),
        internal_pn: "R-0402-4K7-1%".into(),
        class: "resistor".into(),
        lifecycle: "Released".into(),
        lifecycle_tone: PillTone::Ok,
        version: "3".into(),
        mpn: "RC0402FR-074K7L".into(),
        manufacturer: "Yageo".into(),
        updated: "2026-11-12 09:41".into(),
        symbol_summary: t!(i18n, ids::LIBRARY_ROW_SYMBOL_PINS, "pins" => 2usize),
        footprint_summary: t!(i18n, ids::LIBRARY_ROW_NO_FOOTPRINT),
        read_only: false,
        symbol_warn: false,
        footprint_warn: true,
    }
}

fn texts(d: &LibraryRowDetail) -> Vec<String> {
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let i18n = I18n::en_us();
    lay_out_texts(view_library_row_properties(d, &tokens, &i18n), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn clicked(d: &LibraryRowDetail, label: &str) -> Vec<PanelMsg> {
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    messages_on_click(
        view_library_row_properties(d, &tokens, &i18n),
        WINDOW,
        label,
    )
}

#[test]
fn the_header_names_the_table_and_the_rows_place_in_it() {
    let shown = texts(&detail());
    for line in [
        "R-0402-4K7-1%",
        "Library Row · Resistors · row 18",
        "resistor",
        "Released",
        "· v3",
        "RC0402FR-074K7L",
        "Yageo",
        "passives",
        "2026-11-12 09:41",
    ] {
        assert!(shown.iter().any(|t| t == line), "{line}: {shown:#?}");
    }
    let mut unplaced = detail();
    unplaced.position = None;
    let shown = texts(&unplaced);
    assert!(
        shown
            .iter()
            .any(|t| t.starts_with("Library Row · Resistors · row ") && !t.ends_with("18")),
        "a row not found by place shows its short id: {shown:#?}"
    );
}

#[test]
fn a_bound_card_offers_pick_and_an_unbound_one_the_named_primary_pick() {
    let d = detail();
    let shown = texts(&d);
    assert!(shown.iter().any(|t| t == "Pick…"), "{shown:#?}");
    assert!(shown.iter().any(|t| t == "Pick Footprint…"), "{shown:#?}");
    assert!(!shown.iter().any(|t| t == "Pick Symbol…"), "{shown:#?}");
    assert!(matches!(
        clicked(&d, "Pick…").as_slice(),
        [PanelMsg::LibraryRowPickSymbol]
    ));
    assert!(matches!(
        clicked(&d, "Pick Footprint…").as_slice(),
        [PanelMsg::LibraryRowPickFootprint]
    ));
}

#[test]
fn a_read_only_library_disables_both_picks() {
    let mut d = detail();
    d.read_only = true;
    assert!(clicked(&d, "Pick…").is_empty());
    assert!(clicked(&d, "Pick Footprint…").is_empty());
}

#[test]
fn an_empty_field_shows_the_no_value_mark() {
    let mut d = detail();
    d.manufacturer.clear();
    let shown = texts(&d);
    assert!(shown.iter().any(|t| t == "—"), "{shown:#?}");
}
