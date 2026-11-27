//! Step 2's drawn preview (#3171): a new import previews into a staged
//! library and an update still dry-runs; the staged library's parts and
//! the selected part's drawing are read; Back, Cancel, Close, a vanished
//! window and quitting remove its folder; Import into a folder that is no
//! longer free keeps it; the step builds in both graphite themes.

use std::path::{Path, PathBuf};

use signex_i18n::I18n;
use signex_types::theme::{ThemeId, theme_tokens};

use super::parts::{PartRead, PartsState, PreviewTab};
use super::tests::{A, converter, d, dialog, found, outcome};
use super::{ImportDialogContext, ImportDialogMsg, ImportDialogState, PreviewState, Step, view};
use crate::app::Signex;
use crate::library_import::output::claim_output;
use crate::library_import::staging::{TargetBefore, staging_dir_for};
use crate::library_import::{LibraryRunRequest, ModelsMode, RunMode, StagedLibrary};
use crate::task_driver::TaskDriver;

/// `rel` under the platform's root, so a path is absolute on every system.
fn abs(rel: &str) -> PathBuf {
    #[cfg(windows)]
    let root = PathBuf::from(r"C:\");
    #[cfg(not(windows))]
    let root = PathBuf::from("/");
    root.join(rel)
}

fn fixture_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures/library-format/library")
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir(&dest).unwrap();
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

fn request(output: PathBuf) -> LibraryRunRequest {
    LibraryRunRequest {
        inputs: vec![abs("in/Conformance.lib1")],
        output,
        name: Some("Conformance".into()),
        db: None,
        search_paths: Vec::new(),
        models: ModelsMode::Include,
        mode: RunMode::Stage,
    }
}

/// The conformance library staged beside `<root>/libs/Conformance`, as a
/// Stage run leaves it.
fn staged_fixture(root: &Path) -> StagedLibrary {
    let target = root.join("libs").join("Conformance");
    let staging = staging_dir_for(&target).unwrap();
    let claimed = claim_output(&request(staging.clone())).unwrap();
    copy_dir(&fixture_library(), &staging);
    StagedLibrary {
        claimed,
        target: target.clone(),
        target_before: TargetBefore::Absent,
        request: request(target),
        run: outcome(),
        library_file: staging.join("Conformance.snxlib"),
    }
}

/// The app with the dialog on step 2 holding `staged`.
fn previewing(staged: StagedLibrary) -> Signex {
    let (mut app, _boot) = Signex::new();
    let location = staged.target.display().to_string();
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step: Step::Preview,
        location,
        preview: Some(PreviewState::Ready(Box::new(outcome()))),
        staged: Some(Box::new(staged)),
        ..ImportDialogState::new()
    }));
    app
}

fn settle(app: &mut Signex, task: iced::Task<crate::app::Message>) {
    TaskDriver::new().settle(app, task).unwrap();
}

/// Next on a new import stages the preview beside the folder (no
/// `--dry-run`); Next on an accepted update still dry-runs into a temp
/// folder.
#[test]
fn a_new_import_stages_its_preview_and_an_update_still_dry_runs() {
    let ready = |accepted: Option<bool>| {
        let mut state = found(vec![converter(&abs("bin/a").display().to_string(), A)]);
        state.apply_sources(vec![abs("in/P.lib1")], Some(&abs("libs")));
        state.update_offer = accepted.map(|accepted| super::source::UpdateOffer {
            local_commits: 0,
            accepted,
        });
        state.pending_run = Some(RunMode::Preview);
        state.checking_target = true;
        state
    };
    let (mut app, _boot) = Signex::new();
    let state = ready(None);
    let location = state.location_path();
    assert_eq!(location, abs("libs").join("P"));
    app.library.import_dialog = Some(Box::new(state));
    drop(app.update(d(ImportDialogMsg::TargetChecked {
        location: location.clone(),
        check: super::TargetCheck::Free,
    })));
    let run = app.library.import.running.as_ref().expect("started");
    assert_eq!((run.mode, &run.output), (RunMode::Stage, &location));
    let state = dialog(&app);
    assert_eq!(state.awaiting, Some(RunMode::Stage));
    assert_eq!(state.step, Step::Preview);
    assert_eq!(state.preview_dir, None, "no temp folder");
    let args = state
        .request(RunMode::Stage, location.clone())
        .unwrap()
        .args();
    assert!(!args.iter().any(|a| a == "--dry-run"), "{args:?}");

    let (mut app, _boot) = Signex::new();
    app.library.import_dialog = Some(Box::new(ready(Some(true))));
    drop(app.update(d(ImportDialogMsg::TargetChecked {
        location: location.clone(),
        check: super::TargetCheck::Imported { local_commits: 0 },
    })));
    let run = app.library.import.running.as_ref().expect("started");
    assert_eq!(
        run.mode,
        RunMode::Preview,
        "an update's preview is a dry run"
    );
    assert!(run.output.starts_with(std::env::temp_dir()));
    assert_eq!(dialog(&app).preview_dir.as_ref(), Some(&run.output));
    assert!(dialog(&app).staged.is_none());
    let args = dialog(&app)
        .request(RunMode::Preview, run.output.clone())
        .unwrap()
        .args();
    assert!(args.iter().any(|a| a == "--dry-run"), "{args:?}");
}

