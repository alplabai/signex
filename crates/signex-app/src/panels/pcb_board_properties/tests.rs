//! Round 9 of #2262: a PCB tab's Properties show the board, not a schematic
//! hint; #2353: the board panel's sections; #2523 (mockup row 93): the size row, the
//! stat tiles, the sections closed by default and the Health Check cards.

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
        stackup: None,
        rooms: Vec::new(),
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
        free: Default::default(),
    }
}

fn context(selected: usize) -> PanelContext {
    let (app, _boot) = Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.has_pcb = true;
    ctx.unit = Unit::Mm;
    ctx.pcb_board = Some(Box::new(PcbBoardSummary::of(
        &board(),
        "mixed-tht-smd".into(),
        selected,
    )));
    ctx
}

/// The sections and the fold that start closed (#2523), opened as a click
/// on their headers would.
fn open_all(ctx: &mut PanelContext) {
    for key in [
        "pcb_board_filter",
        "pcb_board_snap",
        "pcb_board_guides",
        "pcb_board_panel",
        "pcb_board_info_more",
    ] {
        ctx.collapsed_sections
            .insert(super::chrome::toggle_key(key, true));
    }
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
    let mut ctx = context(0);
    let texts_closed = texts(&ctx);
    open_all(&mut ctx);
    let texts = texts(&ctx);
    for wanted in [
        "Board",
        "mixed-tht-smd",
        "Size 50.000 × 40.000 mm",
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
        "PANEL",
        "OTHER",
    ] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    // #2523: the same sections and figures with the closed ones closed;
    // only the fold's Signal waits for it to open.
    for wanted in [
        "Size 50.000 × 40.000 mm",
        "50.000 mm",
        "2000.00 mm²",
        "SELECTION FILTER",
        "SNAP OPTIONS",
        "GUIDE MANAGER",
        "PANEL",
    ] {
        assert!(
            texts_closed.iter().any(|t| t == wanted),
            "{wanted} in {texts_closed:?}"
        );
    }
    assert!(
        !texts_closed.iter().any(|t| t == "Signal"),
        "{texts_closed:?}"
    );
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
    for gone in ["BOARD INFORMATION", "Edit board shape…"] {
        assert!(!texts.iter().any(|t| t == gone), "{gone} in {texts:?}");
    }
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
        Box::new(
            b.with_footprint(Some(facts))
                .with_component(Some(component)),
        )
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

/// #2353, #2383: the header's filter button counts the kinds that are on; a
/// kind turned off lowers the count and names it on hover. A click opens the
/// floating dropdown, never a second chip block in the panel.
#[test]
fn the_header_counts_the_kinds_that_are_on() {
    let mut ctx = context(0);
    open_all(&mut ctx);
    let total = PickKind::ALL.len();
    let texts_all = texts(&ctx);
    let all = format!("{total}/{total}");
    assert!(texts_all.contains(&all), "{texts_all:?}");
    let chips = texts_all.iter().filter(|t| *t == "3D Bodies").count();
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.filter.toggle(PickKind::Components);
    }
    let texts = texts(&ctx);
    let fewer = format!("{}/{total}", total - 1);
    assert!(texts.contains(&fewer), "{texts:?}");
    let width = DEFAULT_DOCK;
    let button = laid_out(&ctx, width)
        .into_iter()
        .find(|t| t.text == fewer)
        .expect("the filter button");
    let sent = crate::layout_probe::messages_on_click_at(
        super::super::properties::view_properties(&ctx),
        iced::Size::new(width, WINDOW.height),
        None,
        button.bounds.center(),
    );
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::PcbBoard(PcbBoardMsg::FilterMenuToggled))),
        "{sent:?}"
    );
    assert_eq!(
        chips, 1,
        "the chips show once, in the Selection Filter section"
    );
}

/// #2374: the Guide and Grid Managers list the board's guides and grids,
/// and the sheet fields are greyed while the sheet follows the board.
#[test]
fn the_managers_list_the_boards_guides_and_grids() {
    use signex_types::pcb_options::{BoardGrid, BoardGuide, GuideKind, MARK_COLORS};
    let mut ctx = context(0);
    open_all(&mut ctx);
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
    for wanted in ["+45°", "Fine pads", "Global Snap Grid", "Size From Board"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t.starts_with("No guides")),
        "{texts:?}"
    );
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

/// The width of the mockup's dock (#2523, mockup row 93).
const MOCKUP_DOCK: f32 = 300.0;

