//! The net-class table (#2417): one row per class of the active project's
//! table, in match order, under a header. A click on a row selects the class
//! the fields under the table edit. Drawn by the Net Classes dock panel and
//! the Properties section alike (`net_class_panel::editor`).

use iced::widget::{Column, Space, container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::net_class::{NetClass, format_hex_color};
use signex_widgets::{Label, button, text};

use super::{NetClassMsg, PanelMsg, form_look};
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Fill portions of the Name, Colour, Width and Members columns.
const COLUMNS: [u16; 4] = [3, 3, 2, 4];

/// Side of the colour cell's swatch.
const SWATCH: f32 = 10.0;

pub(super) fn view<'a>(
    classes: &[NetClass],
    selected: usize,
    header_ink: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let mut table = Column::new().spacing(SP.xs).width(Length::Fill);
    table = table.push(header(header_ink, i18n));
    for (idx, class) in classes.iter().enumerate() {
        table = table.push(class_row(class, idx, idx == selected, i18n));
    }
    table.boxed()
}

fn header<'a>(ink: Color, i18n: &I18n) -> Element<'a, PanelMsg> {
    let titles: [Localized; 4] = [
        t!(i18n, ids::PANELS_NET_CLASSES_COLUMN_NAME),
        t!(i18n, ids::PANELS_NET_CLASSES_COLUMN_COLOR),
        t!(i18n, ids::PANELS_NET_CLASSES_COLUMN_WIDTH),
        t!(i18n, ids::PANELS_NET_CLASSES_COLUMN_MEMBERS),
    ];
    let mut line: iced::widget::Row<Element<'a, PanelMsg>> = row![].spacing(SP.s);
    for (title, portion) in titles.into_iter().zip(COLUMNS) {
        line = line.push(clipped(face(title.into()).color(ink), portion));
    }
    // The row buttons' padding, so the titles sit over their cells.
    container(line).padding([0.0, SP.m]).boxed()
}

fn class_row<'a>(
    class: &NetClass,
    idx: usize,
    selected: bool,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let colour: Element<'a, PanelMsg> = match class.style.color {
        Some(c) => row![
            container(Space::new())
                .width(SWATCH)
                .height(SWATCH)
                .style(swatch_style(ti(c))),
            clipped(face(UserText::new(format_hex_color(c)).into()), 1),
        ]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .width(Length::FillPortion(COLUMNS[1]))
        .boxed(),
        None => clipped(
            face(t!(i18n, ids::PANELS_NET_CLASSES_THEME_COLOR).into()),
            COLUMNS[1],
        ),
    };
    let width: Label<'static> = match class.style.wire_width_mm {
        Some(mm) => t!(i18n, ids::PANELS_NET_CLASSES_WIDTH_CELL, "width" => mm.to_string()).into(),
        None => t!(i18n, ids::PANELS_NET_CLASSES_WIRE_WIDTH).into(),
    };
    let members: Vec<&str> = class
        .nets
        .iter()
        .chain(&class.patterns)
        .map(String::as_str)
        .collect();
    let members: Label<'static> = if members.is_empty() {
        t!(i18n, ids::PANELS_NET_CLASSES_NO_MEMBERS).into()
    } else {
        UserText::new(members.join(", ")).into()
    };
    let cells = row![
        clipped(face(UserText::new(class.name.clone()).into()), COLUMNS[0]),
        colour,
        clipped(face(width), COLUMNS[2]),
        clipped(face(members), COLUMNS[3]),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    button(cells)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .on_press(PanelMsg::NetClass(NetClassMsg::Select(idx)))
        .style(form_look::option_tile(selected))
        .boxed()
}

/// A cell's text: one line at body size.
fn face<'a>(label: Label<'a>) -> iced::widget::Text<'a, Theme> {
    text(label)
        .size(TS.body)
        .wrapping(iced::widget::text::Wrapping::None)
}

/// One table cell: its column's share of the width, a long value cut at the
/// cell's edge rather than drawn over the next column.
fn clipped<'a>(face: iced::widget::Text<'a, Theme>, portion: u16) -> Element<'a, PanelMsg> {
    container(face)
        .width(Length::FillPortion(portion))
        .clip(true)
        .boxed()
}

fn swatch_style(color: Color) -> impl Fn(&Theme) -> container::Style + use<> {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: 2.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}
