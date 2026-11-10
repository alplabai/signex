//! Board sketch mode in the main window (#2234): the PCB Active Bar's Board
//! Sketch button, and, while the mode is on, the sketch canvas over the
//! board with the mode's bar (Apply, Leave, Discard).

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, column, container, row, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
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
        Some(stack![canvas, self.board_sketch_bar(session)].into())
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
        if let Some(refusal) = session.refusal.as_deref() {
            bar = bar.push(
                text(signex_i18n::UserText::new(refusal))
                    .size(TS.caption)
                    .color(theme_ext::error_color(tokens)),
            );
        }
        let apply = session
            .has_changes()
            .then_some(BoardSketchMsg::ApplyRequested)
            .and_then(sketch);
        bar = bar
            .push(dialog_button(
                &t!(i18n, ids::BOARD_SKETCH_APPLY).into_string(),
                DialogButtonKind::Primary,
                apply,
                tokens,
            ))
            .push(dialog_button(
                &t!(i18n, ids::BOARD_SKETCH_LEAVE).into_string(),
                DialogButtonKind::Secondary,
                sketch(BoardSketchMsg::LeaveRequested),
                tokens,
            ))
            .push(dialog_button(
                &t!(i18n, ids::BOARD_SKETCH_DISCARD).into_string(),
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

    /// The PCB Active Bar of the main window: Board Sketch, which runs the
    /// `board_sketch` command. Nothing in an undocked window.
    pub(in crate::app::view) fn pcb_active_bar(&self, is_main: bool) -> Element<'_, Message> {
        if !is_main {
            return Space::new().boxed();
        }
        let tid = self.ui_state.effective_theme_id();
        let tokens = &self.document_state.panel_ctx.tokens;
        let items = pcb_bar_items(tid, &self.ui_state.i18n);
        column![
            Space::new().height(crate::active_bar::bar_top_margin()),
            container(signex_widgets::active_bar::view(items, tokens))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        ]
        .boxed()
    }
}

/// The PCB Active Bar's buttons: Board Sketch, which runs the
/// `board_sketch` command like its menu row.
pub(in crate::app::view) fn pcb_bar_items(
    tid: ThemeId,
    i18n: &I18n,
) -> Vec<ActiveBarItem<Message>> {
    let run = crate::keymap::AppCommandId::new("board_sketch")
        .ok()
        .map(|id| Message::Menu(crate::menu_bar::MenuMessage::Command(id)));
    vec![ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(crate::icons::icon_dd_rect(tid)),
        tooltip: t!(i18n, ids::BOARD_SKETCH_TOOLTIP).into_string(),
        enabled: run.is_some(),
        selected: false,
        on_press: run,
        ..ActiveBarButton::default()
    })]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #2234: the PCB Active Bar's button runs `board_sketch`.
    #[test]
    fn the_pcb_bar_button_runs_board_sketch() {
        let items = pcb_bar_items(ThemeId::GraphiteDark, &I18n::en_us());
        let [ActiveBarItem::Button(button)] = items.as_slice() else {
            panic!("one button");
        };
        assert!(matches!(
            &button.on_press,
            Some(Message::Menu(crate::menu_bar::MenuMessage::Command(id)))
                if id.as_str() == "board_sketch"
        ));
        assert!(button.enabled);
    }
}
