//! Mockup row 178: the Polygon Manager's model, shared by the tab and the
//! PCB panel's Polygons mode.

use std::path::PathBuf;

use signex_i18n::{I18n, ids, t};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::*;

fn pour(n: u128, layer: &str, net: &str, state: PourState) -> PourRow {
    PourRow {
        zone: Uuid::from_u128(n),
        name: format!("{net}_{n}"),
        net: net.to_owned(),
        layer: layer.to_owned(),
        layer_place: 1,
        order: (state != PourState::Shelved).then_some(1),
        state,
        unfilled: None,
        clearance_mm: 0.3,
        min_width_mm: 0.254,
        islands_below_mm2: 2.5,
        dead_copper: true,
        area_mm2: Some(10.0),
        pieces: Some(1),
        islands_removed: Some(0),
    }
}

fn board() -> PathBuf {
    PathBuf::from("dcdc-buck.snxpcb")
}

fn facts() -> PolygonFacts {
    PolygonFacts {
        board: board(),
        read_only: false,
        rows: vec![
            pour(1, "Top Layer", "VIN_12V", PourState::Poured),
            pour(2, "Top Layer", "GND", PourState::Poured),
            pour(3, "Bottom Layer", "GND", PourState::Shelved),
            pour(4, "Bottom Layer", "", PourState::NotFilled),
        ],
    }
}

fn zones(rows: &[&PourRow]) -> Vec<u128> {
    rows.iter().map(|row| row.zone.as_u128()).collect()
}

fn apply(ui: &mut PolygonsUi, msg: PolygonMsg) {
    ui.apply(&msg);
}

#[test]
fn the_filters_keep_the_layer_net_and_state_together() {
    let facts = facts();
    let mut ui = PolygonsUi::default();
    assert_eq!(zones(&ui.tab_rows(&facts)), [1, 2, 3, 4]);
    apply(
        &mut ui,
        PolygonMsg::NetFilterPicked {
            board: board(),
            net: Some("GND".into()),
        },
    );
    assert_eq!(zones(&ui.tab_rows(&facts)), [2, 3]);
    apply(
        &mut ui,
        PolygonMsg::LayerFilterPicked {
            board: board(),
            layer: Some("Bottom Layer".into()),
        },
    );
    assert_eq!(zones(&ui.tab_rows(&facts)), [3]);
    apply(
        &mut ui,
        PolygonMsg::StateFilterPicked {
            board: board(),
            state: Some(PourState::Poured),
        },
    );
    assert!(ui.tab_rows(&facts).is_empty());
}

#[test]
fn the_shelved_page_lists_only_shelved_pours() {
    let facts = facts();
    let mut ui = PolygonsUi::default();
    apply(
        &mut ui,
        PolygonMsg::SubTabPicked {
            board: board(),
            tab: PolygonSubTab::Shelved,
        },
    );
    assert_eq!(zones(&ui.tab_rows(&facts)), [3]);
}

#[test]
fn a_click_ticks_one_row_and_ticks_add_and_remove() {
    let facts = facts();
    let mut ui = PolygonsUi::default();
    let zone = |n| Uuid::from_u128(n);
    apply(
        &mut ui,
        PolygonMsg::RowTicked {
            board: board(),
            zone: zone(1),
            on: true,
        },
    );
    apply(
        &mut ui,
        PolygonMsg::RowTicked {
            board: board(),
            zone: zone(3),
            on: true,
        },
    );
    assert_eq!(ui.selected_in(&facts), [zone(1), zone(3)]);
    assert_eq!(ui.current_in(&facts).map(|r| r.zone), Some(zone(3)));
    apply(
        &mut ui,
        PolygonMsg::RowTicked {
            board: board(),
            zone: zone(3),
            on: false,
        },
    );
    assert_eq!(
        ui.current,
        Some(zone(1)),
        "the detail moves to a ticked row"
    );
    apply(
        &mut ui,
        PolygonMsg::RowClicked {
            board: board(),
            zone: zone(2),
        },
    );
    assert_eq!(ui.selected_in(&facts), [zone(2)]);
    apply(
        &mut ui,
        PolygonMsg::ShownTicked {
            board: board(),
            zones: vec![zone(1), zone(4)],
            on: true,
        },
    );
    assert_eq!(ui.selected_in(&facts), [zone(1), zone(2), zone(4)]);
}

#[test]
fn state_made_for_another_board_starts_over() {
    let facts = facts();
    let mut ui = PolygonsUi::default();
    apply(
        &mut ui,
        PolygonMsg::RowClicked {
            board: PathBuf::from("other.snxpcb"),
            zone: Uuid::from_u128(1),
        },
    );
    assert!(ui.selected_in(&facts).is_empty(), "not this board's ticks");
    assert!(ui.current_in(&facts).is_none());
    apply(
        &mut ui,
        PolygonMsg::SubTabPicked {
            board: board(),
            tab: PolygonSubTab::BulkEdit,
        },
    );
    assert!(
        ui.selected.is_empty(),
        "following a new board drops the ticks"
    );
    assert_eq!(ui.sub_tab, PolygonSubTab::BulkEdit);
}