/// The texts from Board Information's title to the next section's.
fn board_information(
    texts: &[crate::layout_probe::LaidOutText],
) -> &[crate::layout_probe::LaidOutText] {
    let from = texts
        .iter()
        .position(|t| t.text == "BOARD INFORMATION")
        .expect("Board Information");
    let rest = &texts[from..];
    let to = rest
        .iter()
        .position(|t| t.text == "GRID MANAGER")
        .expect("Grid Manager");
    &rest[..to]
}

/// #2386, #2523: the Components, Top and Bottom tiles share one row, in
/// that order, at the mockup's dock width (three tiles a row); in the
/// default dock two tiles a row keep every value whole.
#[test]
fn the_component_tiles_share_a_row_and_values_fit_their_tiles() {
    let wide = laid_out(&context(0), MOCKUP_DOCK);
    let info = board_information(&wide);
    let y = |label: &str| crate::layout_probe::find(info, label).bounds.y;
    assert_eq!(y("Components"), y("Top"), "{info:?}");
    assert_eq!(y("Top"), y("Bottom"), "{info:?}");
    let x = |label: &str| crate::layout_probe::find(info, label).bounds.x;
    assert!(x("Components") < x("Top") && x("Top") < x("Bottom"));
    assert_eq!(y("Width"), y("Thickness"), "three a row: {info:?}");

    let narrow = laid_out(&context(0), DEFAULT_DOCK);
    let info = board_information(&narrow);
    let width = crate::layout_probe::find(info, "Width").bounds;
    let height = crate::layout_probe::find(info, "Height").bounds;
    assert_eq!(width.y, height.y, "two a row: {info:?}");
    let area = crate::layout_probe::find(info, "2000.00 mm²").bounds;
    assert!(area.x + area.width <= DEFAULT_DOCK, "{area:?}");
    let value = crate::layout_probe::find(info, "50.000 mm").bounds;
    assert!(
        value.x + value.width <= height.x,
        "{value:?} runs into Height"
    );
}

/// #2386: no label of the board panel wraps at the default dock width.
#[test]
fn no_board_panel_label_wraps_at_the_default_dock_width() {
    let mut ctx = context(0);
    open_all(&mut ctx);
    let texts = laid_out(&ctx, DEFAULT_DOCK);
    let one_line = crate::layout_probe::find(&texts, "Width").bounds.height;
    for label in [
        "Width",
        "Height",
        "Thickness",
        "Parts Area",
        "Unrouted nets",
        "Holes",
        "Hidden nets",
        "Keepouts",
        "DRC Violations",
        "Snap to",
        "Snapping",
        "Distance",
        "Axis Range",
        "Visible Grid",
        "Global Snap Grid",
        "Fast Grids",
        "Pour Names",
        "Designators",
        "Edit board shape…",
    ] {
        let found = crate::layout_probe::find(&texts, label);
        assert!(
            found.bounds.height < one_line * 1.5,
            "{label} wraps: {found:?}"
        );
    }
}

/// #2386: a board with no outline shows one muted "No board outline" line
/// in place of its width, height and area (#2523: in the size row, with no
/// Width, Height or Area tile).
#[test]
fn an_outline_less_board_says_so_on_one_line() {
    let mut ctx = context(0);
    let mut no_outline = board();
    no_outline.outline.clear();
    ctx.pcb_board = Some(Box::new(PcbBoardSummary::of(&no_outline, "b".into(), 0)));
    let texts = laid_out(&ctx, DEFAULT_DOCK);
    let line = crate::layout_probe::find(&texts, "No board outline");
    let thickness = crate::layout_probe::find(&texts, "Thickness");
    assert!(
        line.bounds.height < thickness.bounds.height * 2.0,
        "{line:?}"
    );
    let info = board_information(&texts);
    for gone in ["Width", "Height", "Area"] {
        assert!(!info.iter().any(|t| t.text == gone), "{gone} in {info:?}");
    }
    // No size readout: only the no-outline line.
    assert!(!texts.iter().any(|t| t.text.contains(" × ")), "{texts:?}");
}

