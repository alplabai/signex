//! The Gerber Viewer's updates. Opening reads on a worker and lands as
//! `PackageRead`; the view's own changes are in `view_update.rs`. The
//! router lists every message, so a new one does not compile unrouted.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_i18n::{I18n, ids, t};

use super::read::{PackageRead, ReadFailure, read_paths};
use super::recent::package_exists;
use super::state::OpenPackage;
use super::{GerberViewerMsg, OpenKind};
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_gerber_viewer_message(&mut self, msg: GerberViewerMsg) -> Task<Message> {
        use GerberViewerMsg as M;
        match msg {
            M::OpenMenuToggled => self.toggle_gerber_open_menu(),
            M::OpenPressed(kind) => self.pick_gerber_paths(kind),
            M::PathsPicked(Some(paths)) | M::PathsOpened(paths) => self.open_gerber_paths(paths),
            M::PathsPicked(None) => Task::none(),
            M::RecentPressed(index) => self.open_recent_gerber_package(index),
            M::RecentChecked { found, now } => self.gerber_recent_checked(found, now),
            M::PackageRead { job, result } => self.gerber_package_read(job, result),
            M::FileHovered(path) => self.gerber_file_hovered(path),
            M::FilesHoverLeft => self.gerber_hover_left(),
            M::FileDropped { window, path } => self.gerber_file_dropped(window, path),
            M::DropSettled { window } => self.gerber_drop_settled(window),
            M::UnitPicked(unit) => self.gerber_unit_picked(unit),
            M::EscapePressed => self.gerber_escape_pressed(),
            M::LayerVisibilityToggled(layer) => self.on_gerber_package(|o| o.toggle_visible(layer)),
            M::LayerPicked(layer) => self.on_gerber_package(|o| o.pick_layer(layer)),
            M::SwatchPressed(layer) => self.on_gerber_package(|o| o.toggle_swatch(layer)),
            M::ColourPicked { layer, colour } => {
                self.on_gerber_package(|o| o.set_colour(layer, colour))
            }
            M::GripPressed(from) => self.on_gerber_package(|o| o.press_grip(from)),
            M::RowEntered(over) => self.while_gerber_grip(|o| o.enter_row(over)),
            M::GripReleased => self.while_gerber_grip(OpenPackage::release_grip),
            M::FlipToggled => self.on_gerber_package(|o| o.flipped = !o.flipped),
            M::DrillsToggled => self.on_gerber_package(|o| o.drills_on_top = !o.drills_on_top),
            M::NetHighlightToggled => self.on_gerber_package(OpenPackage::toggle_net_highlight),
            M::ViewChanged(camera) => self.on_gerber_package(|o| o.camera = camera),
            M::BoardClicked { at, tolerance } => {
                self.on_gerber_package(|o| o.click_at(at, tolerance))
            }
        }
    }

    /// The Open menu opens or closes; opening it checks the recent
    /// packages it lists.
    fn toggle_gerber_open_menu(&mut self) -> Task<Message> {
        let viewer = &mut self.document_state.gerber_viewer;
        viewer.open_menu = !viewer.open_menu;
        if viewer.open_menu {
            self.check_recent_gerber_packages()
        } else {
            Task::none()
        }
    }

    /// Open `paths` as one package on the Gerber Viewer tab, reading them
    /// on a worker. The tab opens first, so the read shows on it; the
    /// recent packages its empty state lists are checked alongside.
    pub(crate) fn open_gerber_paths(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        if paths.is_empty() {
            return Task::none();
        }
        self.open_gerber_viewer_tab();
        let viewer = &mut self.document_state.gerber_viewer;
        viewer.open_menu = false;
        let job = viewer.start_read();
        let read = Task::perform(
            crate::app::path_op_jobs::run_blocking(move || read_paths(paths)),
            move |result| {
                Message::from(GerberViewerMsg::PackageRead {
                    job,
                    result: result
                        .map_err(ReadFailure::from)
                        .and_then(|read| read)
                        .map(Arc::new),
                })
            },
        );
        Task::batch([read, self.check_recent_gerber_packages()])
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
        result: Result<Arc<PackageRead>, ReadFailure>,
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

    fn gerber_recent_checked(
        &mut self,
        found: Vec<(Vec<PathBuf>, bool)>,
        now: Option<i64>,
    ) -> Task<Message> {
        let viewer = &mut self.document_state.gerber_viewer;
        for (paths, exists) in found {
            viewer.recent.set_missing(&paths, !exists);
        }
        if now.is_some() {
            viewer.now = now;
        }
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
                (recent_presence(listed), chrono::Utc::now().timestamp())
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

    fn gerber_unit_picked(&mut self, unit: super::LengthUnit) -> Task<Message> {
        self.document_state.gerber_viewer.unit = unit;
        Task::none()
    }

    fn gerber_escape_pressed(&mut self) -> Task<Message> {
        self.document_state.gerber_viewer.escape();
        Task::none()
    }

    /// A change to how the open package is shown; with nothing open, none.
    fn on_gerber_package(&mut self, change: impl FnOnce(&mut OpenPackage)) -> Task<Message> {
        if let Some(open) = self.document_state.gerber_viewer.open.as_mut() {
            change(open);
            open.revision += 1;
        }
        Task::none()
    }

    /// A pointer move or release that matters only while a row is held:
    /// without one it changes nothing, so the board keeps its picture.
    fn while_gerber_grip(&mut self, change: impl FnOnce(&mut OpenPackage)) -> Task<Message> {
        let viewer = &self.document_state.gerber_viewer;
        if viewer.open.as_ref().is_some_and(|open| open.grip.is_some()) {
            self.on_gerber_package(change)
        } else {
            Task::none()
        }
    }
}

/// Whether each listed package is still there. Blocking: run it in a task.
pub(super) fn recent_presence(listed: Vec<Vec<PathBuf>>) -> Vec<(Vec<PathBuf>, bool)> {
    listed
        .into_iter()
        .map(|paths| {
            let exists = package_exists(&paths);
            (paths, exists)
        })
        .collect()
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
fn log_read_failure(i18n: &I18n, failure: &ReadFailure) {
    let text = match failure {
        ReadFailure::Unreadable(why) => {
            t!(i18n, ids::GERBER_VIEWER_READ_FAILED, "why" => why.as_str())
        }
        ReadFailure::ZipTooLarge { file, limit_mib } => t!(
            i18n,
            ids::GERBER_VIEWER_READ_ZIP_TOO_LARGE,
            "file" => file.as_str(),
            "limit" => *limit_mib
        ),
    };
    log::error!(target: crate::app::gerber_read::TARGET, "{}", text.into_string());
}
