//! The Graphite chrome of the footprint Properties body: the object header,
//! section headers with a trailing pill or button, small heads, captions,
//! pills, banners, full-width segments, checkbox grids and key-value rows.

use iced::widget::{Column, Row, Space, container, row};
use iced::{Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, Localized};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, SegmentedStyle, button, checkbox, raw, segmented, text};

use crate::panels::{CollapsedSections, PanelMsg, form_look};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, Spacing, TypeScale, pill_colors, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the key column of a key-value row, in captions.
const KV_KEY_EM: f32 = 8.0;

fn ui_font(weight: iced::font::Weight) -> Font {
    Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

fn mono_font() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// `label` upper-cased for the UI language, as section and key labels show.
fn upper<'a>(label: &Localized, i18n: &I18n) -> Label<'a> {
    // raw: the catalog label, upper-cased for the locale.
    raw(i18n.upper_for_display(&label.to_string()))
}

/// The object header: what the panel shows (`title`) over a status line.
pub(in crate::panels::footprint_editor_properties) fn object_header<'a>(
    title: impl Into<Label<'a>>,
    status: Option<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let mut lines = Column::new().spacing(0).push(
        text(title)
            .size(TS.label)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text))
            .boxed(),
    );
    if let Some(status) = status {
        lines = lines.push(
            text(status)
                .size(TS.caption)
                .color(resolve(tokens).ink4)
                .boxed(),
        );
    }
    container(lines)
        .padding(Padding {
            top: SP.m,
            right: SP.m,
            bottom: SP.xs,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// Whether the section `key` is open.
pub(in crate::panels::footprint_editor_properties) fn is_open(
    key: &str,
    collapsed: &CollapsedSections,
) -> bool {
    !collapsed.contains(key)
}

/// A collapsible section header: chevron and the upper-cased `title`, with
/// an optional `trailing` pill or button at the right.
pub(in crate::panels::footprint_editor_properties) fn section<'a>(
    key: &str,
    title: &Localized,
    trailing: Option<Element<'a, PanelMsg>>,
    collapsed: &CollapsedSections,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let ink = resolve(tokens).ink3;
    let open = is_open(key, collapsed);
    let label = row![
        signex_widgets::chevron::chevron(open, TS.section, resolve(tokens).ink4),
        text(upper(title, i18n))
            .size(TS.section)
            .font(ui_font(iced::font::Weight::Medium))
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    let toggle = button(label)
        .padding([SP.s, SP.xs])
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key.to_string()))
        .style(|_: &Theme, _| button::Style::default());
    let mut line = Row::new()
        .push(toggle.boxed())
        .align_y(iced::Alignment::Center)
        .spacing(SP.s);
    if let Some(trailing) = trailing {
        line = line.push(trailing);
    }
    container(line)
        .padding(Padding {
            top: SP.xs,
            right: SP.m,
            bottom: 0.0,
            left: SP.xs,
        })
        .width(Length::Fill)
        .boxed()
}

