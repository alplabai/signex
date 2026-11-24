//! #2852, mockup row 179c: the Import DXF card's three steps laid out. The
//! mapping ends in Next: check profiles; the check names each problem with
//! the heal tolerance and the two ways to import; the result names what
//! landed and closes with Done. The preview rings each problem.

use std::sync::Arc;

use iced::Size;
use signex_engine::pcb::dxf::DxfSource;
use signex_i18n::I18n;
use signex_sketch::plane::PlaneId;
use signex_types::pcb_origin::BoardOrigin;
use signex_types::theme::{ThemeId, theme_tokens};

use super::overlay;
use super::preview::{layer_caption, marks};
use crate::app::state::{DxfImportMode, SketchDxfDialog, SketchDxfMsg};
use crate::app::{DxfImportMsg, Message, PcbMsg};
use crate::layout_probe::{lay_out_inputs, lay_out_texts, messages_on_click};

const WINDOW: Size = Size::new(1400.0, 900.0);

/// The mockup's lid: a 60 x 40 outline open 0.04 mm at (57.00, 0.00), its
/// top edge drawn as two overlapping lines, four holes and a text layer.
const LID: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n0\n11\n56.96\n21\n0\n\
0\nLINE\n8\nBOARD\n10\n57\n20\n0\n11\n60\n21\n0\n\
0\nLINE\n8\nBOARD\n10\n60\n20\n0\n11\n60\n21\n40\n\
0\nLINE\n8\nBOARD\n10\n60\n20\n40\n11\n26\n21\n40\n\
0\nLINE\n8\nBOARD\n10\n30\n20\n40\n11\n0\n21\n40\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n40\n11\n0\n21\n0\n\
0\nCIRCLE\n8\nHOLES\n10\n3.5\n20\n3.5\n40\n1.6\n\
0\nCIRCLE\n8\nHOLES\n10\n56.5\n20\n3.5\n40\n1.6\n\
0\nCIRCLE\n8\nHOLES\n10\n56.5\n20\n36.5\n40\n1.6\n\
0\nCIRCLE\n8\nHOLES\n10\n3.5\n20\n36.5\n40\n1.6\n\
0\nTEXT\n8\nTEXT\n10\n0\n20\n0\n\
0\nENDSEC\n0\nEOF\n";

fn lid() -> SketchDxfDialog {
    let source = Arc::new(DxfSource::read(LID).expect("the file reads"));
    SketchDxfDialog::open(
        "enclosure-lid.dxf".into(),
        source,
        BoardOrigin::default(),
        false,
    )
}

fn checking() -> SketchDxfDialog {
    let mut dialog = lid();
    dialog.apply(SketchDxfMsg::CheckRequested);
    dialog
}

/// Every laid-out text of the card, without the isolation marks.
fn texts(dialog: &SketchDxfDialog) -> Vec<String> {
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    lay_out_texts(overlay(dialog, &tokens, &i18n), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn click(dialog: &SketchDxfDialog, label: &str) -> Vec<Message> {
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    messages_on_click(overlay(dialog, &tokens, &i18n), WINDOW, label)
}

fn sent(messages: &[Message], event: &SketchDxfMsg) -> bool {
    messages.iter().any(|m| {
        matches!(m, Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::SketchDialog(e))) if e == event)
    })
}

#[test]
fn the_mapping_ends_in_next_check_profiles() {
    let dialog = lid();
    let shown = texts(&dialog);
    assert!(
        shown.iter().any(|t| t == "Next: check profiles"),
        "{shown:?}"
    );
    assert!(!shown.iter().any(|t| t == "Import"), "{shown:?}");
    let messages = click(&dialog, "Next: check profiles");
    assert!(sent(&messages, &SketchDxfMsg::CheckRequested));
}

#[test]
fn the_check_names_each_problem_and_the_closed_circles() {
    let shown = texts(&checking());
    for line in [
        "2 problems in 5 profiles",
        "Board outline open: gap 0.04 mm at (56.98, 0.00)",
        "2 overlapping segments at (28.00, 40.00)",
        "4 circles closed",
        "Heal under",
        "Heal and merge",
        "Import as is",
    ] {
        assert!(shown.iter().any(|t| t == line), "{line:?} in {shown:?}");
    }
}

#[test]
fn heal_under_shows_the_one_heal_tolerance() {
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let dialog = checking();
    let inputs = lay_out_inputs(overlay(&dialog, &tokens, &i18n), WINDOW);
    assert!(inputs.iter().any(|i| i.text == "0.05"), "{inputs:?}");
}

#[test]
fn the_check_buttons_import_healed_or_as_drawn() {
    let dialog = checking();
    let heal = click(&dialog, "Heal and merge");
    assert!(sent(
        &heal,
        &SketchDxfMsg::Confirmed(DxfImportMode::HealAndMerge)
    ));
    let as_is = click(&dialog, "Import as is");
    assert!(sent(&as_is, &SketchDxfMsg::Confirmed(DxfImportMode::AsIs)));
}

#[test]
fn the_result_names_what_landed_and_done_closes_it() {
    let mut dialog = checking();
    let import = dialog
        .build_as(PlaneId::new(), DxfImportMode::HealAndMerge)
        .expect("it builds");
    dialog.land(SketchDxfDialog::result_of(
        DxfImportMode::HealAndMerge,
        &import,
    ));
    let shown = texts(&dialog);
    for line in [
        "Imported: 1 outline, 4 holes",
        "1 gap closed (end points merged), 1 overlapping segment merged into one line.",
        "The DXF is not linked: a later change to the file does not reach this sketch.",
        "Done",
    ] {
        assert!(shown.iter().any(|t| t == line), "{line:?} in {shown:?}");
    }
    assert!(
        shown
            .iter()
            .any(|t| t.ends_with("constraints added; DOF 17 left.")),
        "{shown:?}"
    );
    assert!(sent(&click(&dialog, "Done"), &SketchDxfMsg::Dismissed));
}

#[test]
fn the_preview_rings_the_gap_and_the_overlap_and_names_the_layers() {
    let dialog = checking();
    let i18n = I18n::en_us();
    let labels: Vec<String> = marks(&dialog, &i18n)
        .into_iter()
        .map(|m| m.label.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    assert_eq!(labels, ["gap 0.04 mm", "2 overlapping segments"]);
    let caption = layer_caption(&dialog, &i18n).replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(
        caption,
        "layer BOARD → Board outline · HOLES → Mounting hole · TEXT ignored"
    );
}
