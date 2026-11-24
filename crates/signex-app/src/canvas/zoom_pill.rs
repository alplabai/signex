//! Graphite canvas zoom pill: `- 100% + | Fit`. One widget for every canvas:
//! the schematic overlay and the library editors give it their own messages.
//! The schematic pill also names each control's action and shortcut on hover,
//! and its percentage opens the zoom presets (`app/view/overlays/zoom_presets.rs`).

use iced::widget::{Row, Space, Widget, container};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, raw, text, tooltip};

use crate::styles::tokens::resolve;
use crate::styles::{Radii, ti};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Inner padding of the pill.
const PAD: f32 = 3.0;
/// Gap between the pill's controls.
const GAP: f32 = 2.0;
/// Width and height of a `-` / `+` button.
const MINI: f32 = 24.0;
/// Width of the percentage readout.
const READOUT_W: f32 = 48.0;
/// Width of the Fit button.
const FIT_W: f32 = 34.0;
/// Height of the hairline between the `- % +` group and Fit.
const SEPARATOR_H: f32 = 16.0;
/// Gap between the pill and a hover hint.
const TIP_GAP: f32 = 4.0;

/// Zoom levels the preset menu offers, in percent of the default scale.
pub const PRESETS: [u16; 4] = [50, 100, 200, 400];

/// Outer size of the pill: padding and the 1 px border on every side.
pub fn pill_size() -> (f32, f32) {
    // `-`, readout, `+`, the hairline and Fit: four gaps between five cells.
    let inner_w = MINI + READOUT_W + MINI + 1.0 + FIT_W + 4.0 * GAP;
    (inner_w + 2.0 * (PAD + 1.0), MINI + 2.0 * (PAD + 1.0))
}

/// What the pill's controls send.
pub struct PillMessages<M> {
    pub zoom_out: M,
    pub zoom_in: M,
    pub fit_all: M,
    /// Sent by a click on the percentage; `None` leaves it a read-out.
    pub presets: Option<M>,
}

/// The pill's text: the read-out, the Fit label and the hover hints.
pub struct PillText<'a> {
    pub readout: Label<'a>,
    pub fit: Label<'a>,
    /// Hover hints of `-`, `+`, the percentage and Fit, in that order; a
    /// control without one shows none.
    pub hints: [Option<Label<'a>>; 4],
}

fn control_style(
    tokens: &ThemeTokens,
    pinned: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let ink = ti(tokens.text_secondary);
    let hover = ti(tokens.hover);
    move |_: &Theme, status| button::Style {
        background: (pinned || matches!(status, button::Status::Hovered | button::Status::Pressed))
            .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// `control` with `hint` shown above it on hover, in the Graphite hint look
/// (primary fill, primary ink).
fn with_hint<'a, M: 'a>(
    control: Element<'a, M>,
    hint: Option<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let Some(hint) = hint else {
        return control;
    };
    let r = resolve(tokens);
    let card = container(text(hint).size(TS.caption).color(r.primary_ink))
        .padding([SP.xs * 1.5, SP.s * 1.75])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.primary)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    tooltip(control, card, tooltip::Position::Top)
        .gap(TIP_GAP)
        .boxed()
}

/// The pill element at its fixed [`pill_size`]; the three messages are what its
/// buttons send. The library editors' pill: a plain read-out, no hints.
pub fn view<'a, M: Clone + 'a>(
    tokens: &ThemeTokens,
    zoom_percent: f64,
    zoom_out: M,
    zoom_in: M,
    fit_all: M,
) -> Element<'a, M> {
    view_with(
        tokens,
        PillMessages {
            zoom_out,
            zoom_in,
            fit_all,
            presets: None,
        },
        PillText {
            readout: raw(format!("{zoom_percent:.0}%")),
            fit: raw("Fit"),
            hints: [None, None, None, None],
        },
        false,
    )
}

