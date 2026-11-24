//! Builders the passive-calculator pages share: the field label, the
//! composite value field (input, SI-prefix picker and unit inside one
//! field), the metric strip, the boxes, buttons and pill, and the one text
//! helper for data. Sizes come from the theme scale, colours from the
//! caller's [`CalculatorPalette`].

use iced::advanced::text::IntoFragment;
use iced::alignment::Vertical;
use iced::font::Weight;
use iced::widget::{Column, Row, Space, container, pick_list, text_input};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, t};

use super::look::CalculatorPalette;
use crate::dropdown_style::pick_list_menu_style;
use crate::theme_ext::{ChromeMetrics, HAIRLINE, Radii, Spacing, TypeScale};
use crate::{Label, button, raw, text};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;
pub(super) const RADII: Radii = Radii::GRAPHITE;
pub(super) const CM: ChromeMetrics = ChromeMetrics::DEFAULT;

/// Below this body width the input grids, metric strips and part cards
/// stack (the mockup's narrow layout), in em of the body size.
pub(super) const NARROW_WIDTH: f32 = TS.body * 44.0;

/// Gap between the parts inside a composite field.
const FIELD_GAP: f32 = SP.s + SP.xs;
/// Horizontal padding of a metric cell.
const CELL_PAD_X: f32 = SP.m + SP.xs;
/// Alpha of the text-selection wash in a field.
const SELECTION_ALPHA: f32 = 0.3;
/// How much the primary button darkens under the pointer.
const PRIMARY_HOVER_SHADE: f32 = 0.92;

/// Text that is data, not language: a value, a code, a unit symbol, a
/// tolerance or an E-series name.
pub(super) fn data<'a>(value: impl IntoFragment<'a>) -> Label<'a> {
    // raw: values, codes, unit symbols and series names are data, not language.
    raw(value)
}

/// A catalog message upper-cased for the UI language (`.sec .lbl`).
pub(super) fn upper(i18n: &I18n, id: MessageId) -> Label<'static> {
    // raw: catalog text, upper-cased by the locale's casemap.
    raw(i18n.upper_for_display(&t!(i18n, id).into_string()))
}

/// `font` at medium weight.
pub(super) fn medium(font: Font) -> Font {
    Font {
        weight: Weight::Medium,
        ..font
    }
}

/// A caption-size label over its control (`.flabel`).
pub(super) fn labeled<'a, M: 'a>(
    label: Localized,
    palette: CalculatorPalette,
    control: Element<'a, M>,
) -> Element<'a, M> {
    Column::new()
        .push(text(label).size(TS.caption).color(palette.ink3).boxed())
        .push(control)
        .spacing(SP.s)
        .width(Length::Fill)
        .boxed()
}

/// One grid line of labelled controls, each as wide as its share and
/// bottom-aligned; stacked one per line when `is_narrow` (`.cgrid`).
pub(super) fn input_grid<'a, M: 'a>(
    cells: Vec<(u16, Element<'a, M>)>,
    is_narrow: bool,
) -> Element<'a, M> {
    if is_narrow {
        let mut stack: Column<Element<'a, M>> = Column::new().spacing(SP.l).width(Length::Fill);
        for (_, cell) in cells {
            stack = stack.push(cell);
        }
        return stack.boxed();
    }
    let mut line: Row<Element<'a, M>> = Row::new()
        .spacing(SP.l)
        .align_y(Alignment::End)
        .width(Length::Fill);
    for (portion, cell) in cells {
        let width = if portion == 0 {
            Length::Shrink
        } else {
            Length::FillPortion(portion)
        };
        line = line.push(container(cell).width(width).boxed());
    }
    line.boxed()
}

/// One field holding `parts` side by side: a value control, the SI-prefix
/// picker and the unit suffix (`.field`).
pub(super) fn composite_field<'a, M: 'a>(
    parts: Vec<Element<'a, M>>,
    palette: CalculatorPalette,
) -> Element<'a, M> {
    let mut line: Row<Element<'a, M>> = Row::new().spacing(FIELD_GAP).align_y(Alignment::Center);
    for part in parts {
        line = line.push(part);
    }
    container(line)
        .width(Length::Fill)
        .center_y(CM.field)
        .padding(Padding::from([0.0, SP.m]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(palette.field)),
            border: Border {
                width: HAIRLINE,
                radius: RADII.control.into(),
                color: palette.border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A text input drawn without its own box, for use inside a
/// [`composite_field`].
pub(super) fn bare_input_style(
    palette: CalculatorPalette,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    move |_: &Theme, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: palette.ink4,
        value: palette.ink,
        selection: Color {
            a: SELECTION_ALPHA,
            ..palette.accent
        },
    }
}

/// A picker drawn inside a [`composite_field`]: on `fill` (the sunken
/// SI-prefix chip) or on the field itself when `fill` is `None`.
pub(super) fn bare_pick_style(
    palette: CalculatorPalette,
    fill: Option<Color>,
) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style + 'static {
    move |_: &Theme, _| pick_list::Style {
        text_color: palette.ink,
        placeholder_color: palette.ink4,
        handle_color: palette.ink4,
        background: Background::Color(fill.unwrap_or(Color::TRANSPARENT)),
        border: Border {
            radius: RADII.small.into(),
            ..Border::default()
        },
    }
}

