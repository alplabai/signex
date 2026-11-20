//! Pieces the ERC and DRC panels share (#85, #87, #88): the full-width run
//! button, notice banners, the hint card, links, collapsible group headers
//! and the two-line finding rows whose actions show on hover or selection.

use iced::widget::mouse_area;
use iced::widget::text::{Ellipsis, Wrapping};
use signex_i18n::Localized;
use signex_widgets::{Label, rich_text, span};

use super::*;
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, PillTone, Radii, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Alpha of a banner's edge, drawn in its tone's ink.
const BANNER_EDGE_ALPHA: f32 = 0.35;
/// Alpha of a key cap shown on a primary button, in the button's ink.
const KEY_CAP_ALPHA: f32 = 0.7;

/// The tone of a notice banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Ok,
    Info,
    Warn,
}

impl Tone {
    fn pill(self) -> PillTone {
        match self {
            Tone::Ok => PillTone::Ok,
            Tone::Info => PillTone::Info,
            Tone::Warn => PillTone::Warn,
        }
    }
}

fn faded(c: Color, alpha: f32) -> Color {
    Color {
        a: c.a * alpha,
        ..c
    }
}

fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A full-width primary button: the label, and the key that runs it when
/// the keymap binds one. `on_press: None` draws it disabled.
pub(super) fn run_button<'a>(
    label: Localized,
    key: &str,
    on_press: Option<PanelMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let ink = resolve(tokens).primary_ink;
    let mut face = row![text(label).size(TS.label).color(ink)]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    if !key.is_empty() {
        face = face.push(
            text(signex_i18n::UserText::new(key.to_string()))
                .size(TS.caption)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(faded(ink, KEY_CAP_ALPHA))
                .boxed(),
        );
    }
    button(container(face).center_x(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(ChromeMetrics::DEFAULT.button))
        .padding([0.0, SP.m])
        .on_press_maybe(on_press)
        .style(crate::styles::primary_button(tokens))
        .boxed()
}

/// A notice: a soft fill and edge in `tone`, the body, and an action on
/// the right when there is one.
pub(super) fn banner<'a>(
    tone: Tone,
    body: Element<'a, PanelMsg>,
    action: Option<Element<'a, PanelMsg>>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let (fill, ink) = crate::styles::pill_colors(tokens, tone.pill());
    let edge = faded(ink, BANNER_EDGE_ALPHA);
    let mut line = row![container(body).width(Length::Fill)]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    if let Some(action) = action {
        line = line.push(action);
    }
    container(line)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: edge,
                width: crate::styles::HAIRLINE,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The words of a banner, with an inline link after them when given. The
/// line wraps inside the dock.
pub(super) fn notice<'a>(
    words: Localized,
    link: Option<Element<'a, PanelMsg>>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let mut line = row![
        text(words)
            .size(TS.caption)
            .color(theme_ext::text_primary(tokens))
    ]
    .spacing(SP.xs);
    if let Some(link) = link {
        line = line.push(link);
    }
    line.wrap().vertical_spacing(SP.xs).boxed()
}

/// An inline link: accent ink, underlined, sending `on_press`.
pub(super) fn link<'a>(
    label: impl Into<Label<'a>>,
    on_press: PanelMsg,
    color: Color,
) -> Element<'a, PanelMsg> {
    let spans = vec![
        span(label)
            .underline(true)
            .color(color)
            .font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
            })
            .link(on_press),
    ];
    rich_text(spans)
        .on_link_click(|msg: PanelMsg| msg)
        .size(TS.caption)
        .boxed()
}

/// The accent ink of a link.
pub(super) fn link_ink(tokens: &ThemeTokens) -> Color {
    resolve(tokens).accent_text
}

/// A hint card for an idle line: sunken fill, strong hairline.
pub(super) fn hint_card<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (fill, edge, ink) = (r.sunken, r.line_strong, r.ink3);
    container(text(words).size(TS.caption).color(ink))
        .padding(SP.m)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: edge,
                width: crate::styles::HAIRLINE,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A small status pill ("waived", "default", "always").
pub(super) fn status_pill<'a>(
    words: impl Into<Label<'a>>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    container(text(words).size(TS.caption).wrapping(Wrapping::None))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

/// The one-letter severity square at the start of a group or row. A row on
/// the selection fill passes `selected`, so the letter stays readable there.
pub(super) fn severity_badge<'a>(
    letter: impl Into<Label<'a>>,
    tone: PillTone,
    selected: bool,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let side = TS.caption * crate::styles::LINE_HEIGHT;
    let ink = crate::styles::pill_colors(tokens, tone).1;
    let ink = super::erc_panel::erc_severity_text(tokens, selected, ink);
    container(
        text(letter)
            .size(TS.caption)
            .color(ink)
            .wrapping(Wrapping::None),
    )
    .center(Length::Fixed(side))
    .style(crate::styles::pill(tokens, tone))
    .boxed()
}

