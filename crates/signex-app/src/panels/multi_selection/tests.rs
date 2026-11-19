use super::view::{footer_text, kinds_text};
use super::*;
use crate::panels::{PanelContext, PanelMsg};
use iced::Color;
use signex_engine::{LabelGroup, SelectionGroups, SymbolGroup, WireGroup};
use signex_i18n::{I18n, ids};
use signex_types::schematic::{SelectedItem, SelectedKind};

/// The texts the view lays out, isolation marks stripped.
fn shown_texts(ctx: &PanelContext) -> Vec<String> {
    let body = view_multi_selection(ctx, Color::BLACK, Color::BLACK, Color::BLACK);
    crate::layout_probe::lay_out_texts(body, iced::Size::new(400.0, 2000.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn groups() -> SelectionGroups {
    SelectionGroups {
        symbols: Some(SymbolGroup {
            count: 9,
            value: Shared::Same("10k".into()),
            footprint: Shared::Mixed,
            dnp: Shared::Same(false),
            in_bom: Shared::Same(true),
            on_board: Shared::Mixed,
            parameters: Vec::new(),
        }),
        labels: Some(LabelGroup {
            count: 3,
            font_size_mm: Shared::Mixed,
            justify: Shared::Mixed,
            rotation_degrees: Shared::Mixed,
        }),
        wires: Some(WireGroup {
            count: 2,
            width_mm: Shared::Same(0.15),
        }),
    }
}

fn items(kind: SelectedKind, n: usize) -> Vec<SelectedItem> {
    (0..n)
        .map(|_| SelectedItem::new(uuid::Uuid::now_v7(), kind))
        .collect()
}

/// A context with 9 parts, 3 labels and 2 wires selected.
fn mixed_ctx() -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.selection_count = 14;
    ctx.multi_groups = groups();
    let mut selected = items(SelectedKind::Symbol, 9);
    selected.extend(items(SelectedKind::Label, 3));
    selected.extend(items(SelectedKind::Wire, 2));
    ctx.selection_form.kinds = SelectionKinds::of(&selected);
    ctx
}

fn has(texts: &[String], want: &str) -> bool {
    texts.iter().any(|t| t == want)
}

/// #51: one section per kind under the Parts noun, the footer counting
/// every kind, and the mixed caption under the label rows.
#[test]
fn the_view_shows_a_section_per_kind_and_the_footer() {
    let texts = shown_texts(&mixed_ctx());
    for want in [
        "Multi-Selection",
        "14 objects selected",
        "PARTS",
        "LABELS",
        "WIRES",
        "Font Size",
        "Justify and Rotation are mixed; a click sets all 3.",
        "14 selected: 9 parts, 3 labels, 2 wires",
    ] {
        assert!(has(&texts, want), "{want:?} not in {texts:?}");
    }
}

/// #51: a kind chip filters the sections the panel shows.
#[test]
fn a_kind_chip_shows_only_its_section() {
    let mut ctx = mixed_ctx();
    ctx.selection_form.kind_filter = Some(MultiKind::Labels);
    let texts = shown_texts(&ctx);
    assert!(has(&texts, "LABELS"), "{texts:?}");
    assert!(!has(&texts, "PARTS") && !has(&texts, "WIRES"), "{texts:?}");
}

/// A chip whose kind left the selection reads as All.
#[test]
fn a_chip_of_a_kind_no_longer_selected_falls_back_to_all() {
    let mut form = SelectionForm {
        kind_filter: Some(MultiKind::Wires),
        ..Default::default()
    };
    let mut g = groups();
    assert_eq!(form.effective_filter(&g), Some(MultiKind::Wires));
    g.wires = None;
    assert_eq!(form.effective_filter(&g), None);
    assert!(form.shows(MultiKind::Parts, &g));
    assert!(form.update(&SelectionFormMsg::KindChipPicked(None)));
    assert_eq!(form.kind_filter, None);
}

/// #51: picking a chip is view state only; it never selects or deselects.
#[test]
fn a_chip_press_is_view_state_and_never_a_selection_edit() {
    let ctx = mixed_ctx();
    let body = view_multi_selection(&ctx, Color::BLACK, Color::BLACK, Color::BLACK);
    let messages =
        crate::layout_probe::messages_on_click(body, iced::Size::new(400.0, 2000.0), "Labels");
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        matches!(
            &messages[0],
            PanelMsg::SelectionForm(SelectionFormMsg::KindChipPicked(Some(MultiKind::Labels)))
        ),
        "{messages:?}"
    );
}

/// #51: with no shared field the card says so and lists what is selected.
#[test]
fn the_nothing_shared_card_lists_the_selected_kinds() {
    let mut ctx = mixed_ctx();
    ctx.multi_groups = SelectionGroups::default();
    ctx.selection_count = 4;
    let mut selected = items(SelectedKind::Junction, 2);
    selected.extend(items(SelectedKind::NoConnect, 1));
    selected.extend(items(SelectedKind::SheetPin, 1));
    ctx.selection_form.kinds = SelectionKinds::of(&selected);
    assert_eq!(kinds_text(&ctx), "2 junctions, 1 no-connect, 1 sheet entry");
    let texts = shown_texts(&ctx);
    assert!(has(&texts, "Nothing to edit together"), "{texts:?}");
    assert!(
        has(
            &texts,
            "Nothing in this selection has fields shared for editing. \
             Selected: 2 junctions, 1 no-connect, 1 sheet entry"
        ),
        "{texts:?}"
    );
    assert!(!has(&texts, "PARTS"), "no section without a shared field");
}

#[test]
fn the_footer_counts_part_fields_as_one_kind() {
    let mut ctx = mixed_ctx();
    let mut selected = items(SelectedKind::SymbolRefField, 1);
    selected.extend(items(SelectedKind::SymbolValField, 2));
    ctx.selection_form.kinds = SelectionKinds::of(&selected);
    ctx.selection_count = 3;
    let footer = footer_text(&ctx)
        .to_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(footer, "3 selected: 3 part fields");
}

/// The catalog says what the English literals said (#2059), and the new
/// wording of #51.
#[test]
fn the_mixed_and_count_texts_read_as_designed() {
    let i18n = I18n::en_us();
    let plain = |id, args: &[(&str, signex_i18n::Arg)]| i18n.message_plain(id, args);
    assert_eq!(plain(ids::PLACEHOLDER_MIXED, &[]), MIXED);
    assert_eq!(
        plain(
            ids::PANELS_MULTI_OBJECTS_SELECTED,
            &[("count", "1234".into())]
        ),
        "1234 objects selected"
    );
    assert_eq!(plain(ids::PROPERTIES_KIND_PARTS, &[]), "Parts");
    assert_eq!(
        plain(ids::PROPERTIES_COUNT_PARTS, &[("count", 1_usize.into())]),
        "1 part"
    );
    assert_eq!(
        plain(
            ids::PROPERTIES_COUNT_SHEET_ENTRIES,
            &[("count", 2_usize.into())]
        ),
        "2 sheet entries"
    );
}

#[test]
fn numbers_are_trimmed_and_text_is_taken_verbatim() {
    assert_eq!(number_text(1.27), "1.27");
    assert_eq!(number_text(10.0), "10");
    assert_eq!(
        edit_from_text(&MultiField::SymbolValue, " 4k7").unwrap(),
        MultiEdit::SymbolValue(" 4k7".into())
    );
    assert_eq!(
        edit_from_text(&MultiField::SymbolValue, "").unwrap(),
        MultiEdit::SymbolValue(String::new())
    );
}

#[test]
fn font_size_converts_points_and_refuses_zero_negative_and_words() {
    let MultiEdit::LabelFontSizeMm(mm) = edit_from_text(&MultiField::LabelFontPt, "10").unwrap()
    else {
        panic!("a font size edit");
    };
    assert!((mm - 10.0 * SCHEMATIC_PT_TO_MM).abs() < 1e-9);
    for bad in ["0", "-3", "big", ""] {
        assert!(
            edit_from_text(&MultiField::LabelFontPt, bad).is_err(),
            "{bad:?}"
        );
    }
}

#[test]
fn wire_width_refuses_negative_and_zero_is_the_default() {
    assert!(edit_from_text(&MultiField::WireWidth, "-0.1").is_err());
    assert_eq!(
        edit_from_text(&MultiField::WireWidth, "0").unwrap(),
        MultiEdit::WireWidthMm(0.0)
    );
}

#[test]
fn the_rule_filter_ignores_case_and_blanks() {
    assert!(rule_matches("Unused pin", ""));
    assert!(rule_matches("Unused pin", " PIN "));
    assert!(!rule_matches("Unused pin", "wire"));
}
