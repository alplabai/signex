//! The form chrome every role sub-form shares (mockup row 126): the header with its
//! caption line, the collapsible section, label/field rows, the segmented
//! control, checkbox grids and enable sections. Sizes come from the type and
//! spacing scales, colours from the theme tokens.

use iced::alignment::Horizontal;
use iced::widget::{Column, Row, container, row, stack};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, Localized};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::enable_section::{EnableSection, dependents_veil};
use signex_widgets::inert::Inert;
use signex_widgets::property_illustration::Illustration;
use signex_widgets::{CheckboxLabel as _, Label, button, checkbox, text, text_input};

use super::super::super::{
    CollapsedSections, FootprintEditorPanelContext, PROPERTY_LABEL_COLUMN, PROPERTY_ROW_GAP,
    PanelMsg, form_look,
};
use crate::panels::enable_section::enable_section_style;
use crate::panels::widgets::section_title;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, Spacing, TypeScale, ti};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// The illustration under a sub-form header, linked to the panel's hover.
pub(super) fn illustration<'a>(
    drawing: Illustration,
    fp: &'a FootprintEditorPanelContext,
    tokens: &'a ThemeTokens,
) -> Element<'a, PanelMsg> {
    super::super::illustration::view_illustration(drawing, &fp.illustration_hover, tokens)
}

/// The sub-form header: the object name (with an optional count pill) and
/// the context caption line under it.
pub(super) fn header<'a>(
    title: Localized,
    pill: Option<Localized>,
    caption: Element<'a, PanelMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let mut title_row = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s + SP.xs)
        .align_y(iced::Alignment::Center)
        .push(
            text(title)
                .size(TS.label)
                .color(ti(tokens.text))
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::DEFAULT
                })
                .boxed(),
        );
    if let Some(pill) = pill {
        title_row = title_row.push(count_pill(pill, tokens));
    }
    container(
        Column::new()
            .spacing(SP.xs)
            .push(title_row.boxed())
            .push(caption),
    )
    .padding(Padding {
        top: SP.m,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// A small neutral pill, such as "8 instances".
fn count_pill<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let look = resolve(tokens);
    let (fill, ink) = (look.sunken, look.ink3);
    container(text(label).size(TS.caption).color(ink))
        .padding([0.0, SP.s + SP.xs])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The caption line of a header, in the faint ink.
pub(super) fn caption<'a>(
    line: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    text(line)
        .size(TS.caption)
        .color(resolve(tokens).ink4)
        .boxed()
}

/// A note line inside a form, in the faint ink.
pub(super) fn note<'a>(line: impl Into<Label<'a>>, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    container(caption(line, tokens))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// Whether section `key` shows its body. A section closed by default stores
/// its key in `collapsed` while it is open, so one toggle message serves
/// both kinds.
pub(super) fn is_open(key: &str, collapsed: &CollapsedSections, open_by_default: bool) -> bool {
    collapsed.contains(key) != open_by_default
}

/// A collapsible section header: chevron and the uppercase title.
pub(super) fn section_header<'a>(
    key: &'static str,
    title: Localized,
    is_open: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let ink = resolve(tokens).ink4;
    let label = row![
        signex_widgets::chevron::chevron(is_open, TS.section, ink),
        section_title(title, tokens, i18n),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    button(
        container(label)
            .padding(Padding {
                top: SP.m,
                right: SP.m,
                bottom: SP.s,
                left: SP.m,
            })
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::ToggleSection(key.to_owned()))
    .style(|_: &Theme, _| button::Style::default())
    .boxed()
}

/// A label and its control on one row, the label in the fixed column.
pub(super) fn field_row<'a>(
    label: impl Into<Label<'a>>,
    control: Element<'a, PanelMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(label)
                .size(TS.caption)
                .color(resolve(tokens).ink3)
                .width(PROPERTY_LABEL_COLUMN),
            container(control).width(Length::Fill),
        ]
        .spacing(PROPERTY_ROW_GAP)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// Two controls sharing the field column of a row ("nx / ny").
pub(super) fn pair<'a>(
    first: Element<'a, PanelMsg>,
    second: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        container(first).width(Length::FillPortion(1)),
        container(second).width(Length::FillPortion(1)),
    ]
    .spacing(SP.s + SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// A text field in the form look.
pub(super) fn input<'a>(
    placeholder: impl Into<Label<'a>>,
    value: String,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    text_input(placeholder, value)
        .size(TS.caption)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::text_input_style)
        .on_input(on_input)
        .boxed()
}

/// A text field whose placeholder is an example value ("1", "360deg",
/// "i != 0"), not language.
pub(super) fn example_input<'a>(
    example: &'static str,
    value: String,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    // raw: an example expression, the same in every language.
    input(signex_widgets::raw(example), value, on_input)
}

