//! Mockup row 116, Board Print / PDF (decisions 2026-11-16 and 2026-11-19):
//! the layer sets and the page plan, Pages, Scale, Ink, the Show boxes,
//! Cancel, Export PDF and Export Assembly SVG. The board is never edited.

use signex_app::app::{BoardOutputMsg, BoardPrintFlag, Message};
use signex_output::{BoardLayerSet, BoardPrintPages, BoardPrintScale};

use super::support::{Bench, audit_board, open_board};

fn send(b: &mut Bench, msg: BoardOutputMsg) {
    b.send(Message::from(msg));
}

fn opened() -> Bench {
    let mut b = open_board(audit_board());
    b.command("print");
    b
}

fn plan_len(b: &Bench) -> usize {
    b.print().options.page_plan().len()
}

fn untick_every_set(b: &mut Bench) {
    for set in b.print().options.sets.clone() {
        send(b, BoardOutputMsg::PrintSetToggled(set));
    }
}

#[test]
fn the_dialog_opens_on_top_bottom_and_outline_one_page_each() {
    let b = opened();
    let options = &b.print().options;
    assert_eq!(
        options.sets,
        [
            BoardLayerSet::Top,
            BoardLayerSet::Bottom,
            BoardLayerSet::Mechanical
        ]
    );
    assert_eq!(options.pages, BoardPrintPages::PerSet);
    assert_eq!(options.scale, BoardPrintScale::Fit);
    assert!(!options.monochrome);
    assert_eq!(plan_len(&b), 3);
    assert!(!b.dirty(), "opening the dialog is no edit");
}

#[test]
fn ticking_a_layer_set_adds_its_page_and_unticking_takes_it_away() {
    let mut b = opened();
    send(
        &mut b,
        BoardOutputMsg::PrintSetToggled(BoardLayerSet::AssemblyTop),
    );
    assert_eq!(plan_len(&b), 4);
    send(
        &mut b,
        BoardOutputMsg::PrintSetToggled(BoardLayerSet::AssemblyTop),
    );
    assert_eq!(plan_len(&b), 3);
}

#[test]
fn all_sets_on_one_page_folds_the_plan() {
    let mut b = opened();
    send(
        &mut b,
        BoardOutputMsg::PrintPagesPicked(BoardPrintPages::Composite),
    );
    assert_eq!(plan_len(&b), 1);
    send(
        &mut b,
        BoardOutputMsg::PrintPagesPicked(BoardPrintPages::PerSet),
    );
    assert_eq!(plan_len(&b), 3);
}

#[test]
fn only_the_bottom_page_carries_the_mirrored_tag() {
    let b = opened();
    for (sets, mirrored) in b.print().options.page_plan() {
        assert_eq!(mirrored, sets == [BoardLayerSet::Bottom], "{sets:?}");
    }
}

#[test]
fn mirror_the_bottom_side_is_deaf_without_a_bottom_set_and_keeps_its_value() {
    let mut b = opened();
    let mirror = BoardOutputMsg::PrintFlagToggled(BoardPrintFlag::MirrorBottom);
    send(&mut b, mirror.clone());
    assert!(!b.print().options.mirror_bottom, "a bottom set: it works");
    send(&mut b, mirror.clone());
    assert!(b.print().options.mirror_bottom);
    send(
        &mut b,
        BoardOutputMsg::PrintSetToggled(BoardLayerSet::Bottom),
    );
    send(&mut b, mirror);
    assert!(b.print().options.mirror_bottom, "no bottom set: deaf");
}

#[test]
fn scale_and_ink_set_their_options() {
    let mut b = opened();
    send(
        &mut b,
        BoardOutputMsg::PrintScalePicked(BoardPrintScale::OneToOne),
    );
    assert_eq!(b.print().options.scale, BoardPrintScale::OneToOne);
    send(&mut b, BoardOutputMsg::PrintMonochromePicked(true));
    assert!(b.print().options.monochrome);
    send(&mut b, BoardOutputMsg::PrintMonochromePicked(false));
    assert!(!b.print().options.monochrome);
}

#[test]
fn the_show_boxes_turn_their_settings_off_and_on() {
    let mut b = opened();
    for flag in [
        BoardPrintFlag::Outline,
        BoardPrintFlag::DrillMarks,
        BoardPrintFlag::TitleBlock,
    ] {
        assert!(flag.get(&b.print().options), "{flag:?} starts on");
        send(&mut b, BoardOutputMsg::PrintFlagToggled(flag));
        assert!(!flag.get(&b.print().options), "{flag:?}");
        send(&mut b, BoardOutputMsg::PrintFlagToggled(flag));
        assert!(flag.get(&b.print().options), "{flag:?}");
    }
}

#[test]
fn with_no_set_ticked_export_is_refused_and_writes_nothing() {
    let mut b = opened();
    untick_every_set(&mut b);
    assert_eq!(plan_len(&b), 0, "the plan shows its empty state");
    let out = b.out("empty.pdf");
    b.answer(&out);
    send(&mut b, BoardOutputMsg::PdfExportRequested);
    assert!(!out.exists(), "nothing is written");
    assert!(b.has_notice(), "the refusal says why");
}

/// Decision 2026-11-16: the dialog keeps its options for the next opening
/// in the session, so Cancel writes nothing and edits nothing, and the
/// options stay.
#[test]
fn cancel_writes_nothing_and_the_next_opening_keeps_the_options() {
    let mut b = opened();
    b.leaves_document(|b| {
        send(
            b,
            BoardOutputMsg::PrintScalePicked(BoardPrintScale::OneToOne),
        );
        send(b, BoardOutputMsg::Closed);
    });
    assert!(b.app.ui_state.board_output.dialog.is_none(), "it closed");
    assert!(!b.out("audit.pdf").exists());
    b.command("print");
    assert_eq!(b.print().options.scale, BoardPrintScale::OneToOne);
}

#[test]
fn export_pdf_writes_the_file_and_the_dialog_stays() {
    let mut b = opened();
    let out = b.out("board.pdf");
    b.answer(&out);
    b.leaves_document(|b| send(b, BoardOutputMsg::PdfExportRequested));
    let bytes = std::fs::read(&out).expect("the PDF is written");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(b.app.ui_state.board_output.dialog.is_some(), "it stays");
}

#[test]
fn a_cancelled_pdf_save_writes_nothing_and_keeps_the_dialog() {
    let mut b = opened();
    b.cancel_dialog();
    b.leaves_document(|b| send(b, BoardOutputMsg::PdfExportRequested));
    assert!(b.app.ui_state.board_output.dialog.is_some(), "it stays");
    assert!(!b.out("audit.pdf").exists());
}

#[test]
fn export_assembly_svg_writes_a_drawing_for_each_side() {
    let mut b = opened();
    let folder = b.dir.path().to_path_buf();
    b.answer(&folder);
    b.leaves_document(|b| send(b, BoardOutputMsg::SvgExportRequested));
    for side in ["top", "bottom"] {
        let file = folder.join(format!("audit-assembly-{side}.svg"));
        let svg = std::fs::read_to_string(&file).expect("the drawing is written");
        assert!(svg.contains("<svg "), "{side}");
    }
}
