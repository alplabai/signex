//! The title bar a borderless dialog window draws in place of the OS one:
//! the Signex mark, the title, an optional subtitle and the Graphite close.
//! Pressing the bar moves the window; the close runs the OS close path, so
//! the dialog's own cancel (and any dirty guard) still decides.

use std::sync::LazyLock;

use iced::widget::{Row, Space, container, mouse_area, svg, text};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use super::*;
use crate::styles::{Spacing, TypeScale, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of the title bar of a borderless dialog window.
pub(crate) const DIALOG_TITLE_BAR_H: f32 = super::header::DIALOG_HEADER_H;
/// Side of the mark in the title bar.
const MARK_SIZE: f32 = 16.0;

/// The Signex mark, as in the main window's menu bar.
static MARK: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        include_bytes!("../../../../assets/icons/chrome/signex_mark.svg").as_slice(),
    )
});

/// The title bar of the borderless dialog window `window`.
pub(crate) fn dialog_title_bar<'a>(
    window: iced::window::Id,
    title: String,
    subtitle: Option<String>,
    theme_tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(theme_tokens);
    let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    line = line.push(svg(MARK.clone()).width(MARK_SIZE).height(MARK_SIZE).boxed());
    line = line.push(
        text(title)
            .size(TS.heading)
            .color(ti(theme_tokens.text))
            .boxed(),
    );
    if let Some(sub) = subtitle.filter(|s| !s.trim().is_empty()) {
        line = line.push(text(sub).size(TS.label).color(r.ink3).boxed());
    }
    line = line.push(Space::new().width(Length::Fill).boxed());
    line = line.push(super::header::neutral_close_button(
        theme_tokens,
        Message::Window(WindowMsg::DialogCloseClicked(window)),
    ));
    let (paper, rule) = (ti(theme_tokens.paper), r.line_soft);
    let bar = container(line)
        .padding([0.0, SP.l])
        .width(Length::Fill)
        .height(Length::Fixed(DIALOG_TITLE_BAR_H))
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| {
            let mut border = Border {
                color: rule,
                ..Border::default()
            };
            border.bottom = iced::border::Side::default().width(1.0);
            container::Style {
                background: Some(Background::Color(paper)),
                border,
                ..container::Style::default()
            }
        });
    mouse_area(bar)
        .on_press(Message::Window(WindowMsg::DialogTitleBarPressed(window)))
        .boxed()
}

impl Signex {
    /// The whole of the dialog window `window` showing `modal`: the body on
    /// the dialog surface, under the title bar (and inside resize edges for
    /// a resizable dialog) when the window was opened borderless.
    pub(in crate::app::view) fn view_dialog_window(
        &self,
        window: iced::window::Id,
        modal: crate::app::state::ModalId,
    ) -> Element<'_, Message> {
        let theme_tokens = &self.document_state.panel_ctx.tokens;
        let body = container(self.view_detached_modal(window, modal))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::styles::modal_card(theme_tokens))
            .boxed();
        if !self.dialog_window_is_borderless(window) {
            return body;
        }
        let bar = dialog_title_bar(
            window,
            self.title(window),
            self.detached_modal_subtitle(modal),
            theme_tokens,
        );
        let framed = iced::widget::column![bar, body]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed();
        if crate::app::handlers::erc::modal_is_fixed(modal) {
            return framed;
        }
        iced::widget::stack![
            framed,
            Self::resize_edges_overlay_with(move |direction| {
                Message::Window(WindowMsg::DialogEdgePressed(window, direction))
            })
        ]
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mark the bar draws is the bundled Signex mark.
    #[test]
    fn the_title_bar_mark_is_the_signex_mark() {
        let src = include_str!("../../../../assets/icons/chrome/signex_mark.svg");
        assert!(src.contains("aria-label=\"Signex\""));
    }

    #[test]
    fn the_title_bar_builds_for_every_theme() {
        for id in signex_types::theme::ThemeId::BUILTINS {
            let t = signex_types::theme::theme_tokens(*id);
            let w = iced::window::Id::unique();
            let _ = dialog_title_bar(w, "Copy to Library".into(), None, &t);
            let _ = dialog_title_bar(w, "ERC".into(), Some("Board".into()), &t);
        }
    }
}
