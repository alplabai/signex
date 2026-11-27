//! The Gerber Viewer's updates. Opening reads on a worker and lands as
//! `PackageRead`; the view's own changes are in `view_update.rs`. The
//! router lists every message, so a new one does not compile unrouted.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_i18n::{Translatable, ids};

use super::read::{PackageRead, ReadFailure, read_paths};
use super::recent::package_exists;
use super::state::OpenPackage;
use super::{GerberViewerMsg, OpenKind};
use crate::app::agent::WaitingCommand;
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_gerber_viewer_message(&mut self, msg: GerberViewerMsg) -> Task<Message> {
        use GerberViewerMsg as M;
        match msg {
            M::ViewerRequested => self.show_gerber_viewer(),
            M::OpenMenuToggled => self.toggle_gerber_open_menu(),
            M::OpenPressed(kind) => self.pick_gerber_paths(kind),
            M::PathsPicked(Some(paths)) | M::PathsOpened(paths) => self.open_gerber_paths(paths),
            M::PathsPicked(None) => Task::none(),
            M::RecentPressed(index) => self.open_recent_gerber_package(index),
            M::RecentChecked { found, now } => self.gerber_recent_checked(found, now),
            M::RecentCheckFailed(why) => self.gerber_recent_check_failed(&why),
            M::PackageRead {
                job,
                paths,
                result,
                waiting,
            } => self.gerber_package_read(job, paths, result, waiting),
            M::FileHovered(path) => self.gerber_file_hovered(path),
            M::FilesHoverLeft => self.gerber_hover_left(),
            M::FileDropped { window, path } => self.gerber_file_dropped(window, path),
            M::DropSettled { window } => self.gerber_drop_settled(window),
            M::UnitPicked(unit) => self.gerber_unit_picked(unit),
            M::EscapePressed => self.gerber_escape_pressed(),
            M::SelectionCleared => self.on_gerber_package(|o| o.select(None)),
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
            M::RunBannerClosed => self.close_gerber_run_banner(),
            M::BackToRunPressed => self.back_to_gerber_run(),
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
        self.open_gerber_paths_for(paths, None)
    }

    /// [`Self::open_gerber_paths`], answering the `open_gerber_viewer` call
    /// `waiting` once the read lands (#2896).
    pub(super) fn open_gerber_paths_for(
        &mut self,
        paths: Vec<PathBuf>,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        if paths.is_empty() {
            return Task::none();
        }
        self.open_gerber_viewer_tab();
        let viewer = &mut self.document_state.gerber_viewer;
        viewer.open_menu = false;
        // Files opened any other way are not a run's (mockup row 193b).
        viewer.from_run = None;
        viewer.run_missing = false;
        let job = viewer.start_read();
        let reading = paths.clone();
        let read = Task::perform(
            crate::app::path_op_jobs::run_blocking(move || read_paths(reading)),
            move |result| {
                Message::from(GerberViewerMsg::PackageRead {
                    job,
                    paths,
                    result: result
                        .map_err(ReadFailure::from)
                        .and_then(|read| read)
                        .map(Arc::new),
                    waiting,
                })
            },
        );
        Task::batch([read, self.check_recent_gerber_packages()])
    }

    /// `open_gerber_viewer` without a path: the tab opens, or is focused,
    /// and the recent packages its empty state lists are checked.
    pub(super) fn show_gerber_viewer(&mut self) -> Task<Message> {
        self.open_gerber_viewer_tab();
        self.check_recent_gerber_packages()
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
    /// in Messages. A read a later open superseded is dropped. The
    /// `open_gerber_viewer` call `waiting` on it is answered either way.
    fn gerber_package_read(
        &mut self,
        job: u64,
        paths: Vec<PathBuf>,
        result: Result<Arc<PackageRead>, ReadFailure>,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        let viewer = &mut self.document_state.gerber_viewer;
        if viewer.reading != Some(job) {
            let reason = Translatable::new(ids::GERBER_VIEWER_READ_SUPERSEDED)
                .arg("file", package_name(&paths));
            return self.refuse_gerber_command(waiting, &paths, reason);
        }
        viewer.reading = None;
        let read = match result {
            Ok(read) => Arc::unwrap_or_clone(read),
            Err(why) => {
                // A run's files moved since: the banner says where they were.
                viewer.run_missing = viewer.from_run.is_some();
                let reason = read_failure_reason(&why, &package_name(&paths));
                let text = reason.render(&self.ui_state.i18n);
                log::error!(target: crate::app::gerber_read::TARGET, "{text}");
                let check = self.check_recent_gerber_packages();
                let refused = self.refuse_gerber_command(waiting, &paths, reason);
                return Task::batch([check, refused]);
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
        self.answer_gerber_command(waiting, &paths)
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

    /// The recent-package check could not run: a Gerber Viewer warning in
    /// Messages, from the catalog. The rows and the clock stay as they were.
    fn gerber_recent_check_failed(&mut self, why: &str) -> Task<Message> {
        let text = Translatable::new(ids::GERBER_VIEWER_RECENT_CHECK_FAILED)
            .arg("why", why)
            .render(&self.ui_state.i18n);
        log::warn!(target: crate::app::gerber_read::TARGET, "{text}");
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
            |result| match result {
                Ok((found, now)) => Message::from(GerberViewerMsg::RecentChecked {
                    found,
                    now: Some(now),
                }),
                Err(why) => Message::from(GerberViewerMsg::RecentCheckFailed(why)),
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

/// Why the package named `package` could not be read at all: its Messages
/// row and the refusal of the `open_gerber_viewer` call that asked for it.
fn read_failure_reason(failure: &ReadFailure, package: &str) -> Translatable {
    match failure {
        ReadFailure::Unreadable(why) => Translatable::new(ids::GERBER_VIEWER_READ_FAILED)
            .arg("file", package)
            .arg("why", why.as_str()),
        ReadFailure::ZipTooLarge { file, limit_mib } => {
            Translatable::new(ids::GERBER_VIEWER_READ_ZIP_TOO_LARGE)
                .arg("file", file.as_str())
                .arg("limit", *limit_mib)
        }
    }
}

/// The name a package is known by in Messages: the file name of its first
/// path, as the tab title shows it (#2612).
pub(super) fn package_name(paths: &[PathBuf]) -> String {
    paths
        .first()
        .map(|path| {
            path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        })
        .unwrap_or_default()
}
