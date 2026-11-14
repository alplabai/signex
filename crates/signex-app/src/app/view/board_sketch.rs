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
        if let Some(refusal) = session.refusal.as_deref() {
            bar = bar.push(
                text(signex_i18n::UserText::new(refusal))
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

/// The PCB Active Bar's buttons, each running its command like its menu
/// row: Board Sketch (`board_sketch`) and Rotate View (`view_rotate`, a
/// quarter turn clockwise, #2316).
pub(in crate::app::view) fn pcb_bar_items(
    tid: ThemeId,
    i18n: &I18n,
) -> Vec<ActiveBarItem<Message>> {
    let button = |command: &str, icon, tooltip| {
        let run = crate::keymap::AppCommandId::new(command)
            .ok()
            .map(|id| Message::Menu(crate::menu_bar::MenuMessage::Command(id)));
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Svg(icon),
            tooltip,
            enabled: run.is_some(),
            selected: false,
            on_press: run,
            ..ActiveBarButton::default()
        })
    };
    vec![
        button(
            "board_sketch",
            crate::icons::icon_board_sketch(tid),
            t!(i18n, ids::BOARD_SKETCH_TOOLTIP).into_string(),
        ),
        button(
            "view_rotate",
            crate::icons::icon_view_rotate(tid),
            t!(i18n, ids::PCB_VIEW_ROTATE_TOOLTIP).into_string(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(button: &ActiveBarButton<Message>, command: &str) -> bool {
        matches!(
            &button.on_press,
            Some(Message::Menu(crate::menu_bar::MenuMessage::Command(id)))
                if id.as_str() == command
        )
    }

    /// #2234 and #2316: the PCB Active Bar's buttons run `board_sketch` and
    /// `view_rotate`.
    #[test]
    fn the_pcb_bar_buttons_run_board_sketch_and_view_rotate() {
        let tid = ThemeId::GraphiteDark;
        let items = pcb_bar_items(tid, &I18n::en_us());
        let [ActiveBarItem::Button(sketch), ActiveBarItem::Button(rotate)] = items.as_slice()
        else {
            panic!("two buttons");
        };
        assert!(runs(sketch, "board_sketch") && sketch.enabled);
        assert!(runs(rotate, "view_rotate") && rotate.enabled);
        assert_eq!(rotate.tooltip, "Rotate View 90° Clockwise");
        let ActiveBarIcon::Svg(handle) = &rotate.icon else {
            panic!("an SVG icon");
        };
        assert_eq!(handle, &crate::icons::icon_view_rotate(tid));
    }

    /// Round 9 of #2262: the button says what it is, in its tooltip and its
    /// board glyph, not the generic rectangle.
    #[test]
    fn the_pcb_bar_button_is_named_and_drawn_as_board_sketch() {
        let tid = ThemeId::GraphiteDark;
        let items = pcb_bar_items(tid, &I18n::en_us());
        let Some(ActiveBarItem::Button(button)) = items.first() else {
            panic!("a first button");
        };
        assert!(
            button.tooltip.starts_with("Board Sketch"),
            "{}",
            button.tooltip
        );
        let ActiveBarIcon::Svg(handle) = &button.icon else {
            panic!("an SVG icon");
        };
        assert_eq!(handle, &crate::icons::icon_board_sketch(tid));
        assert_ne!(handle, &crate::icons::icon_dd_rect(tid));
    }
}
