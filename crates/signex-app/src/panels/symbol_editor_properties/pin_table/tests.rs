//! The Pin table document tab (mockup row 122b), laid out headless: its
//! seven columns and the Check column, the issue pills, the Hidden box, the
//! bulk bar that waits for a tick, the issues-only view and Paste rows.

use iced::widget::text_editor;
use iced::{Point, Size};
use signex_i18n::{ids, t};

use super::super::super::{PanelMsg, SymbolEditorSelection};
use super::super::tests::{Fixture, ctx, opamp_pins};
use super::*;
use crate::layout_probe::{find, lay_out_texts, messages_on_click, messages_on_click_at};
use crate::library::editor::symbol::{PasteDraft, PinTableMsg, PinTableUi, SymbolPropsMsg};

/// A document area: wide enough for every column.
const TAB: Size = Size::new(1400.0, 900.0);

fn tab_view<'a>(
    sym: &'a SymbolEditorPanelContext,
    ui: &'a PinTableUi,
    fx: &'a Fixture,
) -> Element<'a, PanelMsg> {
    view_pin_table(sym, ui, &fx.toggled, None, &fx.tokens, &fx.i18n)
}

fn layout_texts(sym: &SymbolEditorPanelContext, ui: &PinTableUi, fx: &Fixture) -> Vec<String> {
    lay_out_texts(tab_view(sym, ui, fx), TAB)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

fn word(fx: &Fixture, id: signex_i18n::MessageId) -> String {
    t!(fx.i18n, id).into_string()
}

fn is_table(msg: &PanelMsg, wanted: &PinTableMsg) -> bool {
    matches!(msg, PanelMsg::SymbolProps(SymbolPropsMsg::PinTable(m)) if m == wanted)
}

#[test]
fn the_table_has_all_seven_columns_and_the_check_column() {
    let fx = Fixture::new();
    let sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    let texts = layout_texts(&sym, &PinTableUi::default(), &fx);
    for id in [
        ids::EDITORS_PINS_COLUMN_NUMBER,
        ids::EDITORS_PINS_COLUMN_NAME,
        ids::EDITORS_PINS_COLUMN_ELECTRICAL,
        ids::EDITORS_PINS_COLUMN_PART,
        ids::EDITORS_PINS_COLUMN_HIDDEN,
        ids::EDITORS_PINS_COLUMN_LENGTH,
        ids::EDITORS_PINS_COLUMN_ORIENTATION,
        ids::EDITORS_PIN_TABLE_COLUMN_CHECK,
    ] {
        let heading = word(&fx, id);
        assert!(texts.contains(&heading), "{heading} in {texts:?}");
    }
    let ok = word(&fx, ids::EDITORS_PIN_TABLE_OK);
    assert_eq!(
        texts.iter().filter(|t| **t == ok).count(),
        8,
        "every row is OK"
    );
    assert!(texts.contains(&word(&fx, ids::EDITORS_PIN_TABLE_NO_ISSUES)));
}

#[test]
fn a_duplicate_number_and_an_empty_name_are_checked() {
    let fx = Fixture::new();
    let mut pins = opamp_pins();
    pins[5].number = "5".into();
    pins[6].name = String::new();
    let sym = ctx(pins, SymbolEditorSelection::None);
    let texts = layout_texts(&sym, &PinTableUi::default(), &fx);
    let duplicate = t!(fx.i18n, ids::EDITORS_PIN_TABLE_DUPLICATE, "number" => "5").into_string();
    assert_eq!(texts.iter().filter(|t| **t == duplicate).count(), 2);
    assert!(texts.contains(&word(&fx, ids::EDITORS_PIN_TABLE_NO_NAME)));
    let issues = t!(fx.i18n, ids::EDITORS_PIN_TABLE_ISSUES, "count" => 3usize).into_string();
    assert!(texts.contains(&issues), "{texts:?}");
}

#[test]
fn the_issues_only_view_lists_only_rows_with_an_issue() {
    let fx = Fixture::new();
    let mut pins = opamp_pins();
    pins[2].name = String::new();
    let sym = ctx(pins, SymbolEditorSelection::None);
    let issues = pin_issues(&sym.pins);
    let ui = PinTableUi {
        issues_only: true,
        ..PinTableUi::default()
    };
    let listed: Vec<usize> = listed_rows(&sym, &issues, &ui, &fx.i18n)
        .iter()
        .map(|p| p.idx)
        .collect();
    assert_eq!(listed, [2]);
    let ui = PinTableUi {
        filter: "inb".into(),
        ..PinTableUi::default()
    };
    let listed: Vec<usize> = listed_rows(&sym, &issues, &ui, &fx.i18n)
        .iter()
        .map(|p| p.idx)
        .collect();
    assert_eq!(listed, [4, 5]);
}

/// The Hidden box of a row sends the Pin properties' toggle for that pin.
/// The box sits at the start of its cell, under the HIDDEN heading.
#[test]
fn a_rows_hidden_box_toggles_that_pin() {
    let fx = Fixture::new();
    let sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    let ui = PinTableUi::default();
    let texts = lay_out_texts(tab_view(&sym, &ui, &fx), TAB);
    let hidden = find(&texts, &word(&fx, ids::EDITORS_PINS_COLUMN_HIDDEN)).bounds;
    let ok = word(&fx, ids::EDITORS_PIN_TABLE_OK);
    let first_ok = texts
        .iter()
        .find(|t| t.text == ok)
        .map(|t| t.bounds)
        .expect("a Check pill");
    let at = Point::new(hidden.x + TS.heading / 2.0, first_ok.center_y());
    let messages = messages_on_click_at(tab_view(&sym, &ui, &fx), TAB, None, at);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, PanelMsg::SymEditorTogglePinHidden(0))),
        "{messages:?}"
    );
}

