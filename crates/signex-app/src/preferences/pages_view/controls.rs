//! Controls the Appearance and Editing pages share, drawn as the Graphite
//! mockup draws them (#144, #147): the toggle, the segmented choice, the mono
//! readout, the slider look, a glyph button, a help text with More…, and the
//! frame of a preview with its expand button.

use super::key_catcher::catch_keys;
use super::{HelpTopic, PagesViewMsg, PreviewKind};
use crate::preferences::PrefMsg;
use crate::styles::{self, Radii, ti, tokens};
use iced::alignment::Horizontal;
use iced::keyboard::{Key, key::Named};
use iced::widget::{Space, column, container, row, slider, svg};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _,
};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{
    Label, SegmentedStyle, ToggleSwitchStyle, button, rich_text, segmented, span, text,
    toggle_switch, tooltip,
};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Side of a glyph button: twice the body size (24 px).
pub(in crate::preferences) const ICON_BUTTON: f32 = TS.body * 2.0;
/// Side of a glyph inside it (16 px).
const GLYPH: f32 = TS.body * 4.0 / 3.0;
/// Narrowest readout beside a slider (72 px).
const READOUT_MIN_W: f32 = TS.body * 6.0;
/// Average advance of a monospace glyph, in ems.
const MONO_GLYPH_EM: f32 = 0.62;
/// Height of a preview drawing, as the mockup draws it.
const PREVIEW_H: f32 = 200.0;
/// Height of a preview that fills the page: the content area of the
/// 980 × 680 dialog under its heading row.
const EXPANDED_PREVIEW_H: f32 = 480.0;
/// Slider rail thickness and knob radius, as the mockup draws them.
const SLIDER_RAIL: f32 = 4.0;
const SLIDER_KNOB_RADIUS: f32 = 7.0;
const SLIDER_KNOB_RING: f32 = 1.5;

/// The mockup's 16 px stroke glyphs; the style recolours them.
pub(in crate::preferences) const GLYPH_EXPAND: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9"/></svg>"##;
pub(in crate::preferences) const GLYPH_SHRINK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"><path d="M13.5 6.5h-4v-4M2.5 9.5h4v4M9.5 6.5L14 2M6.5 9.5L2 14"/></svg>"##;
pub(in crate::preferences) const GLYPH_MINUS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round"><path d="M3 8h10"/></svg>"##;
pub(in crate::preferences) const GLYPH_PLUS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round"><path d="M8 3v10M3 8h10"/></svg>"##;
/// The pencil on an editable pitch label.
pub(in crate::preferences) const GLYPH_PENCIL: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"><path d="M3 13l1-3.5 6.5-6.5 2.5 2.5-6.5 6.5z"/></svg>"##;

/// A page message for the view state.
pub(in crate::preferences) fn view_msg(message: PagesViewMsg) -> PrefMsg {
    PrefMsg::PagesView(message)
}

/// Colours of a preview drawing, from the theme's canvas and ink tokens.
#[derive(Debug, Clone, Copy)]
pub(in crate::preferences) struct PreviewColours {
    pub paper: Color,
    pub ink: Color,
    pub field: Color,
    pub border: Color,
    pub ink2: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub line_strong: Color,
    pub info: Color,
    pub part_fill: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub raised: Color,
    pub err: Color,
    pub err_soft: Color,
}

impl PreviewColours {
    pub(in crate::preferences) fn new(t: &ThemeTokens) -> Self {
        let r = tokens::resolve(t);
        Self {
            paper: ti(t.paper),
            ink: ti(t.text),
            field: r.field,
            border: ti(t.border),
            ink2: ti(t.text_secondary),
            ink3: r.ink3,
            ink4: r.ink4,
            line_strong: r.line_strong,
            info: r.info,
            part_fill: r.warn_soft,
            accent: ti(t.accent),
            accent_text: r.accent_text,
            raised: r.raised,
            err: ti(t.error),
            err_soft: r.err_soft,
        }
    }
}

/// An on/off switch; a press sends `on_change` with the flipped value.
pub(in crate::preferences) fn toggle<'a>(
    on: bool,
    t: &ThemeTokens,
    on_change: fn(bool) -> PrefMsg,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let style = ToggleSwitchStyle::new(ti(t.accent), r.line_strong, tokens::on_accent(t));
    toggle_switch(on, style, Some(on_change(!on)))
}

/// A segmented choice filling the control column.
pub(in crate::preferences) fn segmented_choice<'a, V>(
    options: Vec<(V, Localized)>,
    selected: V,
    t: &ThemeTokens,
    on_select: impl Fn(V) -> PrefMsg + 'a,
) -> Element<'a, PrefMsg>
where
    V: Copy + PartialEq + 'a,
{
    let r = tokens::resolve(t);
    let style = SegmentedStyle::new(r.sunken, r.raised, r.shadow, r.ink3, ti(t.text), TS.body)
        .fill_width(true);
    segmented(
        options
            .into_iter()
            .map(|(value, label)| (value, Label::from(label))),
        selected,
        style,
        on_select,
    )
}

/// A right-aligned mono value beside a slider ("100%").
pub(in crate::preferences) fn readout<'a>(
    value: Localized,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let chars = value.to_string().chars().count() as f32;
    let width = (chars * MONO_GLYPH_EM * TS.label).max(READOUT_MIN_W);
    text(value)
        .size(TS.label)
        .font(Font::MONOSPACE)
        .color(ti(t.text))
        .width(Length::Fixed(width))
        .align_x(Horizontal::Right)
        .boxed()
}

