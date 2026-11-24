//! The Gerber Viewer's view changes (layer looks, order, selection, the
//! toolbar toggles, the camera) and the window file drops. None reads or
//! writes a file; a drop opens through `open_gerber_paths`.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_gerber::{FileKind, sniff_kind};

use super::GerberViewerMsg;
use super::read::is_zip;
use super::state::OpenPackage;
use crate::app::{Message, Signex};

/// Whether a dropped path is one the viewer reads when its tab is not in
/// view: a Gerber, drill or job file or a zip package, by its name. A
/// folder or any other file opens only on the tab itself.
pub(crate) fn is_fabrication_name(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    is_zip(path) || sniff_kind(&name, b"") != FileKind::NotRead
}

impl Signex {
    /// A file dragged over or dropped on a window. Drops are gathered until
    /// every hovered file has landed, then open as one package.
    pub(super) fn gerber_drop_message(&mut self, msg: GerberViewerMsg) -> Task<Message> {
        let viewer = &mut self.document_state.gerber_viewer;
        let dropped = match msg {
            GerberViewerMsg::FileHovered(path) => {
                viewer.hovered.push(path);
                None
            }
            GerberViewerMsg::FilesHoverLeft => {
                viewer.hovered.clear();
                None
            }
            GerberViewerMsg::FileDropped(path) if viewer.hovered.is_empty() => {
                // No hover came first (some platforms send none): the
                // drops of one gesture arrive together, so settle after.
                let first = viewer.dropped.is_empty();
                viewer.dropped.push(path);
                return if first {
                    Task::done(Message::from(GerberViewerMsg::DropSettled))
                } else {
                    Task::none()
                };
            }
            GerberViewerMsg::FileDropped(path) => viewer.note_drop(path),
            GerberViewerMsg::DropSettled => Some(viewer.take_drop()),
            _ => None,
        };
        match dropped {
            Some(paths) => self.open_dropped_paths(paths),
            None => Task::none(),
        }
    }

    /// Open a drop: everything on the Gerber Viewer tab, elsewhere only
    /// when a file names fabrication data.
    fn open_dropped_paths(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let on_tab = self.gerber_viewer_tab_in_window(true);
        if paths.is_empty() || !(on_tab || paths.iter().any(|p| is_fabrication_name(p))) {
            return Task::none();
        }
        self.open_gerber_paths(paths)
    }

    /// A change to how the package is shown.
    pub(super) fn gerber_view_message(&mut self, msg: GerberViewerMsg) {
        let viewer = &mut self.document_state.gerber_viewer;
        if let GerberViewerMsg::UnitPicked(unit) = msg {
            viewer.unit = unit;
            return;
        }
        let Some(open) = viewer.open.as_mut() else {
            return;
        };
        match msg {
            GerberViewerMsg::LayerVisibilityToggled(layer) => open.toggle_visible(layer),
            GerberViewerMsg::LayerPicked(layer) => {
                open.current = Some(layer);
                open.selected = None;
            }
            GerberViewerMsg::SwatchPressed(layer) => {
                open.colour_open = (open.colour_open != Some(layer)).then_some(layer);
            }
            GerberViewerMsg::ColourPicked { layer, colour } => open.set_colour(layer, colour),
            other => grip_or_board(open, other),
        }
    }
}

/// The grip drag, the toolbar toggles and the board view.
fn grip_or_board(open: &mut OpenPackage, msg: GerberViewerMsg) {
    match msg {
        GerberViewerMsg::GripPressed(from) => {
            open.grip = Some(super::state::GripDrag { from, over: from });
        }
        GerberViewerMsg::RowEntered(over) => {
            if let Some(grip) = open.grip.as_mut() {
                grip.over = over;
            }
        }
        GerberViewerMsg::GripReleased => {
            if let Some(grip) = open.grip.take() {
                open.move_layer(grip.from, grip.over);
            }
        }
        GerberViewerMsg::FlipToggled => open.flipped = !open.flipped,
        GerberViewerMsg::DrillsToggled => open.drills_on_top = !open.drills_on_top,
        GerberViewerMsg::NetHighlightToggled => {
            open.net = match open.net {
                Some(_) => None,
                None => open.selected_net().map(str::to_string),
            };
        }
        GerberViewerMsg::ViewChanged(camera) => open.camera = camera,
        GerberViewerMsg::BoardClicked { at, tolerance } => {
            open.selected = open.object_at(at, tolerance);
            open.colour_open = None;
        }
        _ => {}
    }
}
