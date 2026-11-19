//! #1259: the Object List as laid out: the empty state, the chips with their
//! counts, the footer, and a row click that names its object.

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::schematic::{SelectedItem, SelectedKind};

use super::model::{ObjectListState, ObjectListStatus, ObjectListView, sheet_rows};
use super::model_tests::power_sheet;
use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};

const WINDOW: iced::Size = iced::Size::new(900.0, 2400.0);

fn power_path() -> PathBuf {
    PathBuf::from("proj").join("Power.snxsch")
}

fn context(view: ObjectListView) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.object_list = view;
    ctx
}

fn listed(selected: &[SelectedItem]) -> PanelContext {
    let mut state = ObjectListState::default();
    state.source.rows = Arc::new(sheet_rows(
        &power_sheet(),
        &power_path(),
        &|_: uuid::Uuid| None,
    ));
    context(ObjectListView::build(
        &state,
        ObjectListStatus::Ready,
        Some(power_path()),
        selected,
        signex_i18n::I18n::en_us_ref(),
    ))
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(view(ctx), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn without_a_schematic_the_panel_shows_the_empty_card() {
    let shown = texts(&context(ObjectListView::default()));
    assert!(
        shown
            .iter()
            .any(|t| t == "Open a schematic to list its objects."),
        "{shown:?}"
    );
    assert!(!shown.iter().any(|t| t == "Filter objects"), "{shown:?}");
}

#[test]
fn the_list_shows_its_columns_chips_rows_and_footer() {
    let shown = texts(&listed(&[SelectedItem::new(
        uuid::Uuid::from_u128(1),
        SelectedKind::Symbol,
    )]));
    for want in [
        "This sheet",
        "Project",
        "All",
        "Parts",
        "Labels",
        "Wires",
        "More ▾",
        "Name",
        "Value / text",
        "Sheet",
        "U3",
        "INA226",
        "VOUT_3V3",
        "net label",
        "power port",
        "input.snxsch",
        "11 objects · 1 selected",
    ] {
        assert!(shown.iter().any(|t| t == want), "{want} in {shown:?}");
    }
    // Counts sit beside their chip labels.
    assert!(shown.iter().any(|t| t == "11"), "{shown:?}");
    assert!(!shown.iter().any(|t| t == "Junctions"), "More is closed");
}

#[test]
fn clicking_a_row_names_that_object() {
    let ctx = listed(&[]);
    let pressed = messages_on_click(view(&ctx), WINDOW, "R3");
    let [PanelMsg::ObjectList(ObjectListMsg::RowClicked(target))] = pressed.as_slice() else {
        panic!("one row click: {pressed:?}");
    };
    assert_eq!(
        target.item,
        SelectedItem::new(uuid::Uuid::from_u128(2), SelectedKind::Symbol)
    );
    assert_eq!(target.sheet, power_path());
}

#[test]
fn the_chips_and_scope_send_their_picks() {
    let ctx = listed(&[]);
    let pressed = messages_on_click(view(&ctx), WINDOW, "Wires");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::ObjectList(ObjectListMsg::KindChipToggled(
                KindChip::Kind(ObjectKind::Wire)
            ))]
        ),
        "{pressed:?}"
    );
    let pressed = messages_on_click(view(&ctx), WINDOW, "Project");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::ObjectList(ObjectListMsg::ScopePicked(
                ObjectScope::Project
            ))]
        ),
        "{pressed:?}"
    );
}
