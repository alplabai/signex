//! Mounting and re-importing (#3123): the update offered for a folder that
//! holds an imported library, the run it becomes once taken, what a failed
//! update says about its folder, and where the result card says the
//! library went.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signex_i18n::I18n;

use super::result::{Mounted, ResultTab};
use super::source::{Blocker, UpdateOffer};
use super::tests::{A, converter, d, dialog, found, import, outcome, picked};
use super::{FolderAfter, ImportDialogMsg, ImportDialogState, ImportEnd, Step, TargetCheck};
use crate::app::Signex;
use crate::library::messages::LibraryImportMsg;
use crate::library_import::run_state::{ImportFinished, ImportRun, MountTarget};
use crate::library_import::{ImportError, ImportJobOutcome, RunFailure, RunMode};

const OUT: &str = "/out/Passives";

/// The app with step 1 filled in for `OUT`, waiting for the folder check
/// behind `mode`'s button.
fn checking(mode: RunMode) -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut state = found(vec![converter("/bin/a", A)]);
    picked(&mut state, &["/in/Passives.lib1"]);
    state.set_location(OUT.into());
    state.checking_target = true;
    state.pending_run = Some(mode);
    if mode != RunMode::Preview {
        state.step = Step::Preview;
    }
    app.library.import_dialog = Some(Box::new(state));
    app
}

fn checked(check: TargetCheck) -> crate::app::Message {
    d(ImportDialogMsg::TargetChecked {
        location: OUT.into(),
        check,
    })
}

#[test]
fn an_imported_library_is_offered_as_an_update_and_runs_none_until_taken() {
    let mut app = checking(RunMode::Preview);
    drop(app.update(checked(TargetCheck::Imported { local_commits: 2 })));
    let state = dialog(&app);
    assert_eq!(state.step, Step::Source);
    assert_eq!(
        state.update_offer,
        Some(UpdateOffer {
            local_commits: 2,
            accepted: false
        })
    );
    assert_eq!(state.blocker(), Some(Blocker::UpdateOffered));
    assert!(!state.is_update());
    assert!(app.library.import.running.is_none(), "nothing runs");
    // Next is off while the offer waits.
    drop(app.update(d(ImportDialogMsg::NextPressed)));
    assert!(!dialog(&app).checking_target);
    // Taken: the folder is checked again for the preview.
    drop(app.update(d(ImportDialogMsg::UpdateAccepted)));
    let state = dialog(&app);
    assert!(state.is_update());
    assert!(state.checking_target);
    assert_eq!(state.pending_run, Some(RunMode::Preview));
    drop(app.update(checked(TargetCheck::Imported { local_commits: 2 })));
    let run = app.library.import.running.as_ref().expect("the preview runs");
    assert_eq!(run.mode, RunMode::Preview);
    assert_eq!(dialog(&app).step, Step::Preview);
}

#[test]
fn a_taken_offer_makes_the_import_an_update_of_the_folder() {
    let mut app = checking(RunMode::Import);
    app.library.import_dialog.as_deref_mut().unwrap().update_offer = Some(UpdateOffer {
        local_commits: 0,
        accepted: true,
    });
    drop(app.update(checked(TargetCheck::Imported { local_commits: 0 })));
    let run = app.library.import.running.as_ref().expect("started");
    assert_eq!(run.mode, RunMode::Update);
    assert_eq!(run.output, PathBuf::from(OUT));
    assert_eq!(dialog(&app).step, Step::Import);
}

#[test]
fn a_folder_that_became_free_is_a_new_import_and_an_edit_drops_the_offer() {
    let mut app = checking(RunMode::Import);
    app.library.import_dialog.as_deref_mut().unwrap().update_offer = Some(UpdateOffer {
        local_commits: 0,
        accepted: true,
    });
    drop(app.update(checked(TargetCheck::Free)));
    let run = app.library.import.running.as_ref().expect("started");
    assert_eq!(run.mode, RunMode::Import);
    assert!(dialog(&app).update_offer.is_none());
    let mut state = found(vec![converter("/bin/a", A)]);
    picked(&mut state, &["/in/Passives.lib1"]);
    for edit in 0..3 {
        state.update_offer = Some(UpdateOffer {
            local_commits: 0,
            accepted: false,
        });
        match edit {
            0 => state.set_location("/elsewhere".into()),
            1 => state.set_name("Other".into(), None),
            _ => picked(&mut state, &["/in/Caps.lib1"]),
        }
        assert!(state.update_offer.is_none(), "edit {edit}");
    }
}

