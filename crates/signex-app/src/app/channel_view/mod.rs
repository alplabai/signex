//! The channel strip under a repeated sheet's canvas and the compiled
//! channel view (#1279). Editor is the editable sheet; a channel tab shows
//! the compiled channels as cards, read-only (the gate is
//! `Signex::refuse_if_channel_view`, at the mutation gateway). The strip's
//! data is `PanelContext::compiled_design.strip`, built in update.

mod cards;
mod strip;

use iced::widget::column;
use iced::{Element, Length, Widget as _};

use crate::app::{Message, Signex};

impl Signex {
    /// `document` with the channel strip under it when the active sheet is
    /// repeated; a channel tab replaces the sheet with the channel cards.
    /// Undocked windows show the sheet alone.
    pub(crate) fn with_channel_strip<'a>(
        &'a self,
        is_main: bool,
        document: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let ctx = &self.document_state.panel_ctx;
        let Some(view) = ctx.compiled_design.strip.as_ref().filter(|_| is_main) else {
            return document;
        };
        let body = match view.active {
            Some(active) => cards::view(ctx, view, active),
            None => document,
        };
        column![body, strip::view(ctx, view)]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    }
}
