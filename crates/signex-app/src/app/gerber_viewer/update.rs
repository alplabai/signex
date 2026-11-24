//! The Gerber Viewer's updates. Opening reads on a worker and lands as
//! `PackageRead`; the view's own changes are in `view_update.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_i18n::{I18n, ids, t};

use super::read::{PackageRead, read_paths};
use super::recent::package_exists;
use super::state::OpenPackage;
use super::{GerberViewerMsg, OpenKind};
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_gerber_viewer_message(&mut self, msg: GerberViewerMsg) -> Task<Message> {
        match msg {
            GerberViewerMsg::OpenMenuToggled => {
                let viewer = &mut self.document_state.gerber_viewer;
                viewer.open_menu = !viewer.open_menu;
                Task::none()
            }
            GerberViewerMsg::OpenPressed(kind) => self.pick_gerber_paths(kind),
            GerberViewerMsg::PathsPicked(Some(paths)) | GerberViewerMsg::PathsOpened(paths) => {
                self.open_gerber_paths(paths)
            }
            GerberViewerMsg::PathsPicked(None) => Task::none(),
            GerberViewerMsg::RecentPressed(index) => self.open_recent_gerber_package(index),
            GerberViewerMsg::RecentChecked { found, now } => {
                let viewer = &mut self.document_state.gerber_viewer;
                for (paths, exists) in found {
                    viewer.recent.set_missing(&paths, !exists);
                }
                if now.is_some() {
                    viewer.now = now;
                }
                Task::none()
            }
            GerberViewerMsg::PackageRead { job, result } => self.gerber_package_read(job, result),
            GerberViewerMsg::FileHovered(_)
            | GerberViewerMsg::FilesHoverLeft
            | GerberViewerMsg::FileDropped(_)
            | GerberViewerMsg::DropSettled => self.gerber_drop_message(msg),
            other => {
                self.gerber_view_message(other);
                Task::none()
            }
        }
    }

    /// Open `paths` as one package on the Gerber Viewer tab, reading them
    /// on a worker. The tab opens first, so the read shows on it.
    pub(crate) fn open_gerber_paths(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        if paths.is_empty() {
            return Task::none();
        }
        self.open_gerber_viewer_tab();
        let viewer = &mut self.document_state.gerber_viewer;
        viewer.open_menu = false;
        let job = viewer.start_read();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || read_paths(paths)),
            move |result| {
                Message::from(GerberViewerMsg::PackageRead {
                    job,
                    result: result.and_then(|read| read).map(Arc::new),
                })
            },
        )
    }

    fn pick_gerber_paths(&mut self, kind: OpenKind) -> Task<Message> {
        self.document_state.gerber_viewer.open_menu = false;
        let i18n = &self.ui_state.i18n;
        let base = self.ui_state.io.picker.dialog();
        Task::perform(
            match kind {
                OpenKind::Folder => {
                    let dialog = base.title_in(i18n, ids::GERBER_VIEWER_OPEN_FOLDER);
                    PickedPaths::Folder(dialog)
                }
                OpenKind::Zip => {
                    let dialog = base.title_in(i18n, ids::GERBER_VIEWER_OPEN_ZIP).filter_in(
                        i18n,
                        ids::GERBER_VIEWER_OPEN_ZIP,
                        &["zip"],
                    );
                    PickedPaths::File(dialog)
                }
                OpenKind::Files => {
                    let dialog = base.title_in(i18n, ids::GERBER_VIEWER_OPEN_FILES);
                    PickedPaths::Files(dialog)
                }
            }
            .pick(),
            |picked| Message::from(GerberViewerMsg::PathsPicked(picked)),
        )
    }

    fn open_recent_gerber_package(&mut self, index: usize) -> Task<Message> {
        let viewer = &self.document_state.gerber_viewer;
        match viewer.recent.entries().get(index) {
            Some(entry) if !entry.missing => self.open_gerber_paths(entry.paths.clone()),
            _ => Task::none(),
        }
    }

    /// A read landed: show it, list it in Recent and report its problems
    /// in Messages. A read a later open superseded is dropped.
    fn gerber_package_read(
        &mut self,
        job: u64,
        result: Result<Arc<PackageRead>, String>,
    ) -> Task<Message> {
        let viewer = &mut self.document_state.gerber_viewer;
        if viewer.reading != Some(job) {
            return Task::none();
        }
        viewer.reading = None;
        let read = match result {
            Ok(read) => Arc::unwrap_or_clone(read),
            Err(why) => {
                log_read_failure(&self.ui_state.i18n, &why);
                return self.check_recent_gerber_packages();
            }
        };
        let package = Arc::new(read.package);
        let now = chrono::Utc::now().timestamp();
        viewer
            .recent
            .record(read.paths.clone(), package.layers.len(), now);
        viewer.now = Some(now);
        crate::app::gerber_read::report_read_problems(&package.problems(), &self.ui_state.i18n);
        viewer.open = Some(OpenPackage::new(package, read.paths));
        self.retitle_gerber_viewer_tab();
        Task::none()
    }

    /// Find out which recent packages are still on disk, off the UI thread.
    pub(crate) fn check_recent_gerber_packages(&self) -> Task<Message> {
        let listed = self.document_state.gerber_viewer.recent.all_paths();
        if listed.is_empty() {
            return Task::none();
        }
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                let found: Vec<(Vec<PathBuf>, bool)> = listed
                    .into_iter()
                    .map(|paths| {
                        let exists = package_exists(&paths);
                        (paths, exists)
                    })
                    .collect();
                (found, chrono::Utc::now().timestamp())
            }),
            |result| {
                let (found, now) = match result {
                    Ok((found, now)) => (found, Some(now)),
                    Err(why) => {
                        crate::diagnostics::log_warning(why);
                        (Vec::new(), None)
                    }
                };
                Message::from(GerberViewerMsg::RecentChecked { found, now })
            },
        )
    }
}

/// The picker an Open action asks, ready to run off the UI thread.
enum PickedPaths {
    Folder(crate::io_ports::FileDialog),
    File(crate::io_ports::FileDialog),
    Files(crate::io_ports::FileDialog),
}

impl PickedPaths {
    async fn pick(self) -> Option<Vec<PathBuf>> {
        match self {
            Self::Folder(dialog) => dialog.pick_folder().await.map(|path| vec![path]),
            Self::File(dialog) => dialog.pick_file().await.map(|path| vec![path]),
            Self::Files(dialog) => dialog.pick_files().await,
        }
    }
}

/// One Messages row for a package that could not be read at all.
fn log_read_failure(i18n: &I18n, why: &str) {
    let text = t!(i18n, ids::GERBER_VIEWER_READ_FAILED, "why" => why);
    log::error!(target: crate::app::gerber_read::TARGET, "{}", text.into_string());
}