#[test]
fn the_panel_chip_keeps_the_pours_that_need_attention() {
    let facts = facts();
    let mut ui = PolygonsUi::default();
    assert_eq!(ui.panel_rows(&facts).len(), 4);
    apply(
        &mut ui,
        PolygonMsg::AttentionPicked {
            board: board(),
            on: true,
        },
    );
    assert_eq!(zones(&ui.panel_rows(&facts)), [3, 4]);
}

#[test]
fn a_bulk_field_reads_mixed_when_the_rows_differ() {
    let mut facts = facts();
    facts.rows[1].clearance_mm = 0.25;
    facts.rows[1].dead_copper = false;
    let all: Vec<&PourRow> = facts.rows.iter().collect();
    assert_eq!(shared_value(&all, PourField::MinWidth), Some(0.254));
    assert_eq!(shared_value(&all, PourField::Clearance), None);
    assert_eq!(shared_dead_copper(&all), None);
    assert_eq!(shared_dead_copper(&all[2..]), Some(true));
    assert_eq!(shared_value(&[], PourField::Clearance), None);
}

#[test]
fn the_layers_and_nets_lists_name_each_once() {
    let facts = facts();
    assert_eq!(facts.layers(), ["Top Layer", "Bottom Layer"]);
    assert_eq!(facts.nets(), ["GND", "VIN_12V"]);
    assert_eq!(facts.count(PourState::Shelved), 1);
}

#[test]
fn the_page_and_mode_text_comes_from_the_catalog() {
    let i18n = I18n::en_us_ref();
    assert_eq!(
        t!(i18n, ids::POLYGON_MANAGER_PAGE).into_string(),
        "Polygons"
    );
    assert_eq!(
        t!(i18n, ids::POLYGON_MANAGER_OPEN).into_string(),
        "Open Polygon Manager"
    );
    let names: Vec<String> = PolygonSubTab::ALL
        .iter()
        .map(|tab| t!(i18n, tab.message()).into_string())
        .collect();
    assert_eq!(names, ["Pours", "Shelved", "Bulk edit"]);
}

#[test]
fn an_empty_clearance_reads_as_the_board_clearance() {
    assert_eq!(PourField::Clearance.read(""), Some(0.0));
    assert_eq!(PourField::Clearance.read("  "), Some(0.0));
    assert_eq!(PourField::Clearance.read("0.4 mm"), Some(0.4));
    assert_eq!(
        PourField::MinWidth.read(""),
        None,
        "only Clearance has a default"
    );
    assert_eq!(PourField::IslandsBelow.read("wide"), None);
}

#[test]
fn a_change_of_ticks_or_of_the_board_drops_the_typed_text() {
    let mut ui = PolygonsUi::default();
    let bulk = |text: &str| PolygonMsg::BulkTyped {
        board: board(),
        field: PourField::MinWidth,
        text: text.into(),
    };
    apply(&mut ui, bulk("0.3"));
    let ticked = PolygonMsg::RowTicked {
        board: board(),
        zone: Uuid::from_u128(2),
        on: true,
    };
    apply(&mut ui, ticked);
    assert!(
        ui.bulk_drafts.is_empty(),
        "typed for the rows ticked before"
    );
    apply(&mut ui, bulk("0.4"));
    let cell = PolygonMsg::FieldTyped {
        board: board(),
        zone: Uuid::from_u128(1),
        field: PourField::Clearance,
        text: "0.6".into(),
    };
    apply(&mut ui, cell);
    ui.drop_drafts_of(&PathBuf::from("another.snxpcb"));
    assert_eq!(
        ui.draft(Uuid::from_u128(1), PourField::Clearance),
        Some("0.6")
    );
    ui.drop_drafts_of(&board());
    assert!(ui.drafts.is_empty() && ui.bulk_drafts.is_empty());
}

#[test]
fn builds_on_both_themes_with_and_without_a_board() {
    let (app, _boot) = crate::app::Signex::new();
    for id in ThemeId::BUILTINS {
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.tokens = theme_tokens(*id);
        let _ = view(&ctx);
        let _ = panel_view(&ctx);
        ctx.polygons = Some(facts());
        ctx.pcb_panel.polygons.board = Some(board());
        ctx.pcb_panel.polygons.selected = [1, 2].map(Uuid::from_u128).into();
        ctx.pcb_panel.polygons.current = Some(Uuid::from_u128(1));
        for tab in PolygonSubTab::ALL {
            ctx.pcb_panel.polygons.sub_tab = tab;
            let _ = view(&ctx);
        }
        let _ = panel_view(&ctx);
    }
}
