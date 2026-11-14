//! Round 9 of #2262: a PCB tab's Properties show the board, not a schematic
//! hint; #2353: the board panel's sections.

use super::*;
use crate::app::Signex;
use crate::layout_probe::lay_out_texts;
use signex_types::pcb::{LayerDef, Point};

// Tall enough to lay out every section of the board panel.
const WINDOW: iced::Size = iced::Size::new(320.0, 4000.0);

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
    ctx.unit = Unit::Mm;
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
        "Board Size",
        "Horizontal",
        "50.000 mm",
        "Vertical",
        "40.000 mm",
        "Area",
        "2000.00 mm²",
        "Signal",
        "1.600 mm",
        "Nets",
        "SELECTION FILTER",
        "SNAP OPTIONS",
        "BOARD INFORMATION",
        "GRID MANAGER",
        "GUIDE MANAGER",
        "OTHER",
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
    assert!(!texts.iter().any(|t| t == "Board Size"), "{texts:?}");
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
        designator_draft: None,
        value_draft: draft.then(|| "4k7".into()),
        editability,
        also_on,
    };
    let component = super::super::pcb_component::tests::one_component("R7", "R_0603");
    ctx.pcb_board = ctx.pcb_board.map(|b| {
        b.with_footprint(Some(facts))
            .with_component(Some(component))
    });
    ctx
}

/// #1312: one selected footprint shows its own rows, not the card. Since
/// #2354 they are the Component view's: its title, and the value is Comment.
#[test]
fn one_selected_footprint_shows_its_designator_value_and_name() {
    use super::super::FootprintEditability as E;
    let texts = texts(&footprint_context(E::Editable, 0, false));
    for wanted in ["Component", "R7", "Designator", "Comment", "R_0603"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t.contains("selected on")),
        "{texts:?}"
    );
    assert!(!texts.iter().any(|t| t.contains("Enter")), "{texts:?}");
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Update PCB renames parts back")),
        "{texts:?}"
    );
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

/// #2353: the search keeps the rows whose label holds the text, and says so
/// when none does.
#[test]
fn the_search_keeps_matching_rows() {
    let mut ctx = context(0);
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.search = "pad".into();
    }
    let texts = texts(&ctx);
    for wanted in ["Pad Centers", "Pads", "Pad/Via Holes"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(!texts.iter().any(|t| t == "Horizontal"), "{texts:?}");
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.search = "zzzz".into();
    }
    let texts = texts(&ctx);
    assert!(
        texts.iter().any(|t| t == "Nothing matches the search."),
        "{texts:?}"
    );
}

/// #2353: the header sums up the Selection Filter.
#[test]
fn the_header_sums_up_the_selection_filter() {
    let mut ctx = context(0);
    let texts_all = texts(&ctx);
    assert!(
        texts_all.iter().any(|t| t == "Components (and 12 more)"),
        "{texts_all:?}"
    );
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.filter.toggle(PickKind::Components);
    }
    let texts = texts(&ctx);
    assert!(
        texts.iter().any(|t| t == "3D Bodies (and 11 more)"),
        "{texts:?}"
    );
}