#[test]
fn a_refused_update_folder_goes_back_to_step_one_with_its_reason() {
    let mut app = checking(RunMode::Preview);
    let dirty = ImportError::UncommittedChanges {
        paths: vec!["symbols/R.snxsym".into()],
    };
    drop(app.update(checked(TargetCheck::Refused(dirty.to_translatable()))));
    let state = dialog(&app);
    assert_eq!(state.step, Step::Source);
    assert_eq!(state.blocker(), Some(Blocker::TargetRefused));
    assert_eq!(
        state.target_problem.as_ref().and_then(|t| t.id()),
        Some(signex_i18n::ids::LIBRARY_IMPORT_UNCOMMITTED)
    );
    assert!(state.update_offer.is_none());
}

/// The app with the dialog on step 3 of an update of `OUT`.
fn updating() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.library.import.running = Some(ImportRun {
        output: OUT.into(),
        mode: RunMode::Update,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(false)),
        group: Default::default(),
        progress: None,
    });
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step: Step::Import,
        awaiting: Some(RunMode::Update),
        location: OUT.into(),
        update_offer: Some(UpdateOffer {
            local_commits: 1,
            accepted: true,
        }),
        ..ImportDialogState::new()
    }));
    app
}

fn failed(restored: Result<(), String>) -> crate::app::Message {
    let mut failure = RunFailure::new(ImportError::ConverterFailed {
        message: "a row you added has the part number R-10K".into(),
    });
    failure.restored = Some(restored);
    import(LibraryImportMsg::Finished(Box::new(ImportFinished {
        result: Err(failure),
        target: MountTarget::Global,
    })))
}

#[test]
fn a_failed_update_says_whether_the_library_was_put_back() {
    let i18n = I18n::en_us();
    for (restored, expected) in [
        (Ok(()), FolderAfter::Restored),
        (
            Err("locked".to_string()),
            FolderAfter::NotRestored("locked".into()),
        ),
    ] {
        let mut app = updating();
        drop(app.update(failed(restored)));
        let end = dialog(&app).import_end.clone().expect("ended");
        let ImportEnd::Failed { folder, .. } = &end else {
            panic!("{end:?}");
        };
        assert_eq!(folder, &expected);
        let (_, first, second) = super::view::end_words(&end, OUT.into(), &i18n);
        assert!(first.into_string().contains("R-10K"), "the converter's reason");
        let second = second.into_string();
        match expected {
            FolderAfter::Restored => assert!(second.contains("put back"), "{second}"),
            _ => assert!(second.contains("could not be put back"), "{second}"),
        }
        assert!(app.document_view.error_notice.is_none(), "shown in the dialog");
    }
}

#[test]
fn the_result_says_where_the_library_was_mounted_and_what_it_requires() {
    let (mut app, _boot) = Signex::new();
    app.library.import.running = Some(ImportRun {
        output: OUT.into(),
        mode: RunMode::Import,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(false)),
        group: Default::default(),
        progress: None,
    });
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step: Step::Import,
        awaiting: Some(RunMode::Import),
        location: OUT.into(),
        ..ImportDialogState::new()
    }));
    let required = uuid::Uuid::from_u128(7);
    let done = ImportJobOutcome {
        run: outcome(),
        library_file: Some(PathBuf::from(OUT).join("Passives.snxlib")),
        commit: Some("0123456789abcdef".into()),
        local_commits: 0,
        library_id: Some(uuid::Uuid::from_u128(1)),
        requires: vec![required],
    };
    drop(app.update(import(LibraryImportMsg::Finished(Box::new(
        ImportFinished {
            result: Ok(done),
            target: MountTarget::Global,
        },
    )))));
    let state = dialog(&app);
    assert_eq!(state.result_tab, ResultTab::Result);
    let facts = state.finished().and_then(|r| r.run.as_ref()).unwrap();
    assert_eq!(facts.mounted, Some(Mounted::AllProjects));
    assert_eq!(facts.commit.as_deref(), Some("0123456789abcdef"));
    assert_eq!(facts.required.len(), 1);
    assert_eq!(facts.required[0].id, required);
    assert_eq!(facts.required[0].open_as, None, "no library with it is open");
}
