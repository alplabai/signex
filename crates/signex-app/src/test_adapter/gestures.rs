//! The adapter's gestures, in paths and mm: the routes that have no command
//! id yet but that the safety net needs (#2326, slice C decisions of
//! 2026-11-18). A tree open and a History restore are the only ways to the
//! read half of the generation check (`finish_claim`); the drag is the
//! event-granularity baseline's input.

use std::path::Path;

use iced::Task;

use super::{DriveError, Pending, TestAdapter};
use crate::app::{Message, UiMsg};
use crate::canvas::{CanvasEvent, MoveConnections};

impl TestAdapter {
    /// Double-click `path` in the project tree of the open project that
    /// holds it. The read runs in the returned handle.
    pub fn open_from_tree(&mut self, path: &Path) -> Pending {
        let projects = &self.app.document_state.projects;
        let found = projects.iter().enumerate().find_map(|(index, project)| {
            let relative = path.strip_prefix(&project.data.dir).ok()?;
            Some((index, relative.to_string_lossy().into_owned()))
        });
        let task = found
            .and_then(|(index, name)| self.app.open_project_tree_document(&[index], name).ok())
            .unwrap_or_else(Task::none);
        self.hold(task)
    }

    /// Restore the document at `path` to the commit `sha` from its History
    /// panel, confirming the second click a document with unsaved edits asks
    /// for. The restore runs in the returned handle.
    pub fn restore(&mut self, path: &Path, sha: &str) -> Pending {
        if !self.app.activate_tab_for_path(path) {
            return self.hold(Task::none());
        }
        let first = self.app.click_restore_on_listed_history(sha);
        let armed = self.app.document_state.history.pending_restore.as_deref() == Some(sha);
        let task = if armed {
            Task::batch([first, self.app.click_restore_on_listed_history(sha)])
        } else {
            first
        };
        self.hold(task)
    }

    /// Drag the item at `from` on the sheet at `path` to `to`, in mm, the
    /// way the canvas reports a press-move-release: a click that selects it,
    /// `steps` pointer moves, and the drop.
    pub fn drag(
        &mut self,
        path: &Path,
        from: (f64, f64),
        to: (f64, f64),
        steps: usize,
    ) -> Result<(), DriveError> {
        self.app.activate_tab_for_path(path);
        self.send(Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: from.0,
            world_y: from.1,
        }))?;
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            let x = from.0 + (to.0 - from.0) * t;
            let y = from.1 + (to.1 - from.1) * t;
            self.drag_step(x, y)?;
        }
        self.send(Message::CanvasEvent(CanvasEvent::MoveSelected {
            dx: to.0 - from.0,
            dy: to.1 - from.1,
            connections: MoveConnections::Preference,
        }))
    }

    /// One pointer move of a drag: what the canvas publishes, then what the
    /// window subscription sends. No layout drag reads the window position,
    /// so it carries the mm value.
    fn drag_step(&mut self, x: f64, y: f64) -> Result<(), DriveError> {
        self.send(Message::CanvasEvent(CanvasEvent::CursorAt {
            x: x as f32,
            y: y as f32,
            zoom_pct: 100.0,
        }))?;
        self.send(Message::Ui(UiMsg::DragMove(x as f32, y as f32)))
    }

    fn send(&mut self, message: Message) -> Result<(), DriveError> {
        self.driver.send(&mut self.app, message)
    }
}
