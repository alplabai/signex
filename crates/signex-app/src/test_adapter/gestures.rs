//! The adapter's gestures, in paths and mm: the routes that have no command
//! id yet but that the safety net needs (#2326, slice C decisions of
//! 2026-11-18). A tree open and a History restore are the only ways to the
//! read half of the generation check (`finish_claim`); the drag is the
//! event-granularity baseline's input. A gesture that finds nothing to act
//! on says so with [`AdapterError::NothingToActOn`].

use std::path::Path;

use iced::Task;

use super::{AdapterError, Pending, TestAdapter};
use crate::app::{Message, UiMsg};
use crate::canvas::{CanvasEvent, MoveConnections};

impl TestAdapter {
    /// Double-click `path` in the project tree of the open project that
    /// holds it. The read runs in the returned handle.
    pub fn open_from_tree(&mut self, path: &Path) -> Result<Pending, AdapterError> {
        let projects = &self.app.document_state.projects;
        let (index, name) = projects
            .iter()
            .enumerate()
            .find_map(|(index, project)| {
                let relative = path.strip_prefix(&project.data.dir).ok()?;
                Some((index, relative.to_string_lossy().into_owned()))
            })
            .ok_or_else(|| nothing(format!("no open project holds {}", path.display())))?;
        let task = self
            .app
            .open_project_tree_document(&[index], name)
            .map_err(|error| nothing(format!("the tree refused the open: {error:#}")))?;
        Ok(self.hold(task))
    }

    /// Restore the document at `path` to the commit `sha` from its History
    /// panel, confirming the second click a document with unsaved edits asks
    /// for. The restore runs in the returned handle.
    pub fn restore(&mut self, path: &Path, sha: &str) -> Result<Pending, AdapterError> {
        self.activate(path)?;
        let first = self.app.click_restore_on_listed_history(sha);
        let armed = self.app.document_view.history.pending_restore.as_deref() == Some(sha);
        let task = if armed {
            Task::batch([first, self.app.click_restore_on_listed_history(sha)])
        } else {
            first
        };
        if !self.app.document_state.pending_restores.contains(path) {
            return Err(nothing(format!(
                "the restore of {} was refused",
                path.display()
            )));
        }
        Ok(self.hold(task))
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
    ) -> Result<(), AdapterError> {
        self.activate(path)?;
        self.send(Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: from.0,
            world_y: from.1,
        }))?;
        if self
            .app
            .interaction_state
            .active_canvas()
            .selected
            .is_empty()
        {
            return Err(nothing(format!("no item at {from:?}")));
        }
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            let x = from.0 + (to.0 - from.0) * t;
            let y = from.1 + (to.1 - from.1) * t;
            self.drag_step(x, y)?;
        }
        let before = self.undo_position(path);
        self.send(Message::CanvasEvent(CanvasEvent::MoveSelected {
            dx: to.0 - from.0,
            dy: to.1 - from.1,
            connections: MoveConnections::Preference,
        }))?;
        if self.undo_position(path) == before {
            return Err(nothing(format!("the drop to {to:?} recorded no undo step")));
        }
        Ok(())
    }

    /// Where the sheet at `path` stands in its history.
    fn undo_position(&self, path: &Path) -> Option<usize> {
        self.app
            .document_state
            .engines
            .get(path)
            .map(|engine| engine.undo_position())
    }

    pub(super) fn activate(&mut self, path: &Path) -> Result<(), AdapterError> {
        if self.app.activate_tab_for_path(path) {
            return Ok(());
        }
        Err(nothing(format!("no tab shows {}", path.display())))
    }

    /// One pointer move of a drag: what the canvas publishes, then what the
    /// window subscription sends. No layout drag reads the window position,
    /// so it carries the mm value.
    fn drag_step(&mut self, x: f64, y: f64) -> Result<(), AdapterError> {
        self.send(Message::CanvasEvent(CanvasEvent::CursorAt {
            x: x as f32,
            y: y as f32,
            zoom_pct: 100.0,
        }))?;
        self.send(Message::Ui(UiMsg::DragMove(x as f32, y as f32)))
    }

    fn send(&mut self, message: Message) -> Result<(), AdapterError> {
        self.driver
            .send(&mut self.app, message)
            .map_err(|error| AdapterError::Unfinished(error.to_string()))
    }
}

fn nothing(why: String) -> AdapterError {
    AdapterError::NothingToActOn(why)
}
