//! A Notebook badge under the pointer (mockup row 197): hovering it shows
//! its card, which lists the blocks about the part or net; clicking it
//! opens the notebook at the first block, before the click would select.

use iced::{Element, Task, Widget as _};
use signex_notebook::{BadgeTarget, badge_of};

use super::NotebookReferenceMsg;
use crate::app::view::canvas_region;
use crate::app::{Message, Signex};
use crate::schematic_runtime::notebook_badges::{badge_at, placed_badges};

/// The width the badge card is laid out at, px.
const CARD_W: f32 = crate::styles::chrome_px(260.0);
/// The height budget of one block line, px, for placing the card.
const LINE_H: f32 = crate::styles::chrome_px(18.0);

fn msg(m: NotebookReferenceMsg) -> Message {
    Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::NotebookReferences(m),
    ))
}

impl Signex {
    /// The badge drawn at the world point `(x, y)` of the sheet in view.
    pub(crate) fn notebook_badge_at(&self, x: f64, y: f64) -> Option<BadgeTarget> {
        let path = self.document_state.active_path.as_deref()?;
        let marks = self.notebook_badge_view(Some(path)).badges?;
        let snapshot = self.active_render_snapshot()?;
        let canvas = self.interaction_state.active_canvas();
        let placed = placed_badges(snapshot, marks, &canvas.designators);
        let scale = canvas.live_camera().2;
        badge_at(&placed, [x as f32, y as f32], scale).map(|badge| badge.target.clone())
    }

    /// Keep the badge under the pointer, for its card.
    pub(crate) fn observe_notebook_badge_hover(&mut self, x: f64, y: f64) {
        self.ui_state.annotations.hovered_badge = self.notebook_badge_at(x, y);
    }

    /// A click on a badge opens the notebook at its first block; `None`
    /// when the click is on no badge, so it selects as before.
    pub(crate) fn click_notebook_badge(&mut self, x: f64, y: f64) -> Option<Task<Message>> {
        let target = self.notebook_badge_at(x, y)?;
        let badges = std::sync::Arc::clone(&self.ui_state.annotations.badges);
        let first = badge_of(&badges, &target)?.blocks.first()?;
        self.open_notebook_at(&first.file, first.line);
        Some(Task::none())
    }

    /// The card of the badge under the pointer, beside it on the canvas.
    pub(in crate::app) fn view_notebook_badge_card(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let target = self.ui_state.annotations.hovered_badge.as_ref()?;
        let badge = badge_of(&self.ui_state.annotations.badges, target)?;
        let tokens = &self.document_state.panel_ctx.tokens;
        let card = super::views::badge_card(badge, &self.ui_state.i18n, tokens).map(msg);
        let height = LINE_H * (badge.blocks.len() as f32 + 3.0);
        let bounds = canvas_region::card_bounds(
            self.ui_state.main_window_id == Some(window_id),
            self.canvas_bounds(),
            self.ui_state.window_size,
        );
        let (x, y) = canvas_region::place_card(
            self.interaction_state.last_mouse_pos,
            (CARD_W, height),
            bounds,
        );
        let sized = iced::widget::container(card).width(iced::Length::Fixed(CARD_W));
        Some(signex_widgets::Translate::new(sized.boxed(), (x, y)).boxed())
    }
}