/// #2386: the Primitives list leaves out kinds the board cannot hold and
/// counts the drawn areas (#2523: in the fold under the tiles, with the
/// layer counts and the hidden nets).
#[test]
fn the_primitives_card_counts_what_the_board_holds() {
    let mut ctx = context(0);
    open_all(&mut ctx);
    let texts = texts(&ctx);
    for wanted in [
        "Layers · Primitives & Others",
        "Layers",
        "Signal",
        "Fills",
        "Regions",
        "Keepouts",
        "Polygons",
        "Holes",
        "Hidden nets",
    ] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    assert!(!texts.iter().any(|t| t == "Not available"), "{texts:?}");
}

/// #2383: Add Preset sends its message from the laid-out panel; #2523: Run
/// DRC is on the Health Check tab, not on General.
#[test]
fn add_preset_and_run_drc_are_wired() {
    let mut ctx = context(0);
    open_all(&mut ctx);
    let window = iced::Size::new(DEFAULT_DOCK, WINDOW.height);
    let added = crate::layout_probe::messages_on_click(
        super::super::properties::view_properties(&ctx),
        window,
        "Add Preset",
    );
    assert!(
        added
            .iter()
            .any(|m| matches!(m, PanelMsg::PcbBoard(PcbBoardMsg::FilterPresetAdded))),
        "{added:?}"
    );
    let general = texts(&ctx);
    assert!(!general.iter().any(|t| t == "Run DRC"), "{general:?}");
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.tab = BoardTab::HealthCheck;
    }
    let run = crate::layout_probe::messages_on_click(
        super::super::properties::view_properties(&ctx),
        window,
        "Run DRC",
    );
    assert!(run.iter().any(|m| matches!(m, PanelMsg::RunDrc)), "{run:?}");
}

/// A context with one extra grid and one guide, as the managers test has.
fn managed(ctx: &mut PanelContext) -> uuid::Uuid {
    use signex_types::pcb_options::{BoardGrid, BoardGuide, GuideKind, MARK_COLORS};
    let id = uuid::Uuid::from_u128(2);
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.options.guides.push(BoardGuide::new(
            uuid::Uuid::from_u128(1),
            GuideKind::Vertical,
            Point { x: 2.5, y: 0.0 },
        ));
        board.options.grids.push(BoardGrid {
            uuid: id,
            name: "Fine pads".into(),
            step_mm: 0.1,
            origin: Point { x: 0.0, y: 0.0 },
            color: MARK_COLORS[1],
            components: true,
            non_components: false,
        });
    }
    id
}

/// #2386: the board's own grid card offers Edit only; an extra grid's card
/// offers Edit, Move Up, Move Down and Delete. No table header is drawn.
#[test]
fn the_global_grid_card_cannot_be_moved_or_deleted() {
    let mut ctx = context(0);
    let id = managed(&mut ctx);
    let shown = texts(&ctx);
    for gone in ["Priority", "Comp", "Non Comp", "Properties"] {
        assert!(!shown.iter().any(|t| t == gone), "{gone} in {shown:?}");
    }
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.grid_menu = Some(GridCard::Global);
    }
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "Edit"), "{shown:?}");
    for gone in ["Move Up", "Move Down", "Delete"] {
        assert!(!shown.iter().any(|t| t == gone), "{gone} in {shown:?}");
    }
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.grid_menu = Some(GridCard::Extra(id));
    }
    let shown = texts(&ctx);
    for wanted in ["Edit", "Move Up", "Move Down", "Delete"] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted} in {shown:?}");
    }
}

/// #2386: the grid and guide cards, the Other rows and the Sheet card keep
/// every text on one line at the default dock width.
#[test]
fn the_manager_cards_do_not_wrap_at_the_default_dock_width() {
    let mut ctx = context(0);
    open_all(&mut ctx);
    managed(&mut ctx);
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.guide_add_open = true;
    }
    let texts = laid_out(&ctx, DEFAULT_DOCK);
    let one_line = crate::layout_probe::find(&texts, "Parts").bounds.height;
    for label in [
        "Global Snap Grid",
        "Fine pads",
        "Parts",
        "Other",
        "+ Add Grid",
        "Vertical",
        "+ Add Guide",
        "Place on Board",
        "Pour Names",
        "Designators",
        "Size From Board",
        "Width",
    ] {
        let found = crate::layout_probe::find(&texts, label);
        assert!(
            found.bounds.height < one_line * 1.5,
            "{label} wraps: {found:?}"
        );
    }
}

/// #2386: the pour-name choices read as example names, and the Sheet's
/// figures are read-only while Size From Board is on.
#[test]
fn pour_names_read_as_examples_and_the_sheet_follows_the_board() {
    let ctx = context(0);
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t.starts_with("GND_")), "{shown:?}");
    assert!(!shown.iter().any(|t| t.contains("NET NAME")), "{shown:?}");
}