#[test]
fn the_drawn_preview_lists_the_parts_and_draws_the_selected_one() {
    let dir = tempfile::tempdir().unwrap();
    let staged = staged_fixture(dir.path());
    let staging = staged.dir().to_path_buf();
    let mut app = previewing(staged);
    let task = app.read_preview_parts();
    assert_eq!(dialog(&app).parts, Some(PartsState::Loading));
    settle(&mut app, task);
    let state = dialog(&app);
    let parts = state.preview_parts();
    let numbers: Vec<&str> = parts.iter().map(|p| p.part_number.as_str()).collect();
    assert_eq!(
        numbers,
        ["R-10K-0603", "R-1K-0805", "U-NAND-DUAL"],
        "{state:?}"
    );
    let r10k = &parts[0];
    assert_eq!(r10k.symbol.as_deref(), Some("Resistor"));
    assert_eq!(r10k.footprint.as_deref(), Some("R_0603"));
    assert_eq!(r10k.alternates, ["R_0805"]);
    assert_eq!(
        (r10k.manufacturer.as_str(), r10k.mpn.as_str()),
        ("Example Corp", "EX-R0603-10K")
    );
    assert_eq!(r10k.suppliers.len(), 1);
    assert_eq!(r10k.suppliers[0].distributor, "ExampleDistributor");
    assert_eq!(parts[2].footprint, None, "symbol only");
    assert_eq!(state.alternate_columns(), 1);

    let task = app.update(d(ImportDialogMsg::PartSelected(0)));
    assert!(matches!(
        dialog(&app).part_view.as_ref().map(|v| &v.read),
        Some(PartRead::Loading)
    ));
    settle(&mut app, task);
    let details = {
        let (view, part) = dialog(&app).selected_part().expect("selected");
        assert_eq!(part.part_number, "R-10K-0603");
        let PartRead::Ready(drawing) = &view.read else {
            panic!("{:?}", view.read);
        };
        drawing.0.clone()
    };
    assert!(details.symbol.is_some(), "{:?}", details.notes);
    assert_eq!(
        details.footprint.as_ref().map(|f| f.name.as_str()),
        Some("R_0603")
    );
    assert_eq!(details.manufacturer, "Example Corp");
    assert!(details.parameters.iter().any(|(k, _)| k == "resistance"));
    let alternate = details.alternate_footprints[0].reference;
    drop(app.update(d(ImportDialogMsg::PartFootprintPicked(alternate))));
    let (view, _) = dialog(&app).selected_part().unwrap();
    assert_eq!(view.footprint, Some(alternate));
    assert_eq!(
        details
            .footprint_preview(view.footprint.as_ref())
            .map(|f| f.name.as_str()),
        Some("R_0805")
    );

    // A read for a preview the dialog no longer holds is dropped.
    drop(app.update(d(ImportDialogMsg::PartsRead {
        dir: dir.path().join("elsewhere"),
        parts: Ok(Vec::new()),
    })));
    assert_eq!(dialog(&app).preview_parts().len(), 3);
    // The filter narrows the table and hides a selection it leaves out.
    drop(app.update(d(ImportDialogMsg::PartFilterEdited("nand".into()))));
    assert_eq!(dialog(&app).visible_parts().len(), 1);
    assert!(dialog(&app).selected_part().is_none());
    assert!(staging.join("Conformance.snxlib").exists(), "reads only");
}

