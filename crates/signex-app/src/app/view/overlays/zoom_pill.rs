//! Graphite canvas zoom pill over the schematic canvas's bottom-right corner.
//! `-`, `+` and Fit send the messages View > Zoom In, Zoom Out and Fit All
//! send; each control names its action and shortcut on hover, and the
//! percentage opens the zoom presets (`zoom_presets.rs`). The widget is
//! `crate::canvas::zoom_pill`.

use iced::Element;
use iced::widget::Widget;
use signex_i18n::{menu_ids, t};
use signex_widgets::Label;

use super::*;
use crate::canvas::zoom_pill::{PillMessages, PillText, pill_size, view_with as pill};
use crate::menu_bar::MenuMessage;

/// Gap between the pill and the canvas edges: one body line (12 px at the
/// default size).
const EDGE_GAP: f32 = TS.body;

/// Commands behind `-`, `+` and Fit, whose menu text and shortcut the hints name.
const ZOOM_OUT_COMMAND: &str = "zoom_out_at_cursor";
const ZOOM_IN_COMMAND: &str = "zoom_in_at_cursor";
const FIT_ALL_COMMAND: &str = "zoom_to_fit";

/// Top-left of the pill for a canvas at `canvas`.
pub(in crate::app::view) fn pill_origin(canvas: iced::Rectangle) -> (f32, f32) {
    let (w, h) = pill_size();
    (
        canvas.x + canvas.width - EDGE_GAP - w,
        canvas.y + canvas.height - EDGE_GAP - h,
    )
}

impl Signex {
    /// The zoom pill over the main window's schematic canvas. An undocked
    /// window has no known canvas rectangle and the zoom messages act on
    /// the main canvas, so it gets none.
    ///
    /// The layer spans the window, so it places the pill against the canvas
    /// of the size it is laid out at, not `window_size`: that one only moves
    /// on a resize event, and a stale value put the pill over the right dock
    /// (#2911).
    pub(in crate::app::view) fn canvas_zoom_pill_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        if self.ui_state.main_window_id != Some(window_id) {
            return None;
        }
        self.document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let layer = iced::widget::responsive(move |size: iced::Size| {
            let canvas = self.canvas_bounds_sized(None, (size.width, size.height));
            let at = pill_origin(canvas);
            // The `Translate` spans the window; only the pill may take the wheel.
            signex_widgets::Translate::new(super::scroll_guard(self.zoom_pill(window_id)), at)
                .boxed()
        });
        Some(layer.boxed())
    }

    /// The pill itself, for the canvas of `window_id`.
    fn zoom_pill(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let zoom = self
            .interaction_state
            .canvas_for_window(window_id)
            .zoom_percent();
        let i18n = &self.ui_state.i18n;
        pill(
            tokens,
            PillMessages {
                zoom_out: Message::Menu(MenuMessage::ZoomOut),
                zoom_in: Message::Menu(MenuMessage::ZoomIn),
                fit_all: Message::CanvasEvent(crate::canvas::CanvasEvent::FitAll),
                presets: Some(Message::Ui(UiMsg::ZoomPresetsToggled)),
            },
            PillText {
                readout: t!(i18n, menu_ids::ZOOM_PILL_PERCENT, "percent" => format!("{zoom:.0}"))
                    .into(),
                fit: t!(i18n, menu_ids::ZOOM_PILL_FIT).into(),
                hints: [
                    self.zoom_pill_hint(ZOOM_OUT_COMMAND),
                    self.zoom_pill_hint(ZOOM_IN_COMMAND),
                    Some(t!(i18n, menu_ids::ZOOM_PILL_PRESETS).into()),
                    self.zoom_pill_hint(FIT_ALL_COMMAND),
                ],
            },
            self.interaction_state.zoom_presets_open,
        )
    }

    /// Hover hint of a pill control: the command's menu text and, when the
    /// active shortcut profile binds it, its key ("Zoom In · Ctrl+=").
    /// `None` for an id missing from the catalog.
    pub(in crate::app::view) fn zoom_pill_hint(&self, command_id: &str) -> Option<Label<'static>> {
        let i18n = &self.ui_state.i18n;
        let action = t!(
            i18n,
            crate::keymap::metadata_for_str(command_id)?.menu_label_id()
        );
        let shortcut = self.keymap_shortcut_label(command_id);
        if shortcut.is_empty() {
            return Some(action.into());
        }
        Some(
            t!(
                i18n,
                menu_ids::ZOOM_PILL_HINT,
                "action" => action.into_string(),
                "shortcut" => shortcut,
            )
            .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pill sits `EDGE_GAP` from the canvas's bottom-right corner.
    #[test]
    fn the_pill_hugs_the_bottom_right_corner() {
        let canvas = iced::Rectangle {
            x: 100.0,
            y: 80.0,
            width: 800.0,
            height: 600.0,
        };
        let (w, h) = pill_size();
        let (x, y) = pill_origin(canvas);
        assert_eq!(x + w, 900.0 - EDGE_GAP);
        assert_eq!(y + h, 680.0 - EDGE_GAP);
    }
}
