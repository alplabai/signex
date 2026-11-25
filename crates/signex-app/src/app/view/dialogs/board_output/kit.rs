//! Parts the board output dialogs share (Fabrication Outputs, Board Print /
//! PDF, Board Reports): the uppercase section head with its count, the soft
//! status banner and its glyphs, the status pill, the segmented look, a
//! form tick box and the greyed form of a dependent control.

use std::sync::LazyLock;

use iced::widget::{Row, Space, column, container, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText};
use signex_types::theme::ThemeTokens;
use signex_widgets::enable_section::dependents_veil;
use signex_widgets::inert::unless_read_only_veiled;
use signex_widgets::{CheckboxLabel as _, Label, SegmentedStyle, checkbox, text};

use crate::app::Message;
use crate::panels::form_look::checkbox_style;
use crate::styles::{HAIRLINE, PillTone, Radii, Spacing, TypeScale, pill, pill_colors, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Gap between a section head's words and its rule (6 px at the 11 px
/// base).
const HEAD_RULE_GAP: f32 = SP.s + SP.xs;
/// Side of a status glyph: the body line.
const GLYPH: f32 = TS.label;

const GLYPH_INFO: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>"##;
const GLYPH_OK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M8 12.5l2.5 2.5L16 9.5"/></svg>"##;
const GLYPH_WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/></svg>"##;

/// The three glyphs parsed once: info, ok, warning.
static GLYPHS: LazyLock<[svg::Handle; 3]> =
    LazyLock::new(|| [GLYPH_INFO, GLYPH_OK, GLYPH_WARN].map(svg::Handle::from_memory));

/// The glyph of `tone`: a circled i for information, a circled tick for
/// success, a triangle for a warning or an error.
fn glyph_of(tone: PillTone) -> svg::Handle {
    let index = match tone {
        PillTone::Ok => 1,
        PillTone::Warn | PillTone::Err => 2,
        PillTone::Info | PillTone::Accent | PillTone::Neutral => 0,
    };
    GLYPHS[index].clone()
}

/// `tone`'s glyph in its ink.
pub(in crate::app::view::dialogs) fn status_glyph<'a>(
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = pill_colors(tokens, tone).1;
    svg(glyph_of(tone))
        .width(Length::Fixed(GLYPH))
        .height(Length::Fixed(GLYPH))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// An uppercase section label (LAYER SETS) with an optional count on the
/// right ("3 of 6"), over a soft rule.
pub(in crate::app::view::dialogs) fn section_head<'a>(
    label: Localized,
    count: Option<Localized>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let mut line: Row<Element<'a, Message>> =
        Row::new().spacing(SP.m).align_y(Alignment::Center).push(
            container(section_label(label, i18n, tokens))
                .width(Length::Fill)
                .boxed(),
        );
    if let Some(count) = count {
        line = line.push(caption(count, tokens));
    }
    column![line, rule(r.line_soft)]
        .spacing(HEAD_RULE_GAP)
        .width(Length::Fill)
        .boxed()
}

/// An uppercase section label: the section size at medium weight in the
/// third ink.
pub(in crate::app::view::dialogs) fn section_label<'a>(
    label: Localized,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let words = label.into_string();
    // The catalog label, already rendered, upper-cased for the locale; it
    // goes back to the widget as finished text.
    text(UserText::new(i18n.upper_for_display(&words)))
        .size(TS.section)
        .font(medium())
        .color(tokens::resolve(tokens).ink3)
        .boxed()
}

/// A full-width hairline in `ink`.
pub(in crate::app::view::dialogs) fn rule<'a>(ink: Color) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(HAIRLINE))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ink)),
            ..container::Style::default()
        })
        .boxed()
}

/// A muted caption line (the caption size in the fourth ink).
pub(in crate::app::view::dialogs) fn caption<'a>(
    words: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(tokens::resolve(tokens).ink4)
        .boxed()
}

/// The medium weight of the UI face.
pub(in crate::app::view::dialogs) fn medium() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A soft banner of `tone`: its glyph, then `body` in the text ink, and
/// `action` on the right.
pub(in crate::app::view::dialogs) fn banner<'a>(
    tone: PillTone,
    body: Element<'a, Message>,
    action: Option<Element<'a, Message>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let fill = pill_colors(tokens, tone).0;
    let ink = ti(tokens.text);
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(status_glyph(tone, tokens))
        .push(container(body).width(Length::Fill).boxed());
    if let Some(action) = action {
        line = line.push(action);
    }
    container(line)
        .padding([SP.m, SP.l - SP.xs])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(ink),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A small status pill ("3 pages", "mirrored").
pub(in crate::app::view::dialogs) fn status_pill<'a>(
    words: impl Into<Label<'a>>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    container(text(words).size(TS.caption))
        .padding([SP.xs / 2.0, SP.s + SP.xs])
        .style(pill(tokens, tone))
        .boxed()
}

/// The segmented look of the dialogs: a sunken track, the picked segment
/// raised.
pub(in crate::app::view::dialogs) fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = tokens::resolve(tokens);
    r.segmented(TS.body)
}

/// The form tick box at the body size; without `msg` it is drawn
/// disabled.
pub(in crate::app::view::dialogs) fn tick<'a>(
    on: bool,
    label: impl Into<Label<'a>>,
    msg: Option<Message>,
) -> Element<'a, Message> {
    let mut check = checkbox(on)
        .style(checkbox_style)
        .size(TICK)
        .labelled(label)
        .text_size(TS.body)
        .spacing(TICK_GAP);
    if let Some(msg) = msg {
        check = check.on_toggle(move |_| msg.clone());
    }
    check.boxed()
}

/// Side of a tick box.
pub(in crate::app::view::dialogs) const TICK: f32 = TS.title;
/// Gap between a tick box and its words.
pub(in crate::app::view::dialogs) const TICK_GAP: f32 = SP.m;

/// `content` indented to line up with the words of a tick box above it.
pub(in crate::app::view::dialogs) fn under_tick<'a>(
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    container(content)
        .padding(Padding {
            left: TICK + TICK_GAP,
            ..Padding::ZERO
        })
        .boxed()
}

/// `control` as it is while `is_live`; otherwise greyed under the dialog
/// colour and deaf to input, as a switched-off section's fields are.
pub(in crate::app::view::dialogs) fn dependent<'a>(
    is_live: bool,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    match dependents_veil(ti(tokens.paper), is_live) {
        Some(veil) => unless_read_only_veiled(true, veil, control),
        None => control,
    }
}
