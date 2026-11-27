//! The error toast of a refused board sketch commit (mockup row 110), just
//! above the mode bar at the canvas's bottom left: why the board did not
//! take the sketch and, for a profile that is not closed, where it is open,
//! as a link that frames it on the canvas. Its close button dismisses it.

use iced::widget::tooltip::Position;
use iced::widget::{container, row};
use iced::{Alignment, Background, Border, Color, Element, Padding, Theme, Widget as _};
use signex_i18n::{I18n, Localized, RenderTranslatable as _, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, text, tooltip};

use crate::app::state::BoardSketchRefusalToast;
use crate::app::{BoardSketchMsg, Message, PcbMsg};
use crate::styles::tokens::resolve;
use crate::styles::{
    DialogIcon, Radii, Spacing, TypeScale, dialog_glyph, floating_shadow, ti, tinted_glyph,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The widest the toast grows, in body-text sizes; longer text wraps.
const TOAST_MAX_BODIES: f32 = 40.0;
/// Any theme loads the monochrome close glyph; its style tints it.
const GLYPH_THEME: ThemeId = ThemeId::GraphiteDark;

fn sketch(event: BoardSketchMsg) -> Message {
    Message::Pcb(PcbMsg::Sketch(event))
}

/// The toast: the error glyph, the sentence, the place link when there is
/// a place, and the close button, on the error tint with a floating shadow.
pub(super) fn refusal_toast<'a>(
    toast: &BoardSketchRefusalToast,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let ink = ti(tokens.error);
    let fill = resolve(tokens).err_soft;
    let mut line = row![
        dialog_glyph::<Message>(DialogIcon::Error, TS.body, tokens),
        text(toast.sentence.localized(i18n))
            .size(TS.body)
            .color(ink),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if let Some(gap) = toast.gap.as_ref() {
        line = line.push(place_link(
            gap.place_label().localized(i18n),
            ink,
            tokens,
            i18n,
        ));
    }
    line = line.push(close_button(ink, tokens, i18n));
    let shadow = floating_shadow(tokens);
    container(line)
        .max_width(TS.body * TOAST_MAX_BODIES)
        .padding(Padding::from([SP.s, SP.m]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(ink),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            shadow,
            ..container::Style::default()
        })
        .boxed()
}

/// The place, in the mono face, as a link that frames the gap on the
/// canvas; "Zoom to the gap" on hover.
fn place_link<'a>(
    label: Localized,
    ink: Color,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let link = button(text(label).size(TS.body).font(mono).color(ink))
        .padding(0)
        .on_press(sketch(BoardSketchMsg::RefusalGapClicked))
        .style(|_: &Theme, _| button::Style::default());
    with_tip(
        link.boxed(),
        t!(i18n, ids::BOARD_SKETCH_REFUSED_PLACE_TOOLTIP),
        tokens,
    )
}

/// The close glyph that dismisses the toast; "Dismiss" on hover.
fn close_button<'a>(ink: Color, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, Message> {
    let glyph = tinted_glyph::<Message>(
        crate::icons::icon_chrome_window_close(GLYPH_THEME),
        TS.body,
        ink,
    );
    let close = button(glyph)
        .padding(0)
        .on_press(sketch(BoardSketchMsg::RefusalToastDismissed))
        .style(|_: &Theme, _| button::Style::default());
    with_tip(
        close.boxed(),
        t!(i18n, ids::BOARD_SKETCH_REFUSED_DISMISS),
        tokens,
    )
}

/// `target` with `tip` above it on hover, in the bar tooltip's look.
fn with_tip<'a>(
    target: Element<'a, Message>,
    tip: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let hint = container(text(tip).size(TS.caption).color(ti(tokens.text)))
        .padding([SP.xs, SP.m])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            r.raised,
            r.line_strong,
        ));
    tooltip(target, hint, Position::Top).gap(SP.xs).boxed()
}
