//! The colour-band representations of a value: one line of bands per
//! convention, each with its spoken description under it.

use iced::widget::{Column, Row, Space, container};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};

use super::color_code::ComponentColorCode;
use super::domain::ComponentKind;
use super::look::CalculatorPalette;
use super::widgets::{RADII, SP, TS, data};
use crate::text;
use crate::theme_ext::HAIRLINE;

/// Size of one band, in em of the body size.
const BAND_W_EM: f32 = 1.25;
const BAND_H_EM: f32 = 2.0;

pub fn color_code_representations<'a, Message: 'a>(
    i18n: &I18n,
    representations: Vec<ComponentColorCode>,
    kind: ComponentKind,
    palette: CalculatorPalette,
) -> Element<'a, Message> {
    let heading = if representations.len() > 1 {
        ids::PASSIVE_CALCULATOR_COLOR_CODE_ALTERNATIVES
    } else {
        ids::PASSIVE_CALCULATOR_COLOR_CODE
    };
    let mut color_codes: Column<Element<'a, Message>> = Column::new()
        .push(
            text(t!(i18n, heading))
                .size(TS.caption)
                .color(palette.ink4)
                .boxed(),
        )
        .spacing(SP.s);
    if representations.is_empty() {
        let unavailable = match kind {
            ComponentKind::Resistor => ids::PASSIVE_CALCULATOR_NO_BANDS_RESISTOR,
            ComponentKind::Capacitor => ids::PASSIVE_CALCULATOR_NO_BANDS_CAPACITOR,
            ComponentKind::Inductor => ids::PASSIVE_CALCULATOR_NO_BANDS_INDUCTOR,
        };
        color_codes = color_codes.push(
            text(t!(i18n, unavailable))
                .size(TS.caption)
                .color(palette.ink3)
                .boxed(),
        );
    } else {
        for representation in representations {
            color_codes = color_codes.push(color_code_line(representation, palette));
        }
    }
    color_codes.width(Length::Fill).boxed()
}

fn color_code_line<'a, Message: 'a>(
    code: ComponentColorCode,
    palette: CalculatorPalette,
) -> Element<'a, Message> {
    let description = code.accessible_label();
    let mut bands: Row<Element<'a, Message>> = Row::new().spacing(SP.xs).align_y(Alignment::Center);
    for band in code.bands {
        bands = bands.push(
            container(Space::new())
                .width(TS.body * BAND_W_EM)
                .height(TS.body * BAND_H_EM)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(band.color())),
                    border: Border {
                        width: HAIRLINE,
                        radius: (RADII.small / 3.0).into(),
                        color: palette.ink3,
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed(),
        );
    }
    Column::new()
        .push(bands.boxed())
        // The band names and convention come from `color_code.rs`, which
        // words them in English (not migrated yet).
        .push(
            text(data(description))
                .size(TS.caption)
                .color(palette.ink)
                .boxed(),
        )
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
}
