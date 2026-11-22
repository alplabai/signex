//! The app, the real sheet files and the small drivers the chrome rows share.
//!
//! Handlers that save a preference return that write as a task. Tests that
//! only read the model use `app.update` and drop it, so parallel tests do not
//! race on the one redirected `prefs.json`.

use std::path::{Path, PathBuf};

use signex_app::app::{CaseProbeOutcome, FileMsg, InputMsg, Message, Signex};
use signex_types::format::SnxSchematic;
use signex_types::schematic::SchematicSheet;
use tempfile::TempDir;
use uuid::Uuid;

use crate::fixture::{self, Fixture};
use crate::settle_support::settle;

/// A fresh app with a main window, no tour, no lists and no recovery folder.
pub(super) fn app() -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.window_size = (1400.0, 900.0);
    app.ui_state.recent_projects = Default::default();
    app.ui_state.pinned_projects = Default::default();
    app.document_state.autosave.set_dir(None);
    app
}

pub(super) fn main_window(app: &Signex) -> iced::window::Id {
    app.ui_state.main_window_id.expect("the main window")
}

/// The text of a sheet file on `paper`.
pub(super) fn sheet_text(paper: &str) -> String {
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: paper.into(),
        ..Default::default()
    };
    SnxSchematic::new(sheet)
        .write_string()
        .expect("encode the sheet")
}

/// Open `path` from disk, through File ▸ Open.
pub(super) fn open(app: &mut Signex, path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = app.update(Message::File(FileMsg::DirCaseProbed {
            dir: dir.to_path_buf(),
            answer: CaseProbeOutcome::Decided(false),
        }));
    }
    settle(
        app,
        Message::File(FileMsg::Opened(Some(path.to_path_buf()))),
    );
}

/// An app and a temp folder for real sheet files.
pub(super) struct Desk {
    pub app: Signex,
    pub dir: TempDir,
}

pub(super) fn desk() -> Desk {
    Desk {
        app: app(),
        dir: TempDir::new().expect("tempdir"),
    }
}

impl Desk {
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Write a sheet on `paper` and open it in a tab.
    pub fn sheet(&mut self, name: &str, paper: &str) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, sheet_text(paper)).expect("write the sheet");
        open(&mut self.app, &path);
        path
    }
}

/// An edit through the engine, as a Properties field makes it.
pub(super) fn set_paper(app: &mut Signex, path: &Path, paper: &str) {
    app.document_state
        .engines
        .get_mut(path)
        .expect("the sheet is open")
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: paper.to_string(),
        })
        .expect("the edit applies");
    app.document_state.mark_document_dirty(path);
}

pub(super) fn paper(app: &Signex, path: &Path) -> String {
    app.document_state
        .engines
        .get(path)
        .expect("the sheet is open")
        .document()
        .paper_size
        .clone()
}

pub(super) fn is_dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path)
}

pub(super) fn esc(app: &mut Signex) {
    let window = app.ui_state.main_window_id;
    settle(app, Message::Input(InputMsg::EscapePressed { window }));
}

/// The bundled sample project, copied to a temp folder.
pub(super) struct Sample {
    pub app: Signex,
    pub project: PathBuf,
    _fixture: Fixture,
}

/// The sample project loaded with no document picked.
pub(super) fn sample_project() -> Sample {
    let fixture = fixture::fixture().expect("the sample copies");
    let mut app = app();
    let project = fixture.sample.clone();
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(project.clone()))),
    );
    Sample {
        app,
        project,
        _fixture: fixture,
    }
}

/// The sample project with its root sheet open and in front.
pub(super) fn sample_with_sheet() -> Sample {
    let mut sample = sample_project();
    let sheet = sample.project.with_extension("snxsch");
    settle(&mut sample.app, Message::File(FileMsg::Opened(Some(sheet))));
    sample
}
