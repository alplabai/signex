//! Mockup row 153, the Differential Pair Classes dialog: it edits a copy, OK
//! is one project step, Cancel drops the copy.

use signex_app::app::{
    DiffPairClassesMsg, DiffPairClassesState, Message, PairClassField, PairClassProblem,
    PairMemberRow,
};
use signex_types::diff_pair_class::DiffPairClass;

use super::support::{ProjectBench, empty_sheet, project_bench};

fn dialog_bench(classes: &[&str]) -> ProjectBench {
    let mut b = project_bench(empty_sheet(), empty_sheet());
    b.data_mut().schematic_design.diff_pair_classes =
        classes.iter().map(|n| DiffPairClass::named(*n)).collect();
    b.post(Message::from(DiffPairClassesMsg::Requested));
    assert!(
        b.app.ui_state.diff_pair_classes.is_some(),
        "the dialog opens"
    );
    b
}

fn send(b: &mut ProjectBench, msg: DiffPairClassesMsg) {
    b.post(Message::from(msg));
}

fn dialog(b: &ProjectBench) -> &DiffPairClassesState {
    b.app
        .ui_state
        .diff_pair_classes
        .as_deref()
        .expect("the dialog is open")
}

fn stored(b: &ProjectBench) -> Vec<DiffPairClass> {
    b.data().schematic_design.diff_pair_classes.clone()
}

#[test]
fn m153_class_edits_wait_for_ok_which_is_one_project_step() {
    let mut b = dialog_bench(&["USB"]);
    let before = stored(&b);
    for (field, text) in [
        (PairClassField::Impedance, "90"),
        (PairClassField::Tolerance, "10"),
        (PairClassField::Width, "0.2"),
        (PairClassField::Gap, "0.15"),
    ] {
        send(
            &mut b,
            DiffPairClassesMsg::FieldEdited(field, text.to_string()),
        );
    }
    send(&mut b, DiffPairClassesMsg::PairBySuffixToggled(false));
    assert_eq!(stored(&b), before, "nothing is applied before OK");
    assert!(!b.is_dirty(&b.file));

    send(&mut b, DiffPairClassesMsg::OkRequested);
    assert!(b.app.ui_state.diff_pair_classes.is_none(), "OK closes");
    let class = &stored(&b)[0];
    assert_eq!(class.impedance_ohm, Some(90.0));
    assert_eq!(class.impedance_tolerance_pct, Some(10.0));
    assert_eq!(class.width_mm, Some(0.2));
    assert_eq!(class.gap_mm, Some(0.15));
    assert!(!class.pair_by_suffix);
    assert!(b.is_dirty(&b.file));

    assert!(b.undo_project());
    assert_eq!(stored(&b), before);
    assert!(!b.project_can_undo(), "it was one step");
}

#[test]
fn m153_cancel_drops_every_class_edit() {
    let mut b = dialog_bench(&["USB"]);
    let before = stored(&b);
    send(&mut b, DiffPairClassesMsg::ClassAdded);
    send(
        &mut b,
        DiffPairClassesMsg::FieldEdited(PairClassField::Width, "0.2".to_string()),
    );
    send(&mut b, DiffPairClassesMsg::CancelRequested);
    assert!(b.app.ui_state.diff_pair_classes.is_none());
    assert_eq!(stored(&b), before);
    assert!(!b.is_dirty(&b.file));
    assert!(!b.project_can_undo());
}

#[test]
fn m153_a_bad_class_value_is_refused_with_the_text_kept() {
    let mut b = dialog_bench(&["USB"]);
    for bad in ["wide", "-1"] {
        send(
            &mut b,
            DiffPairClassesMsg::FieldEdited(PairClassField::Width, bad.to_string()),
        );
        assert_eq!(dialog(&b).classes[0].width_mm, None, "{bad:?}");
        assert_eq!(dialog(&b).draft(PairClassField::Width), Some(bad));
    }
}

#[test]
fn m153_ok_with_a_repeated_or_blank_name_stays_open_and_says_why() {
    let mut b = dialog_bench(&["USB"]);
    let before = stored(&b);
    send(&mut b, DiffPairClassesMsg::ClassAdded);
    send(&mut b, DiffPairClassesMsg::NameEdited("USB".to_string()));
    send(&mut b, DiffPairClassesMsg::OkRequested);
    assert_eq!(dialog(&b).problem, Some(PairClassProblem::DuplicateName));
    send(&mut b, DiffPairClassesMsg::NameEdited("  ".to_string()));
    send(&mut b, DiffPairClassesMsg::OkRequested);
    assert_eq!(dialog(&b).problem, Some(PairClassProblem::BlankName));
    assert_eq!(stored(&b), before, "nothing was applied");
}

#[test]
fn m153_the_row_menu_duplicates_renames_and_deletes_a_free_class() {
    let mut b = dialog_bench(&["USB"]);
    send(&mut b, DiffPairClassesMsg::ClassDuplicated(0));
    assert_eq!(dialog(&b).classes[1].name, "USB_2");
    assert_eq!(dialog(&b).selected, 1, "the copy is selected");
    send(&mut b, DiffPairClassesMsg::NameEdited("HDMI".to_string()));
    assert_eq!(dialog(&b).classes[1].name, "HDMI");
    send(&mut b, DiffPairClassesMsg::ClassDeleted(1));
    assert_eq!(dialog(&b).classes.len(), 1);
}

#[test]
fn m153_a_class_with_pairs_keeps_its_name_and_cannot_be_deleted() {
    let mut b = dialog_bench(&["USB"]);
    if let Some(state) = b.app.ui_state.diff_pair_classes.as_mut() {
        state.members = vec![PairMemberRow {
            class: "USB".to_string(),
            pair: "USB_D".to_string(),
            positive: "USB_DP".to_string(),
            negative: "USB_DM".to_string(),
            sheet: "root.snxsch".to_string(),
        }];
    }
    send(&mut b, DiffPairClassesMsg::NameEdited("USB3".to_string()));
    assert_eq!(dialog(&b).classes[0].name, "USB");
    send(&mut b, DiffPairClassesMsg::ClassDeleted(0));
    assert_eq!(dialog(&b).classes.len(), 1);
}