/// #2262: at a narrow dock width the three Snapping choices sit side by side
/// without running into each other (they used to wrap over one another in
/// the field column).
#[test]
fn the_snapping_choices_do_not_overlap_in_a_narrow_dock() {
    let narrow = iced::Size::new(220.0, 4000.0);
    let mut ctx = context(0);
    open_all(&mut ctx);
    let laid = lay_out_texts(super::super::properties::view_properties(&ctx), narrow);
    // The last of each: the Snap Options header names the scope in use too.
    let bounds: Vec<iced::Rectangle> = ["All Layers", "Current Layer", "Off"]
        .iter()
        .map(|label| {
            laid.iter()
                .rev()
                .find(|t| t.text == *label)
                .unwrap_or_else(|| panic!("{label:?} not laid out"))
                .bounds
        })
        .collect();
    for (i, a) in bounds.iter().enumerate() {
        assert!(
            a.x + a.width <= narrow.width + 0.5,
            "{a:?} past the dock edge"
        );
        for b in &bounds[i + 1..] {
            assert!(a.intersection(b).is_none(), "{a:?} overlaps {b:?}");
        }
    }
}

/// A board without a panel shows no Panel section or Panelize button; the
/// section comes with a panel (Caner, 2026-11-21).
#[test]
fn the_panel_section_shows_only_on_a_panelized_board() {
    let mut ctx = context(0);
    open_all(&mut ctx);
    let shown = texts(&ctx);
    for gone in ["Panel", "Panelize Board"] {
        assert!(!shown.iter().any(|t| t == gone), "{gone} in {shown:?}");
    }
}

/// #2523 (mockup row 93): Selection Filter, Snap Options, Guide Manager and Panel
/// start closed with a summary in their header, and the fold of layer and
/// primitive counts too; a click on a header opens it.
#[test]
fn closed_sections_show_their_summary_and_open_on_a_click() {
    let ctx = context(0);
    let shown = texts(&ctx);
    for gone in [
        "3D Bodies",
        "Pad Centers",
        "+ Add Guide",
        "Panelize Board",
        "Signal",
    ] {
        assert!(!shown.iter().any(|t| t == gone), "{gone} in {shown:?}");
    }
    let total = PickKind::ALL.len();
    let count = format!("{total}/{total}");
    // The header's filter button and the Selection Filter's header pill.
    assert_eq!(
        shown.iter().filter(|t| **t == count).count(),
        2,
        "{shown:?}"
    );
    for summary in ["All Layers"] {
        assert_eq!(
            shown.iter().filter(|t| *t == summary).count(),
            1,
            "{summary} in {shown:?}"
        );
    }
    let window = iced::Size::new(DEFAULT_DOCK, WINDOW.height);
    let sent = crate::layout_probe::messages_on_click(
        super::super::properties::view_properties(&ctx),
        window,
        "SELECTION FILTER",
    );
    let key = super::chrome::toggle_key("pcb_board_filter", true);
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::ToggleSection(k) if *k == key)),
        "{sent:?}"
    );
    let mut opened = ctx.clone();
    opened.collapsed_sections.insert(key);
    let shown = texts(&opened);
    assert!(shown.iter().any(|t| t == "3D Bodies"), "{shown:?}");
}

/// #2523 (mockup row 93): the size is read from the board shape and
/// read-only; the size row says so and offers Edit board shape….
#[test]
fn the_size_row_reads_the_board_shape_and_offers_to_edit_it() {
    let ctx = context(0);
    let shown = texts(&ctx);
    for wanted in [
        "Size 50.000 × 40.000 mm",
        "· from board shape",
        "Edit board shape…",
    ] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted} in {shown:?}");
    }
    let window = iced::Size::new(DEFAULT_DOCK, WINDOW.height);
    let sent = crate::layout_probe::messages_on_click(
        super::super::properties::view_properties(&ctx),
        window,
        "Edit board shape…",
    );
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::PcbBoard(PcbBoardMsg::BoardShapeEditPressed))),
        "{sent:?}"
    );
}

