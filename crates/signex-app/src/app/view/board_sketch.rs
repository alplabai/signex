//! Board sketch mode in the main window (#2234): while the mode is on, the
//! sketch canvas over the board with the sketch mode bar, the same one the
//! footprint editor shows (#2518): Bake preview · Discard changes · Bake.
//! The PCB Active Bar's Board Sketch button lives in `pcb_active_bar`.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{text, theme_ext};

use crate::app::state::BoardSketchSession;
use crate::app::{BoardSketchMsg, Message, PcbMsg, Signex};
use crate::library::editor::standalone::sketch_exit::{
    ModeBarLook, ModeBarMessages, mode_bar, preview_card,
};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
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
        let tokens = &self.document_state.panel_ctx.tokens;
        let canvas = crate::library::editor::standalone::view_board_sketch(
            &session.editor,
            tokens,
            &self.ui_state.i18n,
        )
        .map(Message::Library);
        Some(stack![canvas, self.board_sketch_bar(session)].boxed())
    }

    /// The mode bar, with the bake preview card above it while open: the
    /// mode's name, the last refusal, then Bake preview, Discard changes and
    /// Bake ("Apply to board"), which commits one board command and leaves.
    fn board_sketch_bar<'a>(&'a self, session: &'a BoardSketchSession) -> Element<'a, Message> {
        let i18n = &self.ui_state.i18n;
        let tokens = &self.document_state.panel_ctx.tokens;
        let sketch = |event| Message::Pcb(PcbMsg::Sketch(event));
        let status = session.refusal.as_ref().map(|refusal| {
            use signex_i18n::RenderTranslatable as _;
            text(refusal.localized(i18n))
                .size(TS.caption)
                .color(theme_ext::error_color(tokens))
                .boxed()
        });
        let look = ModeBarLook {
            title: t!(i18n, ids::BOARD_SKETCH_TITLE),
            status,
            bake_tooltip: t!(i18n, ids::BOARD_SKETCH_BAKE_TOOLTIP),
        };
        let msgs = ModeBarMessages {
            preview: sketch(BoardSketchMsg::BakePreviewToggled),
            discard: sketch(BoardSketchMsg::DiscardRequested),
            confirm_discard: sketch(BoardSketchMsg::DiscardConfirmed),
            keep_editing: sketch(BoardSketchMsg::DiscardCancelled),
            bake: Some(sketch(BoardSketchMsg::BakeRequested)),
        };
        let exit = &session.editor.state.sketch_exit;
        let mut overlay = column![].spacing(SP.m);
        if let Some(preview) = session.preview.as_ref().filter(|_| exit.preview) {
            overlay = overlay.push(preview_card(preview, tokens, i18n));
        }
        overlay = overlay.push(mode_bar(look, exit, msgs, tokens, i18n));
        container(overlay)
            .padding(SP.xl)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Bottom)
            .boxed()
    }
}
