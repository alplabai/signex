//! Graphite canvas zoom pill: `- 100% + Fit` in the canvas's bottom-right
//! corner. It sends the messages View > Zoom In, Zoom Out and Fit All send,
//! so it adds no behaviour of its own.

use iced::widget::{Row, Widget, button, container, text};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use super::*;
use crate::menu_bar::MenuMessage;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, is_graphite, ti};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// Gap between the pill and the canvas edges.
const EDGE_GAP: f32 = 12.0;
/// Inner padding of the pill.
const PAD: f32 = 3.0;
/// Gap between the pill's controls.
const GAP: f32 = 2.0;
/// Width and height of a `-` / `+` button.
const MINI: f32 = 24.0;
/// Width of the percentage readout.
const READOUT_W: f32 = 44.0;
/// Width of the Fit button.
const FIT_W: f32 = 34.0;
/// Radius of the controls inside the pill (the mock's 6 px).
const CONTROL_RADIUS: f32 = 6.0;

/// Outer size of the pill: padding and the 1 px border on every side.
pub(super) fn pill_size() -> (f32, f32) {
    let inner_w = MINI + READOUT_W + MINI + FIT_W + 3.0 * GAP;
    (inner_w + 2.0 * (PAD + 1.0), MINI + 2.0 * (PAD + 1.0))
}

/// Top-left of the pill for a canvas at `canvas`.
pub(super) fn pill_origin(canvas: iced::Rectangle) -> (f32, f32) {
    let (w, h) = pill_size();
    (
        canvas.x + canvas.width - EDGE_GAP - w,
        canvas.y + canvas.height - EDGE_GAP - h,
    )
}

fn control_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let ink = ti(tokens.text_secondary);
    let hover = ti(tokens.hover);
    move |_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(hover)),
            _ => None,
        },
        text_color: ink,
        border: Border {
            radius: CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The pill element at its fixed [`pill_size`].
fn view<'a>(tokens: &ThemeTokens, zoom_percent: f64) -> Element<'a, Message> {
    let r = resolve(tokens);
    let line = ti(tokens.border);
    let ink = ti(tokens.text_secondary);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);

    let mini = |label: &'static str, on_press: Message| -> Element<'a, Message> {
        button(
            container(text(label).size(TS.title).color(ink))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(MINI)
        .height(MINI)
        .padding(0)
        .on_press(on_press)
        .style(control_style(tokens))
        .boxed()
    };
    let readout: Element<'a, Message> = container(
        text(format!("{zoom_percent:.0}%"))
            .size(TS.caption)
            .font(mono)
            .color(ink),
    )
    .width(READOUT_W)
    .height(MINI)
    .center_x(READOUT_W)
    .center_y(MINI)
    .boxed();
    let fit: Element<'a, Message> = button(
        container(text("Fit").size(TS.caption).color(ink))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(FIT_W)
    .height(MINI)
    .padding(0)
    .on_press(Message::CanvasEvent(crate::canvas::CanvasEvent::FitAll))
    .style(control_style(tokens))
    .boxed();

    let row = Row::<Element<'a, Message>>::new()
        .spacing(GAP)
        .align_y(iced::Alignment::Center)
        .push(mini("\u{2212}", Message::Menu(MenuMessage::ZoomOut)))
        .push(readout)
        .push(mini("+", Message::Menu(MenuMessage::ZoomIn)))
        .push(fit);
    let (w, h) = pill_size();
    container(row)
        .padding(PAD)
        .width(w)
        .height(h)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.raised)),
            border: Border {
                width: 1.0,
                radius: Radii::GRAPHITE.card.into(),
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

impl Signex {
    /// The zoom pill over the main window's schematic canvas. Graphite
    /// themes only; an undocked window has no known canvas rectangle and
    /// the zoom messages act on the main canvas, so it gets none.
    pub(in crate::app::view) fn canvas_zoom_pill_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let tokens = &self.document_state.panel_ctx.tokens;
        if !is_graphite(tokens) || self.ui_state.main_window_id != Some(window_id) {
            return None;
        }
        self.document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let zoom = self
            .interaction_state
            .canvas_for_window(window_id)
            .zoom_percent();
        let at = pill_origin(self.canvas_bounds());
        Some(super::super::translate::Translate::new(view(tokens, zoom), at).boxed())
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