/// #2523: an outline that is not a rectangle reads as its bounding box.
#[test]
fn a_shaped_outline_reads_as_a_bounding_box() {
    let p = |x: f64, y: f64| Point { x, y };
    let mut shaped = board();
    // An L: the extent is still 50 x 40 mm.
    shaped.outline = vec![
        p(10.0, 5.0),
        p(60.0, 5.0),
        p(60.0, 25.0),
        p(35.0, 25.0),
        p(35.0, 45.0),
        p(10.0, 45.0),
    ];
    let summary = PcbBoardSummary::of(&shaped, "b".into(), 0);
    assert!(!summary.outline_is_rectangle);
    let mut ctx = context(0);
    ctx.pcb_board = Some(Box::new(summary));
    let shown = texts(&ctx);
    assert!(
        shown.iter().any(|t| t == "Bounding box 50.000 × 40.000 mm"),
        "{shown:?}"
    );
    assert!(PcbBoardSummary::of(&board(), "b".into(), 0).outline_is_rectangle);
    // A closing vertex repeated at the end is still a rectangle.
    let mut closed = board();
    closed.outline.push(p(10.0, 5.0));
    assert!(PcbBoardSummary::of(&closed, "b".into(), 0).outline_is_rectangle);
}

/// #2523 (mockup row 176): Other links to the Layer Stack for the board's
/// materials, and Board Information's header holds Reports.
#[test]
fn the_materials_link_and_reports_send_their_messages() {
    let ctx = context(0);
    let window = iced::Size::new(MOCKUP_DOCK, WINDOW.height);
    let view = || super::super::properties::view_properties(&ctx);
    let link = "Board materials & finish → Open Layer Stack";
    let sent = crate::layout_probe::messages_on_click(view(), window, link);
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::PcbBoard(PcbBoardMsg::LayerStackOpened))),
        "{sent:?}"
    );
    let sent = crate::layout_probe::messages_on_click(view(), window, "Reports");
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::PcbBoard(PcbBoardMsg::ReportsOpened))),
        "{sent:?}"
    );
    let laid = laid_out(&ctx, MOCKUP_DOCK);
    let title = crate::layout_probe::find(&laid, "BOARD INFORMATION").bounds;
    let reports = crate::layout_probe::find(&laid, "Reports").bounds;
    assert!(
        (title.center_y() - reports.center_y()).abs() < title.height,
        "Reports sits in the header: {title:?} {reports:?}"
    );
}

/// #2523: the pointer over the Width tile reports the Width field, so the
/// illustration lights its callout.
#[test]
fn hovering_the_width_tile_reports_its_field() {
    use signex_widgets::property_illustration::IllustrationMsg;
    let ctx = context(0);
    let laid = laid_out(&ctx, MOCKUP_DOCK);
    let info = board_information(&laid);
    let tile = crate::layout_probe::find(info, "Width").bounds;
    let sent = crate::layout_probe::messages_for_moves(
        super::super::properties::view_properties(&ctx),
        iced::Size::new(MOCKUP_DOCK, WINDOW.height),
        &[iced::Point::ORIGIN, tile.center()],
    );
    assert!(
        sent.iter().any(|m| matches!(
            m,
            PanelMsg::PcbBoard(PcbBoardMsg::IllustrationHovered(
                IllustrationMsg::FieldHovered(field)
            )) if *field == BOARD_WIDTH
        )),
        "{sent:?}"
    );
}

/// #2523 (mockup row 93): the Health Check tab is one card per check, with Run DRC
/// and Show results on the design rules card; neither check has run here.
#[test]
fn the_health_check_tab_has_a_card_per_check() {
    let mut ctx = context(0);
    if let Some(board) = ctx.pcb_board.as_mut() {
        board.view.tab = BoardTab::HealthCheck;
    }
    let shown = texts(&ctx);
    for wanted in [
        "Design rules",
        "Run DRC",
        "Show results",
        "Electrical rules",
        "From the project's schematic check",
    ] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted} in {shown:?}");
    }
    assert_eq!(
        shown.iter().filter(|t| *t == "Not run").count(),
        2,
        "{shown:?}"
    );
    for gone in ["Search", "SNAP OPTIONS", "Edit board shape…"] {
        assert!(!shown.iter().any(|t| t == gone), "{gone} in {shown:?}");
    }
    let window = iced::Size::new(DEFAULT_DOCK, WINDOW.height);
    let sent = crate::layout_probe::messages_on_click(
        super::super::properties::view_properties(&ctx),
        window,
        "Show results",
    );
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::PcbBoard(PcbBoardMsg::DrcResultsOpened))),
        "{sent:?}"
    );
}