/// The pill with its hover hints and, when `messages.presets` is set, a
/// clickable percentage; `presets_open` keeps the percentage highlighted
/// while its menu is up.
pub fn view_with<'a, M: Clone + 'a>(
    tokens: &ThemeTokens,
    messages: PillMessages<M>,
    labels: PillText<'a>,
    presets_open: bool,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let line = ti(tokens.border);
    let ink = ti(tokens.text_secondary);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let [out_hint, in_hint, readout_hint, fit_hint] = labels.hints;

    let mini = |label: &'static str, on_press: M| -> Element<'a, M> {
        button(
            container(text(raw(label)).size(TS.title).color(ink))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(MINI)
        .height(MINI)
        .padding(0)
        .on_press(on_press)
        .style(control_style(tokens, false))
        .boxed()
    };
    let readout_text = container(text(labels.readout).size(TS.caption).font(mono).color(ink))
        .center_x(Length::Fill)
        .center_y(Length::Fill);
    let readout: Element<'a, M> = match messages.presets {
        Some(open) => button(readout_text)
            .width(READOUT_W)
            .height(MINI)
            .padding(0)
            .on_press(open)
            .style(control_style(tokens, presets_open))
            .boxed(),
        None => readout_text.width(READOUT_W).height(MINI).boxed(),
    };
    let fit: Element<'a, M> = button(
        container(text(labels.fit).size(TS.caption).color(ink))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(FIT_W)
    .height(MINI)
    .padding(0)
    .on_press(messages.fit_all)
    .style(control_style(tokens, false))
    .boxed();
    let soft = r.line_soft;
    let separator: Element<'a, M> = container(Space::new())
        .width(1.0)
        .height(SEPARATOR_H)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(soft)),
            ..container::Style::default()
        })
        .boxed();

    let row = Row::<Element<'a, M>>::new()
        .spacing(GAP)
        .align_y(iced::Alignment::Center)
        .push(with_hint(
            mini("\u{2212}", messages.zoom_out),
            out_hint,
            tokens,
        ))
        .push(with_hint(readout, readout_hint, tokens))
        .push(with_hint(mini("+", messages.zoom_in), in_hint, tokens))
        .push(separator)
        .push(with_hint(fit, fit_hint, tokens));
    let (w, h) = pill_size();
    container(row)
        .padding(PAD)
        .width(w)
        .height(h)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.raised)),
            border: Border {
                width: 1.0,
                radius: Radii::GRAPHITE.button.into(),
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

impl super::CanvasSlot {
    /// Set the zoom to `percent` of the default scale about the canvas centre
    /// (the zoom presets). False when the camera did not change: already
    /// there, or held at a scale limit it already sits on.
    pub fn zoom_to_percent_about_centre(&self, percent: u16) -> bool {
        let current = self.zoom_percent();
        // Within a hundredth of a percent is already there: the read-out shows
        // whole percents, and a factor a hair off 1.0 would still move it.
        if !current.is_finite() || current <= 0.0 || (f64::from(percent) - current).abs() < 0.01 {
            return false;
        }
        let factor = (f64::from(percent) / current) as f32;
        let bounds = self.viewport.get();
        let centre = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        self.camera_mut().zoom_by_factor(centre, factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A preset scales the camera to that share of the default scale, about
    /// the canvas centre.
    #[test]
    fn a_preset_sets_the_scale_about_the_centre() {
        let slot = super::super::CanvasSlot::default();
        slot.set_viewport(iced::Rectangle::new(
            iced::Point::ORIGIN,
            iced::Size::new(800.0, 600.0),
        ));
        let centre = iced::Point::new(400.0, 300.0);
        slot.zoom_to_percent_about_centre(100);
        let (_, _, at_100) = slot.live_camera();
        let centre_world = slot.screen_to_world(centre);
        assert!(slot.zoom_to_percent_about_centre(200));
        let (_, _, at_200) = slot.live_camera();
        assert!((at_200 / at_100 - 2.0).abs() < 1e-4, "{at_100} -> {at_200}");
        let after = slot.screen_to_world(centre);
        assert!((after.x - centre_world.x).abs() < 1e-3, "the centre moved");
        assert!((after.y - centre_world.y).abs() < 1e-3, "the centre moved");
        assert!((slot.zoom_percent() - 200.0).abs() < 0.01);
        assert!(!slot.zoom_to_percent_about_centre(200), "already there");
    }

    /// The hairline sits inside the pill's width: five cells, four gaps.
    #[test]
    fn the_pill_width_counts_the_separator() {
        let (w, h) = pill_size();
        assert_eq!(
            w,
            2.0 * MINI + READOUT_W + FIT_W + 1.0 + 4.0 * GAP + 2.0 * (PAD + 1.0)
        );
        assert_eq!(h, MINI + 2.0 * (PAD + 1.0));
    }
}