/// A small upper-case head inside a section ("Snapping").
pub(in crate::panels::footprint_editor_properties) fn mini_head<'a>(
    label: &Localized,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    container(
        text(upper(label, i18n))
            .size(TS.caption)
            .color(resolve(tokens).ink4),
    )
    .padding(Padding {
        top: SP.s + SP.xs,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// A muted caption line.
pub(in crate::panels::footprint_editor_properties) fn caption<'a>(
    label: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    container(
        text(label)
            .size(TS.caption)
            .color(resolve(tokens).ink4)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// A status pill in `tone`.
pub(in crate::panels::footprint_editor_properties) fn pill<'a>(
    label: impl Into<Label<'a>>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let (_, ink) = pill_colors(tokens, tone);
    container(text(label).size(TS.caption).color(ink))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

/// A full-width banner in `tone` (the DOF state of a sketch entity).
pub(in crate::panels::footprint_editor_properties) fn banner<'a>(
    label: impl Into<Label<'a>>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let (fill, ink) = pill_colors(tokens, tone);
    container(
        container(
            text(label)
                .size(TS.body)
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::Word),
        )
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// The look of a segmented control on `tokens`, stretched to the width.
fn segment_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = resolve(tokens);
    r.segmented(TS.body).fill_width(true)
}

/// A segmented control stretched to its column (units in a row).
pub(in crate::panels::footprint_editor_properties) fn segment<'a, V>(
    options: impl IntoIterator<Item = (V, Label<'a>)>,
    selected: V,
    tokens: &ThemeTokens,
    on_select: impl Fn(V) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg>
where
    V: Copy + PartialEq + 'a,
{
    segmented(options, selected, segment_style(tokens), on_select)
}

/// A segmented control across the panel (the snapping scope).
pub(in crate::panels::footprint_editor_properties) fn full_width_segment<'a, V>(
    options: impl IntoIterator<Item = (V, Label<'a>)>,
    selected: V,
    tokens: &ThemeTokens,
    on_select: impl Fn(V) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg>
where
    V: Copy + PartialEq + 'a,
{
    container(segment(options, selected, tokens, on_select))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// A checkbox with its label; a click on either sends `msg`.
pub(in crate::panels::footprint_editor_properties) fn check_cell<'a>(
    label: Localized,
    is_on: bool,
    msg: PanelMsg,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let toggle = msg.clone();
    let content = row![
        checkbox(is_on)
            .on_toggle(move |_| toggle.clone())
            .size(TS.body)
            .spacing(0)
            .style(form_look::checkbox_style),
        text(label)
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    button(content)
        .padding([SP.xs, 0.0])
        .width(Length::Fill)
        .on_press(msg)
        .style(|_: &Theme, _| button::Style::default())
        .boxed()
}

/// `cells` two to a row, as the mockup's checkbox grid.
pub(in crate::panels::footprint_editor_properties) fn check_grid<'a>(
    cells: Vec<Element<'a, PanelMsg>>,
) -> Element<'a, PanelMsg> {
    let mut grid = Column::new().spacing(0).width(Length::Fill);
    let mut cells = cells.into_iter();
    while let Some(first) = cells.next() {
        let second = cells
            .next()
            .unwrap_or_else(|| Space::new().width(Length::Fill).boxed());
        grid = grid.push(
            Row::new()
                .spacing(SP.l)
                .push(container(first).width(Length::FillPortion(1)).boxed())
                .push(container(second).width(Length::FillPortion(1)).boxed())
                .boxed(),
        );
    }
    container(grid)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// A compact read-only key-value row: the key upper-cased and muted, the
/// value in the monospace face.
pub(in crate::panels::footprint_editor_properties) fn kv_row<'a>(
    key: &Localized,
    value: String,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(upper(key, i18n))
                .size(TS.caption)
                .color(resolve(tokens).ink4)
                .width(Length::Fixed(TS.caption * KV_KEY_EM)),
            // raw: the value is a measured number, not language.
            text(raw(value))
                .size(TS.body)
                .font(mono_font())
                .color(ti(tokens.text))
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(SP.l)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// A small ghost button ("+ Add", "Delete"); `None` draws it disabled.
pub(in crate::panels::footprint_editor_properties) fn ghost<'a>(
    label: Localized,
    msg: Option<PanelMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    button(text(label).size(TS.caption))
        .padding([SP.xs, SP.s])
        .on_press_maybe(msg)
        .style(crate::styles::ghost_button(tokens))
        .boxed()
}

/// `content` greyed and deaf to input while `is_enabled` is off, as a
/// switched-off enable section greys its fields.
pub(in crate::panels::footprint_editor_properties) fn gated<'a>(
    content: Element<'a, PanelMsg>,
    is_enabled: bool,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    match signex_widgets::enable_section::dependents_veil(ti(tokens.panel_bg), is_enabled) {
        Some(veil) => signex_widgets::inert::Inert::new(content, veil).boxed(),
        None => content,
    }
}

/// A hairline under a table header.
pub(in crate::panels::footprint_editor_properties) fn rule<'a>(
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let line: Color = resolve(tokens).line_soft;
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .width(Length::Fill)
        .boxed()
}
