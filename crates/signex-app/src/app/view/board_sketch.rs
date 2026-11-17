//! Board sketch mode in the main window (#2234): while the mode is on, the
//! sketch canvas over the board with the mode's bar (Apply, Leave, Discard).
//! The PCB Active Bar's Board Sketch button lives in `pcb_active_bar`.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{container, row, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{text, theme_ext};

use crate::app::state::BoardSketchSession;
use crate::app::{BoardSketchMsg, Message, PcbMsg, Signex};
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

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

    /// The mode's bar: its name, the last refusal, Apply, Leave and Discard.
    fn board_sketch_bar<'a>(&'a self, session: &'a BoardSketchSession) -> Element<'a, Message> {
        let i18n = &self.ui_state.i18n;
        let tokens = &self.document_state.panel_ctx.tokens;
        let sketch = |event| Some(Message::Pcb(PcbMsg::Sketch(event)));
        let mut bar = row![
            text(t!(i18n, ids::BOARD_SKETCH_TITLE))
                .size(TS.body)
                .color(theme_ext::text_primary(tokens)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);
        if let Some(refusal) = session.refusal.as_ref() {
            use signex_i18n::RenderTranslatable as _;
            bar = bar.push(
                text(refusal.localized(i18n))
                    .size(TS.caption)
                    .color(theme_ext::error_color(tokens))
                    .boxed(),
            );
        }
        let apply = session
            .has_changes()
            .then_some(BoardSketchMsg::ApplyRequested)
            .and_then(sketch);
        bar = bar
            .push(dialog_button(
                t!(i18n, ids::BOARD_SKETCH_APPLY),
                DialogButtonKind::Primary,
                apply,
                tokens,
            ))
            .push(dialog_button(
                t!(i18n, ids::BOARD_SKETCH_LEAVE),
                DialogButtonKind::Secondary,
                sketch(BoardSketchMsg::LeaveRequested),
                tokens,
            ))
            .push(dialog_button(
                t!(i18n, ids::BOARD_SKETCH_DISCARD),
                DialogButtonKind::Danger,
                sketch(BoardSketchMsg::DiscardRequested),
                tokens,
            ));
        container(
            container(bar)
                .padding([SP.s, SP.m])
                .style(crate::styles::modal_card(tokens)),
        )
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Bottom)
        .boxed()
    }
}
