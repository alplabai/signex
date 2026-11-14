//! The Component view (#2354): the header names the selection, the search
//! leaves rows out, fields the board cannot store are still listed, and the
//! Parameters tab lists the shared parameters.

use signex_types::pcb_layer::BoardSide;

use super::*;
use crate::app::Signex;
use crate::layout_probe::lay_out_texts;
use crate::panels::PcbBoardSummary;

const WINDOW: iced::Size = iced::Size::new(360.0, 2400.0);

/// One unlocked top-side component at (10, 5) with an MPN parameter.
pub(crate) fn one_component(designator: &str, footprint: &str) -> PcbComponentFacts {
    PcbComponentFacts {
        board: "board.snxpcb".into(),
        footprints: vec![Uuid::from_u128(1)],
        x: Shared::Same(10.0),
        y: Shared::Same(5.0),
        rotation: Shared::Same(90.0),
        locked: Shared::Same(false),
        side: Shared::Same(BoardSide::Top),
        designator: Shared::Same(designator.into()),
        designator_shown: Some(Shared::Same(true)),
        comment: Shared::Same("10k".into()),
        comment_shown: Some(Shared::Same(false)),
        area_mm2: Shared::Same(Some(2.5)),
        footprint_name: Shared::Same(footprint.into()),
        library: Shared::Same(None),
        schematic_path: Shared::Same(None),
        parameters: vec![ParameterFacts {
            key: "MPN".into(),
            value: Shared::Same("RC0603".into()),
        }],
        drafts: BTreeMap::new(),
        read_only: false,
        search: String::new(),
    }
}

fn texts_of(facts: PcbComponentFacts, tab: usize) -> Vec<String> {
    let (app, _boot) = Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.has_pcb = true;
    ctx.properties_tab = tab;
    let summary = PcbBoardSummary::default().with_component(Some(facts));
    ctx.pcb_board = Some(summary);
    let board = ctx.pcb_board.clone().unwrap();
    lay_out_texts(
        super::super::pcb_board_properties::view(&ctx, &board),
        WINDOW,
    )
    .into_iter()
    .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
    .collect()
}

fn has(texts: &[String], wanted: &str) -> bool {
    texts.iter().any(|t| t == wanted)
}

#[test]
fn shared_values_are_kept_and_differing_ones_are_mixed() {
    assert_eq!(Shared::of([1.0, 1.0]), Some(Shared::Same(1.0)));
    assert_eq!(Shared::of([1.0, 2.0]), Some(Shared::Mixed));
    assert_eq!(Shared::<f64>::of([]), None);
}

#[test]
fn one_component_lists_every_section_and_its_unstored_rows() {
    let texts = texts_of(one_component("R7", "R_0603"), 0);
    for wanted in [
        "Component",
        "R7",
        "X (mm)",
        "Rotation (°)",
        "Locked",
        "Layer",
        "Top Layer",
        "Bottom Layer",
        "Designator",
        "Comment",
        "Area",
        "2.500 mm²",
        "Type",
        "Height",
        "3D Body Opacity",
        "Footprint Name",
        "R_0603",
        "Library",
        "Not from a library",
        "Enable Pin Swapping",
        "Hierarchical Path",
        "Not linked to a schematic part",
        "Channel Offset",
    ] {
        assert!(has(&texts, wanted), "{wanted} in {texts:?}");
    }
}

#[test]
fn several_components_are_counted_in_the_header() {
    let mut facts = one_component("R7", "R_0603");
    facts.footprints.push(Uuid::from_u128(2));
    facts.designator = Shared::Mixed;
    facts.x = Shared::Mixed;
    let texts = texts_of(facts, 0);
    assert!(has(&texts, "2 components"), "{texts:?}");
    assert!(!has(&texts, "R7"), "{texts:?}");
}

#[test]
fn the_search_leaves_out_rows_whose_label_does_not_match() {
    let mut facts = one_component("R7", "R_0603");
    facts.search = "rot".into();
    let texts = texts_of(facts, 0);
    assert!(has(&texts, "Rotation (°)"), "{texts:?}");
    for gone in ["X (mm)", "Layer", "Footprint Name", "Channel Offset"] {
        assert!(!has(&texts, gone), "{gone} in {texts:?}");
    }
}

#[test]
fn the_parameters_tab_lists_the_shared_parameters() {
    let texts = texts_of(one_component("R7", "R_0603"), 1);
    assert!(has(&texts, "MPN"), "{texts:?}");
    assert!(!has(&texts, "X (mm)"), "{texts:?}");
    let mut bare = one_component("R7", "R_0603");
    bare.parameters.clear();
    assert!(has(&texts_of(bare, 1), "No parameters"));
}

#[test]
fn placement_edits_stop_while_any_component_is_locked() {
    let mut facts = one_component("R7", "R_0603");
    assert!(facts.is_editable());
    facts.locked = Shared::Mixed;
    assert!(!facts.is_editable());
    facts.locked = Shared::Same(false);
    facts.read_only = true;
    assert!(!facts.is_editable());
}
