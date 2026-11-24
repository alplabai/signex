//! Edge pan (#2872, DECISIONS 2026-11-24): while a tool is armed or a drag
//! or move is in progress, the pointer in a thin band at a canvas edge pans
//! the view that way, faster nearer the edge, and what follows the pointer
//! keeps following it. Over a panel the pointer is off the canvas, so the
//! panning stops and the tool stays armed. Board, sheet, footprint and
//! symbol canvases. The timer ticks only while panning is possible and the
//! pointer is in the band.

mod prefs;
#[cfg(test)]
mod tests;

pub use prefs::{EdgePanPrefs, EdgePanSpeed};

use iced::{Subscription, Task};

use crate::app::{Message, Signex, SymbolDocument, TabKind, Tool};
use crate::canvas::edge_pan::{band_px, edge_step};
use crate::canvas::{CanvasEvent, EdgePanLink, EdgePointer};

/// How often the view moves while the pointer stays in the band.
const EDGE_PAN_TICK: std::time::Duration = std::time::Duration::from_millis(16);

/// The canvas the edge pan works on: the one the active document shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgePanSurface {
    Sheet,
    Board,
    Footprint,
    Symbol,
}

impl Signex {
    /// The timer, only while a tick would pan.
    pub(crate) fn edge_pan_subscription(&self) -> Subscription<Message> {
        if self.edge_pan_motion().is_none() {
            return Subscription::none();
        }
        iced::time::every(EDGE_PAN_TICK).map(|_| Message::CanvasEvent(CanvasEvent::EdgePanTicked))
    }

    /// One tick: pan the view toward the edge the pointer is near, and let
    /// what follows the pointer read the point the move brought under it.
    pub(crate) fn edge_pan_ticked(&mut self) -> Task<Message> {
        let Some((surface, pointer, (x, y))) = self.edge_pan_motion() else {
            return Task::none();
        };
        // The view moves the other way: at the right edge, what lies to the
        // right comes in.
        let (dx, dy) = (-x, -y);
        match surface {
            EdgePanSurface::Sheet => {
                let canvas = self.interaction_state.active_canvas_mut();
                canvas.pan_view(dx, dy);
                canvas.edge_pan.mark_view_moved();
                let world = canvas.screen_to_world(pointer.at);
                let zoom_pct = canvas.zoom_percent();
                let moved = self.handle_canvas_interaction_event(CanvasEvent::ViewMoved);
                let followed = self.update(Message::CanvasEvent(CanvasEvent::CursorAt {
                    x: world.x,
                    y: world.y,
                    zoom_pct,
                }));
                Task::batch([moved, followed])
            }
            EdgePanSurface::Board => {
                let canvas = &self.interaction_state.pcb_canvas;
                canvas.pan_by(dx, dy);
                canvas.edge_pan.mark_view_moved();
                let bounds = iced::Rectangle::with_size(pointer.size);
                let report = canvas.pointer_report(pointer.at, bounds);
                let moved = self.handle_canvas_interaction_event(CanvasEvent::ViewMoved);
                let followed = self.update(report);
                Task::batch([moved, followed])
            }
            EdgePanSurface::Symbol => {
                if let Some(editor) = self.active_symbol_document_mut() {
                    editor.edge_panned(dx, dy, pointer.at);
                    // The canvas replays the pointer at its next redraw.
                    self.interaction_state.symbol_edge_pan.mark_view_moved();
                }
                Task::none()
            }
            EdgePanSurface::Footprint => {
                // Its camera lives in the canvas widget, which takes the pan.
                if let Some(editor) = self.active_footprint_editor() {
                    editor.state.edge_pan.owe_pan(dx, dy);
                    editor.canvas_cache.clear();
                }
                Task::none()
            }
        }
    }