/// The Graphite slider: accent fill on a strong rail, a raised knob ringed
/// in the accent.
pub(in crate::preferences) fn slider_style(
    t: &ThemeTokens,
) -> impl Fn(&Theme, slider::Status) -> slider::Style + 'static {
    let r = tokens::resolve(t);
    let (accent, rail, knob) = (ti(t.accent), r.line_strong, r.raised);
    move |_, _| slider::Style {
        rail: slider::Rail {
            backgrounds: (Background::Color(accent), Background::Color(rail)),
            width: SLIDER_RAIL,
            border: Border {
                radius: (SLIDER_RAIL / 2.0).into(),
                ..Border::default()
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle {
                radius: SLIDER_KNOB_RADIUS,
            },
            background: Background::Color(knob),
            border_width: SLIDER_KNOB_RING,
            border_color: accent,
        },
    }
}

/// A stroke glyph at `side` px in `ink`.
pub(in crate::preferences) fn glyph<'a>(
    bytes: &'static [u8],
    side: f32,
    ink: Color,
) -> Element<'a, PrefMsg> {
    svg(svg::Handle::from_memory(bytes))
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// A 24 px glyph button with a tooltip. `radius` is the corner of its
/// hover fill.
pub(in crate::preferences) fn glyph_button<'a>(
    bytes: &'static [u8],
    tip: Localized,
    on_press: PrefMsg,
    radius: f32,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let ink = ti(t.text_secondary);
    let hover = tokens::overlay(t, tokens::Overlay::Hover);
    let face = button(
        container(glyph(bytes, GLYPH, ink))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding(0)
    .width(Length::Fixed(ICON_BUTTON))
    .height(Length::Fixed(ICON_BUTTON))
    .on_press(on_press)
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });
    let tip = container(text(tip).size(TS.caption))
        .padding([SP.xs, SP.m])
        .style(styles::context_menu(t));
    tooltip(face, tip, tooltip::Position::Top)
        .gap(SP.xs)
        .boxed()
}

/// A help text cut to `summary` with a More… link that shows `full` in
/// place, and a Less link that cuts it again. Nothing is dropped from the
/// catalog: the whole text stays one press away.
pub(in crate::preferences) fn help_with_more<'a>(
    topic: HelpTopic,
    [summary, full]: [MessageId; 2],
    expanded: bool,
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (body, link) = if expanded {
        (full, ids::PREFERENCES_PAGES_LESS)
    } else {
        (summary, ids::PREFERENCES_PAGES_MORE)
    };
    let spans = vec![
        span::<PrefMsg>(t!(i18n, body)).color(r.ink3),
        span::<PrefMsg>(t!(i18n, link))
            .color(r.accent_text)
            .link(view_msg(PagesViewMsg::HelpExpandToggled(topic))),
    ];
    rich_text::<PrefMsg, PrefMsg, Theme>(spans)
        .on_link_click(|message| message)
        .size(TS.caption)
        .width(Length::Fill)
        .boxed()
}

/// A preview under a setting: its label, a caption, the expand button and
/// the framed drawing, with an optional caption under the frame.
pub(in crate::preferences) struct PreviewBlock<'a> {
    pub kind: PreviewKind,
    pub expanded: bool,
    pub title: Localized,
    pub caption: Localized,
    /// The drawing; it fills the frame.
    pub drawing: Element<'a, PrefMsg>,
    pub footer: Option<Localized>,
}

/// The height a preview's drawing gets.
pub(in crate::preferences) fn preview_height(expanded: bool) -> f32 {
    if expanded {
        EXPANDED_PREVIEW_H
    } else {
        PREVIEW_H
    }
}

/// Lay out `block`. Expanded, it is a raised card that fills the page in
/// place of the settings, and Esc shrinks it again.
pub(in crate::preferences) fn preview_block<'a>(
    block: PreviewBlock<'a>,
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let toggle_msg = view_msg(PagesViewMsg::PreviewExpandToggled(block.kind));
    let (icon, tip) = if block.expanded {
        (GLYPH_SHRINK, ids::PREFERENCES_PAGES_CLOSE_PREVIEW)
    } else {
        (GLYPH_EXPAND, ids::PREFERENCES_PAGES_EXPAND_PREVIEW)
    };
    let header = row![
        text(block.title)
            .size(TS.section)
            .style(styles::section_label(t)),
        text(block.caption).size(TS.caption).color(r.ink4),
        Space::new().width(Length::Fill),
        glyph_button(
            icon,
            t!(i18n, tip),
            toggle_msg.clone(),
            Radii::GRAPHITE.small,
            t
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let (paper, line) = (ti(t.paper), ti(t.border));
    let frame = container(block.drawing)
        .width(Length::Fill)
        .height(Length::Fixed(preview_height(block.expanded)))
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(paper)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let mut body = column![header, frame].spacing(SP.s).width(Length::Fill);
    if let Some(footer) = block.footer {
        body = body.push(text(footer).size(TS.caption).color(r.ink4).boxed());
    }
    if !block.expanded {
        let soft = r.line_soft;
        let rule = container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(soft)),
                ..container::Style::default()
            });
        return column![
            container(body).padding(Padding {
                top: SP.m,
                right: 0.0,
                bottom: SP.m,
                left: 0.0,
            }),
            rule,
        ]
        .width(Length::Fill)
        .boxed();
    }
    let (panel, edge) = (ti(t.panel_bg), ti(t.border));
    let card = container(body)
        .padding([SP.l, SP.xl])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: Radii::GRAPHITE.dialog.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed();
    catch_keys(card, move |key, _| {
        matches!(key, Key::Named(Named::Escape)).then(|| toggle_msg.clone())
    })
}
