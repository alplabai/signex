//! Round 9 of #2262: a PCB tab's Properties show the board, not a schematic
//! hint.

use super::*;
use crate::app::Signex;
use crate::layout_probe::lay_out_texts;
use signex_types::pcb::{LayerDef, Point};

const WINDOW: iced::Size = iced::Size::new(320.0, 900.0);

fn board() -> PcbBoard {
    let layer = |id: u8, name: &str| LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    };
    let p = |x: f64, y: f64| Point { x, y };
    PcbBoard {
        sketch: None,
        unknown: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![p(10.0, 5.0), p(60.0, 5.0), p(60.0, 45.0), p(10.0, 45.0)],
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

fn context(selected: usize) -> PanelContext {
    let (app, _boot) = Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.has_pcb = true;
    ctx.pcb_board = Some(PcbBoardSummary::of(
        &board(),
        "mixed-tht-smd".into(),
        selected,
    ));
    ctx
}

/// Every text the Properties panel lays out, isolation marks removed.
fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(super::super::properties::view_properties(ctx), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_summary_reads_the_outline_layers_and_thickness() {
    let summary = PcbBoardSummary::of(&board(), "b".into(), 0);
    assert_eq!(summary.size_mm, Some((50.0, 40.0)));
    assert_eq!(summary.copper_layers, 2);
    assert_eq!(summary.thickness_mm, 1.6);
    let mut no_outline = board();
    no_outline.outline.clear();
    assert_eq!(
        PcbBoardSummary::of(&no_outline, "b".into(), 0).size_mm,
        None
    );
}

#[test]
fn an_unselected_board_shows_its_own_properties() {
    let texts = texts(&context(0));
    for wanted in [
        "Board",
        "mixed-tht-smd",
        "Size",
        "50.00 × 40.00 mm",
        "2 copper layers",
        "1.60 mm",
        "Nets",
    ] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t.contains("schematic is active")),
        "{texts:?}"
    );
}

#[test]
fn a_selection_shows_a_card_naming_the_board() {
    let texts = texts(&context(3));
    assert!(
        texts
            .iter()
            .any(|t| t == "3 items selected on mixed-tht-smd"),
        "{texts:?}"
    );
    assert!(!texts.iter().any(|t| t == "Size"), "{texts:?}");
}

fn footprint_context(
    editability: super::super::FootprintEditability,
    also_on: usize,
    draft: bool,
) -> PanelContext {
    let mut ctx = context(1);
    let facts = super::super::PcbFootprintFacts {
        board: "board.snxpcb".into(),
        uuid: uuid::Uuid::from_u128(1),
        designator: "R7".into(),
        value: "10k".into(),
        footprint_name: "R_0603".into(),
        draft: draft.then(|| {
            (
                signex_engine::pcb::designators::FootprintTextField::Value,
                "4k7".into(),
            )
        }),
        editability,
        also_on,
    };
    ctx.pcb_board = ctx.pcb_board.map(|b| b.with_footprint(Some(facts)));
    ctx
}

/// #1312: one selected footprint shows its own rows, not the card.
#[test]
fn one_selected_footprint_shows_its_designator_value_and_name() {
    use super::super::FootprintEditability as E;
    let texts = texts(&footprint_context(E::Editable, 0, false));
    for wanted in ["Footprint", "R7", "Designator", "Value", "R_0603"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t.contains("selected on")),
        "{texts:?}"
    );
    assert!(!texts.iter().any(|t| t.contains("Enter")), "{texts:?}");
}

#[test]
fn the_footprint_rows_say_why_they_are_read_only_and_warn_of_a_shared_designator() {
    use super::super::FootprintEditability as E;
    let locked = texts(&footprint_context(E::Locked, 0, false));
    assert!(locked.iter().any(|t| t.contains("is locked")), "{locked:?}");
    let read_only = texts(&footprint_context(E::ReadOnly, 0, false));
    assert!(
        read_only.iter().any(|t| t.contains("read-only")),
        "{read_only:?}"
    );
    let shared = texts(&footprint_context(E::Editable, 2, true));
    assert!(
        shared
            .iter()
            .any(|t| t == "R7 is also on 2 other footprints."),
        "{shared:?}"
    );
    assert!(
        shared
            .iter()
            .any(|t| t == "Press Enter to apply the change."),
        "{shared:?}"
    );
}