#[test]
fn the_bulk_bar_waits_for_a_ticked_row() {
    let fx = Fixture::new();
    let sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    let apply = word(&fx, ids::EDITORS_PIN_TABLE_APPLY);
    let click = |ui: &PinTableUi| messages_on_click(tab_view(&sym, ui, &fx), TAB, &apply);
    let idle = PinTableUi::default();
    assert!(
        layout_texts(&sym, &idle, &fx).contains(&word(&fx, ids::EDITORS_PIN_TABLE_NONE_SELECTED))
    );
    assert!(click(&idle).is_empty());
    let mut ticked = PinTableUi::default();
    ticked.checked.extend([1, 2]);
    let selected = t!(fx.i18n, ids::EDITORS_PIN_TABLE_SELECTED, "count" => 2usize).into_string();
    assert!(layout_texts(&sym, &ticked, &fx).contains(&selected));
    let messages = click(&ticked);
    assert!(
        messages
            .iter()
            .any(|m| is_table(m, &PinTableMsg::BulkApplied)),
        "{messages:?}"
    );
}

#[test]
fn delete_selected_needs_a_tick_and_paste_rows_opens_the_box() {
    let fx = Fixture::new();
    let sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    let delete = word(&fx, ids::EDITORS_PIN_TABLE_DELETE);
    let paste = word(&fx, ids::EDITORS_PIN_TABLE_PASTE);
    let idle = PinTableUi::default();
    assert!(messages_on_click(tab_view(&sym, &idle, &fx), TAB, &delete).is_empty());
    let messages = messages_on_click(tab_view(&sym, &idle, &fx), TAB, &paste);
    assert!(
        messages
            .iter()
            .any(|m| is_table(m, &PinTableMsg::PasteToggled)),
        "{messages:?}"
    );
    let mut ticked = PinTableUi::default();
    ticked.checked.insert(3);
    let messages = messages_on_click(tab_view(&sym, &ticked, &fx), TAB, &delete);
    assert!(
        messages
            .iter()
            .any(|m| is_table(m, &PinTableMsg::DeleteSelectedPressed)),
        "{messages:?}"
    );
}

#[test]
fn the_paste_box_counts_what_apply_paste_does() {
    let fx = Fixture::new();
    let sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    let ui = PinTableUi {
        paste: Some(PasteDraft {
            content: text_editor::Content::with_text("1\tOUTA\tO\n9\tNC\tP"),
            ..PasteDraft::default()
        }),
        ..PinTableUi::default()
    };
    let texts = layout_texts(&sym, &ui, &fx);
    let counted = t!(
        fx.i18n,
        ids::EDITORS_PIN_TABLE_PREVIEW_UPDATE,
        "rows" => 2usize,
        "updated" => 1usize,
        "added" => 1usize
    )
    .into_string();
    assert!(texts.contains(&counted), "{texts:?}");
    let column = t!(fx.i18n, ids::EDITORS_PIN_TABLE_PASTE_COLUMN, "number" => 4usize).into_string();
    assert!(texts.contains(&column), "four columns at least");
    let apply = word(&fx, ids::EDITORS_PIN_TABLE_PASTE_APPLY);
    let messages = messages_on_click(tab_view(&sym, &ui, &fx), TAB, &apply);
    assert!(
        messages
            .iter()
            .any(|m| is_table(m, &PinTableMsg::PasteApplied)),
        "{messages:?}"
    );
}

#[test]
fn an_empty_paste_box_offers_nothing_to_apply() {
    let fx = Fixture::new();
    let sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    let ui = PinTableUi {
        paste: Some(PasteDraft::default()),
        ..PinTableUi::default()
    };
    let texts = layout_texts(&sym, &ui, &fx);
    assert!(texts.contains(&word(&fx, ids::EDITORS_PIN_TABLE_PREVIEW_EMPTY)));
    let apply = word(&fx, ids::EDITORS_PIN_TABLE_PASTE_APPLY);
    assert!(messages_on_click(tab_view(&sym, &ui, &fx), TAB, &apply).is_empty());
}
