//! Board sketch mode in the main window (#2234): while the mode is on, the
//! sketch canvas over the board with the sketch mode bar, the same one the
//! footprint editor shows (#2518): Bake preview · Discard changes · Bake.
//! The PCB Active Bar's Board Sketch button lives in `pcb_active_bar`.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};

use crate::app::state::BoardSketchSession;
use crate::app::{BoardSketchMsg, Message, PcbMsg, Signex};
use crate::library::editor::standalone::sketch_exit::{
    MODE_BAR_INSET, ModeBarLook, ModeBarMessages, mode_bar,
};
use crate::styles::Spacing;

const SP: Spacing = Spacing::DEFAULT;

impl Signex {
    /// The main window's body while its board is in sketch mode; `None`
    /// otherwise, and in every undocked window.
    pub(in crate::app::view) fn board_sketch_view(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        if self.ui_state.main_window_id != Some(window_id) {
            return None;
        }
        let session = self.board_sketch_in_view()?;
        let tokens = &self.document_view.panel_ctx.tokens;
        let canvas = crate::library::editor::standalone::view_board_sketch(
            &session.editor,
            tokens,
            &self.ui_state.i18n,
            self.library.mirror_cards.get(&session.board_path),
        )
        .map(Message::Library);
        // The constraint / pattern menu at the right-click (mockup row 188), measured
        // in the UI font and kept inside the canvas, under the mode bar.
        let menu = crate::library::editor::footprint::sketch_mode::constraint_menu::overlay(
            &session.editor,
            tokens,
            &self.ui_state.i18n,
            &self.document_view.panel_ctx.ui_font_name,
        )
        .map(Message::Library);
        let mut layers = stack![canvas, menu, self.board_sketch_bar(session)];
        // Over everything: the sketch waits for Import or Cancel (mockup row 179c).
        if let Some(dialog) = session.dxf_dialog.as_deref() {
            layers = layers.push(super::board_sketch_dxf::overlay(
                dialog,
                tokens,
                &self.ui_state.i18n,
            ));
        }
        Some(layers.boxed())
    }

    /// The mode bar: the mode's name, then Bake preview, Discard changes and
    /// Bake ("Apply to board"), which commits one board command and leaves.
    /// The last refusal is the error toast just above it (mockup row 110).
    fn board_sketch_bar<'a>(&'a self, session: &'a BoardSketchSession) -> Element<'a, Message> {
        let i18n = &self.ui_state.i18n;
        let tokens = &self.document_view.panel_ctx.tokens;
        let sketch = |event| Message::Pcb(PcbMsg::Sketch(event));
        let look = ModeBarLook {
            title: t!(i18n, ids::BOARD_SKETCH_TITLE),
            status: None,
            bake_tooltip: t!(i18n, ids::BOARD_SKETCH_BAKE_TOOLTIP),
        };
        let msgs = ModeBarMessages {
            preview: sketch(BoardSketchMsg::BakePreviewToggled),
            discard: sketch(BoardSketchMsg::DiscardRequested),
            confirm_discard: sketch(BoardSketchMsg::DiscardConfirmed),
            keep_editing: sketch(BoardSketchMsg::DiscardCancelled),
            bake: Some(sketch(BoardSketchMsg::BakeRequested)),
        };
        // The bake preview is the Properties Bake panel (mockup row 179), not
        // a card over the canvas; this bar's Bake preview button closes it.
        let exit = &session.editor.state.sketch_exit;
        let mut layers = column![].spacing(SP.m);
        if let Some(toast) = session.toast.as_ref() {
            layers = layers.push(toast::refusal_toast(toast, tokens, i18n));
        }
        layers = layers.push(mode_bar(look, exit, msgs, tokens, i18n));
        container(layers)
            .padding(MODE_BAR_INSET)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Bottom)
            .boxed()
    }
}

mod toast;

#[cfg(test)]
mod bake_panel_tests;