/// Back, Cancel, Close and a vanished window each remove the staged
/// preview and the parent folder it made; so does quitting.
#[test]
fn leaving_the_preview_removes_its_folder() {
    type Leave = fn(&mut Signex) -> iced::Task<crate::app::Message>;
    let leaves: [(&str, Leave); 5] = [
        ("back", |app| app.update(d(ImportDialogMsg::BackPressed))),
        ("cancel", |app| {
            app.update(d(ImportDialogMsg::CancelPressed))
        }),
        ("close", |app| app.update(d(ImportDialogMsg::Closed))),
        ("vanished", |app| app.import_dialog_window_vanished()),
        ("quit", |app| {
            app.discard_import_preview_then(iced::Task::none())
        }),
    ];
    for (how, leave) in leaves {
        let dir = tempfile::tempdir().unwrap();
        let staged = staged_fixture(dir.path());
        let staging = staged.dir().to_path_buf();
        assert!(staging.join("Conformance.snxlib").exists());
        let mut app = previewing(staged);
        let task = leave(&mut app);
        settle(&mut app, task);
        assert!(!staging.exists(), "{how}: the preview is removed");
        assert!(
            !dir.path().join("libs").exists(),
            "{how}: and the parent it made"
        );
        if let Some(state) = app.library.import_dialog.as_deref() {
            assert!(state.staged.is_none(), "{how}");
        }
        if how == "back" {
            assert_eq!(dialog(&app).step, Step::Source);
        }
    }
}

/// Import into a folder that is no longer free keeps the staged preview
/// on step 2, with why; nothing is removed or moved.
#[test]
fn import_into_a_folder_no_longer_free_keeps_the_preview() {
    let dir = tempfile::tempdir().unwrap();
    let staged = staged_fixture(dir.path());
    let staging = staged.dir().to_path_buf();
    let target = staged.target.clone();
    let mut app = previewing(staged);
    for check in [
        super::TargetCheck::Refused(signex_i18n::Translatable::new(
            signex_i18n::ids::LIBRARY_IMPORT_DIALOG_TARGET_NOT_EMPTY,
        )),
        super::TargetCheck::Imported { local_commits: 0 },
    ] {
        if let Some(d) = app.library.import_dialog.as_deref_mut() {
            d.pending_run = Some(RunMode::Import);
            d.checking_target = true;
            d.move_problem = None;
        }
        let task = app.update(d(ImportDialogMsg::TargetChecked {
            location: target.clone(),
            check,
        }));
        settle(&mut app, task);
        let state = dialog(&app);
        assert_eq!(state.step, Step::Preview);
        assert!(state.move_problem.is_some());
        assert!(state.staged.is_some());
        assert!(app.library.import.running.is_none(), "nothing ran");
        assert!(staging.join("Conformance.snxlib").exists());
    }
}

#[test]
fn the_drawn_preview_builds_in_both_graphite_themes() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = previewing(staged_fixture(dir.path()));
    let task = app.read_preview_parts();
    settle(&mut app, task);
    let task = app.update(d(ImportDialogMsg::PartSelected(0)));
    settle(&mut app, task);
    let drawn = dialog(&app).clone();
    assert!(matches!(
        drawn.part_view.as_ref().map(|v| &v.read),
        Some(PartRead::Ready(_))
    ));
    let mut states = vec![drawn.clone()];
    states.push(ImportDialogState {
        preview_tab: PreviewTab::Report,
        selected_item: Some(0),
        ..drawn.clone()
    });
    states.push(ImportDialogState {
        parts: Some(PartsState::Loading),
        part_view: None,
        move_problem: Some(
            crate::library_import::ImportError::TargetChanged {
                path: abs("libs/Conformance"),
            }
            .to_translatable(),
        ),
        ..drawn.clone()
    });
    states.push(ImportDialogState {
        parts: Some(PartsState::Failed("unreadable".into())),
        ..drawn.clone()
    });
    states.push(ImportDialogState {
        preview: Some(PreviewState::Running),
        awaiting: Some(RunMode::Stage),
        ..drawn.clone()
    });
    states.push(ImportDialogState {
        step: Step::Import,
        preview: None,
        ..drawn
    });
    let i18n = I18n::en_us();
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        for state in &states {
            for windowed in [false, true] {
                let ctx = ImportDialogContext {
                    running: None,
                    project_open: true,
                    library_open: false,
                    look: crate::library::browser::PreviewLook::for_tests(),
                };
                drop(view(state, ctx, &tokens, &i18n, windowed));
            }
        }
    }
}