/// A collapsible group heading: chevron, badge, title, count, and an
/// optional trailing link outside the toggle.
pub(super) struct GroupHeader<'a> {
    pub open: bool,
    pub badge: Option<Element<'a, PanelMsg>>,
    pub title: Label<'a>,
    /// Ink of the title (the Waived group is muted).
    pub title_ink: Color,
    /// Shown after the title in mono; `None` when the title carries it.
    pub count: Option<usize>,
    pub trailing: Option<Element<'a, PanelMsg>>,
    pub on_toggle: PanelMsg,
}

pub(super) fn group_header<'a>(
    header: GroupHeader<'a>,
    i18n: &signex_i18n::I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let ink3 = resolve(tokens).ink3;
    let mut face = row![signex_widgets::chevron::chevron(header.open, TS.body, ink3)]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    if let Some(badge) = header.badge {
        face = face.push(badge);
    }
    face = face.push(
        text(header.title)
            .size(TS.body)
            .font(semibold())
            .color(header.title_ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .boxed(),
    );
    if let Some(count) = header.count {
        face = face.push(
            text(signex_i18n::UserText::new(
                signex_widgets::chips::count_text(i18n, count as u32),
            ))
            .size(TS.caption)
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT,
            ))
            .color(ink3)
            .boxed(),
        );
    }
    let toggle = button(face)
        .width(Length::Fill)
        .padding([SP.xs, SP.m])
        .on_press(header.on_toggle)
        .style(crate::styles::menu_item(tokens));
    let mut line = row![toggle].align_y(iced::Alignment::Center);
    if let Some(trailing) = header.trailing {
        line = line.push(container(trailing).padding([0.0, SP.m]).boxed());
    }
    line.width(Length::Fill).boxed()
}

/// A two-line finding row: badge, the message (wrapped in full when the row
/// is open), the second line and any extra lines; then its actions, shown
/// only while the row is hovered or selected.
pub(super) struct FindingRow<'a> {
    pub badge: Element<'a, PanelMsg>,
    pub message: Label<'a>,
    pub place: Element<'a, PanelMsg>,
    pub extra: Vec<Element<'a, PanelMsg>>,
    pub actions: Vec<Element<'a, PanelMsg>>,
    pub selected: bool,
    pub hovered: bool,
    /// What a press on the row sends; `None` for a row that only lists.
    pub on_press: Option<PanelMsg>,
    pub on_enter: Option<PanelMsg>,
    pub on_exit: Option<PanelMsg>,
}

pub(super) fn finding_row<'a>(item: FindingRow<'a>, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let open = item.selected || item.hovered;
    let (ink, _) = super::erc_panel::erc_row_text(tokens, item.selected);
    let mut message = text(item.message).size(TS.body).color(ink);
    if !open {
        message = message.wrapping(Wrapping::None).ellipsis(Ellipsis::End);
    }
    let mut lines: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.xs / 2.0)
        .width(Length::Fill)
        .push(message.width(Length::Fill).boxed())
        .push(item.place);
    for extra in item.extra {
        lines = lines.push(extra);
    }
    if open && !item.actions.is_empty() {
        let mut acts = row![].spacing(SP.xs);
        for action in item.actions {
            acts = acts.push(action);
        }
        lines = lines.push(
            container(acts.wrap().vertical_spacing(SP.xs).boxed())
                .padding([SP.xs, 0.0])
                .boxed(),
        );
    }
    let fill = item.selected.then(|| theme_ext::selection_color(tokens));
    let menu = crate::styles::menu_item(tokens);
    let body = button(
        row![item.badge, lines]
            .spacing(SP.s)
            .align_y(iced::Alignment::Start),
    )
    .width(Length::Fill)
    .padding([SP.xs, SP.m])
    .on_press_maybe(item.on_press)
    .style(move |theme: &Theme, status: button::Status| {
        let base = menu(theme, status);
        button::Style {
            background: fill.map(Background::Color).or(base.background),
            ..base
        }
    });
    let mut area = mouse_area(body);
    if let Some(enter) = item.on_enter {
        area = area.on_enter(enter);
    }
    if let Some(exit) = item.on_exit {
        area = area.on_exit(exit);
    }
    area.boxed()
}

/// A small secondary row action ("Show", "Waive net").
pub(super) fn row_action<'a>(
    label: impl Into<Label<'a>>,
    on_press: PanelMsg,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    crate::styles::dialog_button(
        label,
        crate::styles::DialogButtonKind::Secondary,
        Some(on_press),
        tokens,
    )
}