    /// The surface, the pointer and the step of the next tick; `None` while
    /// a tick would not pan.
    pub(crate) fn edge_pan_motion(&self) -> Option<(EdgePanSurface, EdgePointer, (f32, f32))> {
        let prefs = self.ui_state.edge_pan;
        if !prefs.enabled {
            return None;
        }
        let surface = self.edge_pan_surface()?;
        if !self.edge_pan_possible(surface) {
            return None;
        }
        let pointer = self.edge_pan_link(surface)?.pointer()?;
        let band = band_px();
        let step = edge_step(pointer, band, prefs.speed.step_px(band))?;
        // A dialog over the canvas has the pointer, even one a key opened
        // with the pointer still in the band.
        if self.dialogs_open_above_grid_snap_popover() > 0 {
            return None;
        }
        Some((surface, pointer, step))
    }

    /// The focus left a window: forget the pointer on every canvas the edge
    /// pan reads, so no tick pans until a canvas sees the pointer again.
    pub(crate) fn forget_edge_pan_pointers(&self) {
        self.interaction_state
            .active_canvas()
            .edge_pan
            .forget_pointer();
        self.interaction_state.pcb_canvas.edge_pan.forget_pointer();
        self.interaction_state.symbol_edge_pan.forget_pointer();
        for editor in self.document_state.footprint_editors.values() {
            editor.state.edge_pan.forget_pointer();
        }
        if let Some(editor) = self.board_sketch_editor() {
            editor.state.edge_pan.forget_pointer();
        }
    }

    /// The canvas the active document shows, when it is one that pans.
    pub(crate) fn edge_pan_surface(&self) -> Option<EdgePanSurface> {
        // A board in sketch mode edits through the footprint canvas.
        if self.board_sketch_editor().is_some() {
            return Some(EdgePanSurface::Footprint);
        }
        if self.has_active_schematic() {
            return Some(EdgePanSurface::Sheet);
        }
        if self.has_active_pcb() {
            return Some(EdgePanSurface::Board);
        }
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        match &tab.kind {
            TabKind::SymbolEditor(_) => Some(EdgePanSurface::Symbol),
            TabKind::FootprintEditor(_) => Some(EdgePanSurface::Footprint),
            _ => None,
        }
    }

    /// Where that canvas last saw the pointer.
    pub(crate) fn edge_pan_link(&self, surface: EdgePanSurface) -> Option<&EdgePanLink> {
        match surface {
            EdgePanSurface::Sheet => Some(&self.interaction_state.active_canvas().edge_pan),
            EdgePanSurface::Board => Some(&self.interaction_state.pcb_canvas.edge_pan),
            EdgePanSurface::Symbol => self
                .active_symbol_document()
                .map(|_| &self.interaction_state.symbol_edge_pan),
            EdgePanSurface::Footprint => self.active_footprint_editor().map(|e| &e.state.edge_pan),
        }
    }

    /// A tool is armed, or a drag or move is in progress, on `surface`.
    fn edge_pan_possible(&self, surface: EdgePanSurface) -> bool {
        match surface {
            EdgePanSurface::Sheet => {
                let interaction = &self.interaction_state;
                interaction.current_tool != Tool::Select
                    || interaction.pending_power.is_some()
                    || interaction.placement.is_some()
                    || self.ui_state.move_pick.is_some()
                    || interaction.active_canvas().move_behaviour.get().is_some()
            }
            EdgePanSurface::Board => {
                let canvas = &self.interaction_state.pcb_canvas;
                canvas.is_clicks_only() || canvas.is_carrying()
            }
            EdgePanSurface::Symbol => self
                .active_symbol_document()
                .is_some_and(SymbolDocument::edge_pan_armed),
            EdgePanSurface::Footprint => self
                .active_footprint_editor()
                .is_some_and(|editor| editor.state.edge_pan_armed()),
        }
    }

    /// The symbol editor of the active tab (not its pin table).
    fn active_symbol_document(&self) -> Option<&SymbolDocument> {
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        match &tab.kind {
            TabKind::SymbolEditor(path) => self.document_state.symbol_editors.get(path),
            _ => None,
        }
    }

    fn active_symbol_document_mut(&mut self) -> Option<&mut SymbolDocument> {
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let path = match &tab.kind {
            TabKind::SymbolEditor(path) => path.clone(),
            _ => return None,
        };
        self.document_state.symbol_editors.get_mut(&path)
    }
}