/// The SI-prefix picker inside a field: mono text on the sunken chip.
pub(super) fn prefix_picker<'a, T, M>(
    selected: T,
    options: &'static [T],
    on_select: impl Fn(T) -> M + 'a,
    palette: CalculatorPalette,
    mono: Font,
) -> Element<'a, M>
where
    T: ToString + PartialEq + Clone + 'a,
    M: Clone + 'a,
{
    pick_list(Some(selected), options, T::to_string)
        .on_select(on_select)
        .font(mono)
        .text_size(TS.body)
        .padding(Padding::from([0.0, SP.xs]))
        .style(bare_pick_style(palette, Some(palette.sunken)))
        .menu_style(pick_list_menu_style)
        .boxed()
}

/// The muted unit suffix at the end of a field (`Ω`, `F`, `H`).
pub(super) fn unit_suffix<'a, M: 'a>(
    unit: &'static str,
    palette: CalculatorPalette,
) -> Element<'a, M> {
    text(data(unit)).size(TS.body).color(palette.ink3).boxed()
}

/// One cell of a metric strip: the caption key over the value, and an
/// optional delta line under it (`.metric`).
pub(super) fn metric_cell<'a, M: 'a>(
    key: Localized,
    value: Element<'a, M>,
    delta: Option<Element<'a, M>>,
    palette: CalculatorPalette,
) -> Element<'a, M> {
    let mut cell = Column::new()
        .push(text(key).size(TS.caption).color(palette.ink4).boxed())
        .push(value)
        .spacing(SP.xs);
    if let Some(delta) = delta {
        cell = cell.push(delta);
    }
    cell.width(Length::Fill).boxed()
}

/// A mono metric value at the label size.
pub(super) fn metric_value<'a, M: 'a>(
    value: impl IntoFragment<'a>,
    palette: CalculatorPalette,
    mono: Font,
) -> Element<'a, M> {
    text(data(value))
        .font(mono)
        .size(TS.label)
        .color(palette.ink)
        .boxed()
}

/// A bordered strip of metric cells, `per_row` to a line, each cell as wide
/// as its share; hairlines between the cells and between the lines
/// (`.metrics`).
pub(super) fn metric_strip<'a, M: 'a>(
    cells: Vec<(u16, Element<'a, M>)>,
    per_row: usize,
    palette: CalculatorPalette,
) -> Element<'a, M> {
    let per_row = per_row.max(1);
    let mut lines: Column<Element<'a, M>> = Column::new().width(Length::Fill);
    let mut cells = cells.into_iter().peekable();
    let mut is_first_line = true;
    while cells.peek().is_some() {
        if !is_first_line {
            lines = lines.push(hairline(Length::Fill, HAIRLINE, palette.line_soft));
        }
        is_first_line = false;
        let mut line: Row<Element<'a, M>> = Row::new().width(Length::Fill);
        for (index, (portion, cell)) in cells.by_ref().take(per_row).enumerate() {
            if index > 0 {
                line = line.push(hairline(HAIRLINE, Length::Fill, palette.line_soft));
            }
            line = line.push(
                container(cell)
                    .width(Length::FillPortion(portion))
                    .padding(Padding::from([SP.m, CELL_PAD_X]))
                    .boxed(),
            );
        }
        lines = lines.push(line.boxed());
    }
    container(lines)
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                radius: RADII.button.into(),
                color: palette.border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A filled rule `width` x `height`.
fn hairline<'a, M: 'a>(
    width: impl Into<Length>,
    height: impl Into<Length>,
    color: Color,
) -> Element<'a, M> {
    container(Space::new())
        .width(width)
        .height(height)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// A panel-coloured box with the card radius around `content`.
pub(super) fn panel_box<'a, M: 'a>(
    content: Element<'a, M>,
    palette: CalculatorPalette,
) -> Element<'a, M> {
    container(content)
        .padding(SP.l)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(palette.panel)),
            border: Border {
                width: HAIRLINE,
                radius: RADII.card.into(),
                color: palette.border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The primary action button, as tall as a field (`.btn.primary`).
pub(super) fn primary_button<'a, M: Clone + 'a>(
    label: Localized,
    on_press: M,
    palette: CalculatorPalette,
) -> Element<'a, M> {
    button(
        text(label)
            .size(TS.body)
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .height(CM.field)
    .padding(Padding::from([0.0, CELL_PAD_X]))
    .on_press(on_press)
    .style(move |_: &Theme, status: button::Status| {
        let fill = match status {
            button::Status::Hovered | button::Status::Pressed => shade(palette.primary),
            _ => palette.primary,
        };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: palette.primary_ink,
            border: Border {
                radius: RADII.button.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .boxed()
}

/// A small borderless button in the accent ink (`.btn.ghost.sm`).
pub(super) fn ghost_button<'a, M: Clone + 'a>(
    label: Localized,
    on_press: M,
    palette: CalculatorPalette,
) -> Element<'a, M> {
    button(text(label).size(TS.caption))
        .padding(Padding::from([SP.xs, SP.m]))
        .on_press(on_press)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(palette.hover)),
            text_color: palette.accent_text,
            border: Border {
                radius: RADII.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .boxed()
}

/// A status pill: `ink` text on its `fill` (`.pill`).
pub(super) fn pill<'a, M: 'a>(label: Localized, ink: Color, fill: Color) -> Element<'a, M> {
    container(text(label).size(TS.caption).color(ink))
        .padding(Padding::from([0.0, SP.s + SP.xs]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: RADII.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn shade(color: Color) -> Color {
    Color {
        r: color.r * PRIMARY_HOVER_SHADE,
        g: color.g * PRIMARY_HOVER_SHADE,
        b: color.b * PRIMARY_HOVER_SHADE,
        ..color
    }
}