/// A dropdown over `options`, each shown by its `Display` text.
pub(super) fn pick<'a, T>(
    options: &'a [T],
    selected: T,
    on_select: impl Fn(T) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg>
where
    T: Clone + PartialEq + std::fmt::Display + 'static,
{
    iced::widget::pick_list(Some(selected), options, |option: &T| option.to_string())
        .graphite()
        .on_select(on_select)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// A read-only value in the form's value box.
pub(super) fn value_box<'a>(value: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    container(text(value).size(TS.caption))
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let (fill, line, radius) = form_look::value_box(theme);
            container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    width: 1.0,
                    color: line,
                    radius: radius.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        })
        .boxed()
}

/// One option of a segmented control; `msg: None` draws it disabled.
pub(super) struct Segment {
    pub label: Localized,
    pub active: bool,
    pub msg: Option<PanelMsg>,
}

/// A segmented control on the sunken frame, one equal cell per option.
pub(super) fn segmented<'a>(options: Vec<Segment>) -> Element<'a, PanelMsg> {
    let mut cells: Row<Element<'a, PanelMsg>> = Row::new().width(Length::Fill);
    for Segment { label, active, msg } in options {
        let enabled = msg.is_some();
        let cell = button(
            text(label)
                .size(TS.caption)
                .align_x(Horizontal::Center)
                .width(Length::Fill),
        )
        .padding([SP.xs, SP.xs])
        .width(Length::Fill)
        .on_press_maybe(msg)
        .style(move |theme: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered);
            let mut style = form_look::segment_cell(theme, active, hovered && !active);
            if !enabled && !active {
                style.text_color = form_look::FormLook::of(theme).ink4;
            }
            style
        });
        cells = cells.push(cell.boxed());
    }
    container(cells)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed()
}

/// A labelled form checkbox.
pub(super) fn check<'a>(
    label: impl Into<Label<'a>>,
    is_on: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let ink = ti(tokens.text);
    checkbox(is_on)
        .labelled(label)
        .on_toggle(on_toggle)
        .size(TS.body)
        .text_size(TS.caption)
        .spacing(SP.s + SP.xs)
        .style(move |theme: &Theme, status| {
            let mut style = form_look::checkbox_style(theme, status);
            style.text_color = Some(ink);
            style
        })
        .boxed()
}

/// Checkboxes two to a row.
pub(super) fn check_grid<'a>(cells: Vec<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    let mut grid = Column::<Element<'a, PanelMsg>>::new().spacing(SP.xs);
    let mut cells = cells.into_iter();
    while let Some(first) = cells.next() {
        let second = cells
            .next()
            .unwrap_or_else(|| iced::widget::Space::new().width(Length::Fill).boxed());
        grid = grid.push(pair(first, second));
    }
    container(grid)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// An enable section: the checkbox header above `body`, which greys and
/// takes no input while the switch is off. `state` is the short word the
/// header shows at its right ("off", "tented").
pub(super) fn enable_section<'a>(
    label: Localized,
    is_on: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
    state: Option<Localized>,
    body: Element<'a, PanelMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let section = EnableSection::new(label, is_on)
        .on_toggle(on_toggle)
        .view(enable_section_style(tokens), body);
    let Some(state) = state else {
        return section;
    };
    let word = container(caption(state, tokens))
        .padding(Padding {
            top: SP.m,
            right: SP.m,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .align_x(Horizontal::Right);
    stack![section, word].boxed()
}

/// `body` as given while `is_enabled`; otherwise greyed and deaf to input,
/// like the fields of a switched-off enable section.
pub(super) fn gated<'a>(
    body: Element<'a, PanelMsg>,
    is_enabled: bool,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    match dependents_veil(ti(tokens.panel_bg), is_enabled) {
        Some(veil) => Inert::new(body, veil).boxed(),
        None => body,
    }
}

/// A small button in the danger ink, right-aligned at the end of a form.
pub(super) fn danger_action<'a>(
    label: Localized,
    msg: PanelMsg,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    container(
        button(text(label).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press(msg)
            .style(crate::styles::danger_button(tokens)),
    )
    .padding([SP.m, SP.m])
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .boxed()
}

/// A small ghost button.
pub(super) fn ghost_action<'a>(label: Localized, msg: PanelMsg) -> Element<'a, PanelMsg> {
    button(text(label).size(TS.caption))
        .padding([SP.xs, SP.m])
        .on_press(msg)
        .style(form_look::ghost_button)
        .boxed()
}

/// The sub-form body: header, illustration and sections, one column.
pub(super) fn form<'a>(parts: Vec<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    Column::with_children(parts)
        .spacing(0)
        .width(Length::Fill)
        .boxed()
}
