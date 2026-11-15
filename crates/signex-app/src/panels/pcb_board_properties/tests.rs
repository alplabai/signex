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
        options: Default::default(),
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
        "Width",
        "50.000 mm",
        "Height",
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
    let shown = texts(&ctx);
    for wanted in ["Pad Centers", "Pads", "Holes"] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted} in {shown:?}");
    }
    assert!(!shown.iter().any(|t| t == "Width"), "{shown:?}");
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.search = "zzzz".into();
    }
    let shown = texts(&ctx);
    assert!(
        shown.iter().any(|t| t == "Nothing matches the search."),
        "{shown:?}"
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

/// #2374: the Guide and Grid Managers list the board's guides and grids,
/// and the sheet fields are greyed while the sheet follows the board.
#[test]
fn the_managers_list_the_boards_guides_and_grids() {
    use signex_types::pcb_options::{BoardGrid, BoardGuide, GuideKind, MARK_COLORS};
    let mut ctx = context(0);
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.options.guides.push(BoardGuide::new(
            uuid::Uuid::from_u128(1),
            GuideKind::Plus45,
            Point { x: 2.5, y: 0.0 },
        ));
        board.options.grids.push(BoardGrid {
            uuid: uuid::Uuid::from_u128(2),
            name: "Fine pads".into(),
            step_mm: 0.1,
            origin: Point { x: 0.0, y: 0.0 },
            color: MARK_COLORS[1],
            components: true,
            non_components: false,
        });
    }
    let texts = texts(&ctx);
    for wanted in [
        "+45°",
        "Fine pads",
        "Always",
        "50",
        "Get Size From Sheet Layer",
    ] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(!texts.iter().any(|t| t == "No guides"), "{texts:?}");
}

/// The default width of the right dock (`UiState::right_width` at startup).
const DEFAULT_DOCK: f32 = 220.0;

fn laid_out(ctx: &PanelContext, width: f32) -> Vec<crate::layout_probe::LaidOutText> {
    let window = iced::Size::new(width, WINDOW.height);
    lay_out_texts(super::super::properties::view_properties(ctx), window)
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

/// #2386: the Components card lays Total, Top and Bottom out on one row.
#[test]
fn the_components_card_puts_its_three_figures_on_one_row() {
    let texts = laid_out(&context(0), DEFAULT_DOCK);
    // The Selection Filter has a Components chip too: start at Board Size.
    let size = texts
        .iter()
        .position(|t| t.text == "Board Size")
        .expect("card");
    let card = size
        + texts[size..]
            .iter()
            .position(|t| t.text == "Components")
            .expect("the Components card");
    let after = &texts[card..];
    let y = |label: &str| crate::layout_probe::find(after, label).bounds.y;
    assert_eq!(y("Total"), y("Top"), "{after:?}");
    assert_eq!(y("Top"), y("Bottom"), "{after:?}");
    let x = |label: &str| crate::layout_probe::find(after, label).bounds.x;
    assert!(x("Total") < x("Top") && x("Top") < x("Bottom"));
}

/// #2386: no label of the board panel wraps at the default dock width.
#[test]
fn no_board_panel_label_wraps_at_the_default_dock_width() {
    let texts = laid_out(&context(0), DEFAULT_DOCK);
    let one_line = crate::layout_probe::find(&texts, "Width").bounds.height;
    for label in [
        "Width",
        "Height",
        "Thickness",
        "Parts Area",
        "Unrouted",
        "Holes",
        "Keepouts",
        "DRC Violations",
        "Snapping",
        "Distance",
        "Axis Range",
        "Global Snap Grid",
        "Fast Grids",
        "Pour Names",
        "Designators",
    ] {
        let found = crate::layout_probe::find(&texts, label);
        assert!(
            found.bounds.height < one_line * 1.5,
            "{label} wraps: {found:?}"
        );
    }
}

/// #2386: a board with no outline shows one muted "No board outline" line
/// in place of its width, height and area.
#[test]
fn an_outline_less_board_says_so_on_one_line() {
    let mut ctx = context(0);
    let mut no_outline = board();
    no_outline.outline.clear();
    ctx.pcb_board = Some(PcbBoardSummary::of(&no_outline, "b".into(), 0));
    let texts = laid_out(&ctx, DEFAULT_DOCK);
    let line = crate::layout_probe::find(&texts, "No board outline");
    let thickness = crate::layout_probe::find(&texts, "Thickness");
    assert!(
        line.bounds.height < thickness.bounds.height * 2.0,
        "{line:?}"
    );
    let from = texts
        .iter()
        .position(|t| t.text == "Board Size")
        .expect("card");
    let rest = &texts[from..];
    let to = rest
        .iter()
        .position(|t| t.text == "Components")
        .expect("next card");
    let card = &rest[..to];
    for gone in ["Width", "Height", "Area"] {
        assert!(!card.iter().any(|t| t.text == gone), "{gone} in {card:?}");
    }
}

/// #2386: the Primitives card leaves out kinds the board cannot hold and
/// counts the drawn areas.
#[test]
fn the_primitives_card_counts_what_the_board_holds() {
    let texts = texts(&context(0));
    for wanted in ["Fills", "Regions", "Keepouts", "Polygons", "Holes"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(!texts.iter().any(|t| t == "Not available"), "{texts:?}");
}

/// #2383, #2386: Add Preset and the Board Information Run DRC send their
/// messages from the laid-out panel.
#[test]
fn add_preset_and_run_drc_are_wired() {
    let ctx = context(0);
    let window = iced::Size::new(DEFAULT_DOCK, WINDOW.height);
    let view = || super::super::properties::view_properties(&ctx);
    let added = crate::layout_probe::messages_on_click(view(), window, "Add Preset");
    assert!(
        added
            .iter()
            .any(|m| matches!(m, PanelMsg::PcbBoard(PcbBoardMsg::FilterPresetAdded))),
        "{added:?}"
    );
    let run = crate::layout_probe::messages_on_click(view(), window, "Run DRC");
    assert!(run.iter().any(|m| matches!(m, PanelMsg::RunDrc)), "{run:?}");
}
