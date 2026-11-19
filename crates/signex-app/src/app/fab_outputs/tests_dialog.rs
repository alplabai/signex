//! m115: the grouped kinds and their counts, the ODB++ folder form that
//! waits for ODB++, the time a run finished, Open folder, and the notes'
//! severities.

use std::path::PathBuf;

use signex_output::fab::FabNote;

use super::{
    BoardNote, FabGroup, FabOutput, FabOutputKinds, FabOutputsMsg, FabOutputsState, FabWritten,
    NoteSeverity, outcome_notes, ticked_count,
};
use crate::app::{Message, Signex};

/// The kinds of the m115 mockup: three fabrication files, pick and place
/// and the assembly PDF.
pub(super) fn mockup_kinds() -> FabOutputKinds {
    FabOutputKinds {
        gerber: true,
        drill: true,
        centroid: true,
        test_netlist: true,
        ipc2581: false,
        odb: false,
        odb_folder: false,
        assembly_pdf: true,
        assembly_svg: false,
        step: false,
    }
}

/// An app with the dialog open on a board no project names, so its ticks
/// are the dialog's own.
pub(super) fn app_with_dialog(kinds: FabOutputKinds) -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut state = FabOutputsState::new(PathBuf::from("dcdc.snxpcb"), "dcdc.snxpcb".into());
    state.kinds = kinds;
    app.ui_state.fab_outputs = Some(Box::new(state));
    app
}

fn send(app: &mut Signex, msg: FabOutputsMsg) -> iced::Task<Message> {
    app.update(Message::from(msg))
}

fn shown(app: &Signex) -> &FabOutputsState {
    app.ui_state.fab_outputs.as_deref().expect("the dialog")
}

pub(super) fn written(stale: &[&str]) -> FabWritten {
    FabWritten {
        files: vec!["dcdc-job.gbrjob".into(); 14],
        folder: PathBuf::from("fab"),
        notes: vec![FabNote::ZonesNotFilled { count: 1 }],
        board_notes: Vec::new(),
        stale: stale.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn every_kind_sits_in_exactly_one_group() {
    for kind in FabOutput::ALL {
        let homes = FabGroup::ALL
            .iter()
            .filter(|group| group.kinds().contains(&kind))
            .count();
        assert_eq!(homes, 1, "{kind:?}");
    }
    let listed: Vec<FabGroup> = FabGroup::COLUMNS.into_iter().flatten().collect();
    assert_eq!(listed, FabGroup::ALL, "the columns hold every group once");
}

#[test]
fn the_group_counts_and_the_total_follow_the_ticks() {
    let mut kinds = mockup_kinds();
    let counts = |k: &FabOutputKinds| FabGroup::ALL.map(|g| g.count(k));
    assert_eq!(counts(&kinds), [(3, 3), (0, 2), (2, 3), (0, 1)]);
    assert_eq!(ticked_count(&kinds), 5, "the footer counts the ticks");
    FabOutput::Odb.toggle(&mut kinds);
    kinds.odb_folder = true;
    assert_eq!(counts(&kinds)[1], (1, 2));
    assert_eq!(ticked_count(&kinds), 6, "the folder form is not a kind");
    FabOutput::Gerber.toggle(&mut kinds);
    assert_eq!(counts(&kinds)[0], (2, 3));
    assert_eq!(ticked_count(&kinds), 5);
}

#[test]
fn the_odb_folder_box_waits_for_odb() {
    let mut app = app_with_dialog(mockup_kinds());
    let _ = send(&mut app, FabOutputsMsg::OdbFolderToggled);
    assert!(!shown(&app).kinds.odb_folder, "greyed while ODB++ is off");
    let _ = send(&mut app, FabOutputsMsg::OutputToggled(FabOutput::Odb));
    let _ = send(&mut app, FabOutputsMsg::OdbFolderToggled);
    assert!(shown(&app).kinds.odb_folder, "live once ODB++ is ticked");
    let _ = send(&mut app, FabOutputsMsg::OutputToggled(FabOutput::Odb));
    assert!(
        shown(&app).kinds.odb_folder,
        "unticking ODB++ keeps the form for the next time"
    );
}

#[test]
fn a_finished_run_is_timed_in_update_and_an_edit_clears_it() {
    let mut app = app_with_dialog(mockup_kinds());
    assert!(!shown(&app).has_run());
    if let Some(state) = app.ui_state.fab_outputs.as_mut() {
        state.writing = Some(7);
    }
    let _ = send(
        &mut app,
        FabOutputsMsg::Written {
            generation: 7,
            result: Ok(written(&[])),
        },
    );
    let state = shown(&app);
    assert!(state.has_run() && state.writing.is_none());
    assert!(state.generated_at.is_some(), "the time the outcome arrived");
    let _ = send(&mut app, FabOutputsMsg::NameEdited("other".into()));
    let state = shown(&app);
    assert!(!state.has_run(), "an edit makes the outcome stale");
    assert!(state.generated_at.is_none());
}

#[test]
fn an_answer_for_an_older_run_leaves_the_dialog_alone() {
    let mut app = app_with_dialog(mockup_kinds());
    if let Some(state) = app.ui_state.fab_outputs.as_mut() {
        state.writing = Some(8);
    }
    let _ = send(
        &mut app,
        FabOutputsMsg::Written {
            generation: 7,
            result: Ok(written(&[])),
        },
    );
    assert!(!shown(&app).has_run());
    assert!(shown(&app).generated_at.is_none());
}

#[test]
fn open_folder_needs_a_written_set() {
    let mut app = app_with_dialog(mockup_kinds());
    let task = send(&mut app, FabOutputsMsg::FolderOpenRequested);
    assert_eq!(task.units(), 0, "nothing written, nothing to open");
    if let Some(state) = app.ui_state.fab_outputs.as_mut() {
        state.outcome = Some(Ok(written(&[])));
    }
    let task = send(&mut app, FabOutputsMsg::FolderOpenRequested);
    assert_eq!(task.units(), 1, "the launch runs off the update thread");
    assert!(app.ui_state.fab_outputs.is_some(), "the dialog stays open");
}

#[test]
fn a_failed_folder_launch_keeps_the_dialog_open() {
    let mut app = app_with_dialog(mockup_kinds());
    let task = send(
        &mut app,
        FabOutputsMsg::FolderOpenFailed {
            folder: PathBuf::from("fab"),
            why: "no file manager".into(),
        },
    );
    assert_eq!(task.units(), 0);
    assert!(app.ui_state.fab_outputs.is_some());
}

#[test]
fn stale_files_are_the_last_note_and_an_error() {
    let i18n = signex_i18n::I18n::en_us();
    let mut set = written(&["dcdc-old.drl"]);
    set.notes.push(FabNote::PartsWithoutDesignator { count: 2 });
    set.board_notes.push(BoardNote::HolesLeftOut(1));
    let notes = outcome_notes(&set, &i18n);
    let severities: Vec<NoteSeverity> = notes.iter().map(|(s, _)| *s).collect();
    assert_eq!(
        severities,
        [
            NoteSeverity::Warning,
            NoteSeverity::Warning,
            NoteSeverity::Warning,
            NoteSeverity::Error
        ]
    );
    assert!(notes[3].1.to_string().contains("dcdc-old.drl"));
    let clean = written(&[]);
    assert!(
        outcome_notes(&clean, &i18n)
            .iter()
            .all(|(s, _)| *s == NoteSeverity::Warning)
    );
}
