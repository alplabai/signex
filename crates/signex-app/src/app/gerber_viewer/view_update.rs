//! The window file drops. None reads a file here; a drop opens through
//! `open_gerber_paths`. The view's other changes are `OpenPackage` methods
//! the router in `update.rs` calls.

use std::path::PathBuf;

use iced::Task;
use iced::window;

use super::GerberViewerMsg;
use super::read::is_fabrication_name;
use crate::app::{Message, Signex};

impl Signex {
    /// A file is dragged over a window: the drop frame counts it.
    pub(super) fn gerber_file_hovered(&mut self, path: PathBuf) -> Task<Message> {
        self.document_state.gerber_viewer.hovered.push(path);
        Task::none()
    }

    pub(super) fn gerber_hover_left(&mut self) -> Task<Message> {
        self.document_state.gerber_viewer.hovered.clear();
        Task::none()
    }

    /// A file landed on `window`. Drops are gathered until every hovered
    /// file has landed, then open as one package. With no hover first
    /// (some platforms send none) the drops of one gesture arrive
    /// together, so they settle after the batch.
    pub(super) fn gerber_file_dropped(
        &mut self,
        window: window::Id,
        path: PathBuf,
    ) -> Task<Message> {
        let viewer = &mut self.document_state.gerber_viewer;
        if viewer.hovered.is_empty() {
            let first = viewer.dropped.is_empty();
            viewer.dropped.push(path);
            return if first {
                Task::done(Message::from(GerberViewerMsg::DropSettled { window }))
            } else {
                Task::none()
            };
        }
        match viewer.note_drop(path) {
            Some(paths) => self.open_dropped_paths(window, paths),
            None => Task::none(),
        }
    }

    pub(super) fn gerber_drop_settled(&mut self, window: window::Id) -> Task<Message> {
        let paths = self.document_state.gerber_viewer.take_drop();
        self.open_dropped_paths(window, paths)
    }

    /// Open a drop: everything when `window` shows the Gerber Viewer tab,
    /// elsewhere only when a file names fabrication data.
    fn open_dropped_paths(&mut self, window: window::Id, paths: Vec<PathBuf>) -> Task<Message> {
        let on_tab = self
            .document_state
            .tab_shown_in_window(window, &self.ui_state)
            .is_some_and(|tab| tab.kind.is_gerber_viewer());
        if paths.is_empty() || !(on_tab || paths.iter().any(|p| is_fabrication_name(p))) {
            return Task::none();
        }
        self.open_gerber_paths(paths)
    }
}
