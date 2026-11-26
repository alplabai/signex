//! The layer list (mockup row 193): one row per file in the drawing order,
//! the top of the list drawing on top. Each row has a grip, an eye, a
//! colour swatch, the layer name, the file name and a badge saying where
//! the name came from (X2, the job file, or the file name). A swatch opens
//! a row of colours under its layer. Files that are not fabrication data
//! close the list as "Not read".

use iced::widget::{Column, Row, Space, column, container, mouse_area, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_gerber::{CopperSide, DrillKind, FileFunction, NameSource, PackageLayer, Side};
use signex_i18n::{Localized, UserText, ids, t};
use signex_widgets::{button, checkbox, text};

use super::{Look, SP, TS};
use crate::app::gerber_viewer::GerberViewerMsg;
use crate::app::gerber_viewer::state::{OpenPackage, SWATCHES};
use crate::panels::graphite::hint_tooltip;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, ti};

/// A swatch's side, in body-text ems.
const SWATCH_EMS: f32 = 0.9;

pub(super) fn view<'a>(look: Look<'a>, open: &'a OpenPackage) -> Element<'a, GerberViewerMsg> {
    let mut list: Column<Element<'a, GerberViewerMsg>> = Column::new().spacing(SP.xs);
    list = list.push(
        text(look.t(ids::GERBER_VIEWER_LAYERS))
            .size(TS.section)
            .color(resolve(look.tokens).ink3)
            .boxed(),
    );
    for (place, layer) in open.order.iter().copied().enumerate() {
        list = list.push(layer_row(look, open, place, layer));
        if open.colour_open == Some(layer) {
            list = list.push(swatches(look, layer));
        }
    }
    for problem in &open.package.not_read {
        list = list.push(not_read_row(look, &problem.at.file));
    }
    // The grip's release is caught tab-wide (`view::tab`), so a release
    // over the board or the toolbar ends the drag too.
    let list = scrollable(list.padding(SP.s)).height(Length::Fill);
    container(list)
        .height(Length::Fill)
        .style(crate::styles::panel_region(look.tokens))
        .boxed()
}

fn layer_row<'a>(
    look: Look<'a>,
    open: &'a OpenPackage,
    place: usize,
    layer: usize,
) -> Element<'a, GerberViewerMsg> {
    let entry = &open.package.layers[layer];
    let shown = open.looks.get(layer).is_some_and(|l| l.visible);
    let grip = mouse_area(
        text(look.t(ids::GERBER_VIEWER_GRIP))
            .size(TS.body)
            .color(resolve(look.tokens).ink3),
    )
    .on_press(GerberViewerMsg::GripPressed(place));
    let grip = hint_tooltip(
        grip.boxed(),
        look.t(ids::GERBER_VIEWER_GRIP_TIP),
        look.tokens,
        below(),
    );
    let eye = checkbox(shown).on_toggle(move |_| GerberViewerMsg::LayerVisibilityToggled(layer));
    let eye = hint_tooltip(
        eye.boxed(),
        look.t(ids::GERBER_VIEWER_SHOW_TIP),
        look.tokens,
        below(),
    );
    let names = column![
        text(layer_name(look, entry))
            .size(TS.dense)
            .color(ti(look.tokens.text))
            .boxed(),
        text(UserText::new(entry.image.file.clone()))
            .size(TS.caption)
            .color(resolve(look.tokens).ink3)
            .boxed(),
    ];
    let name = button(names)
        .padding(0)
        .width(Length::Fill)
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(GerberViewerMsg::LayerPicked(layer));
    let line = row![
        grip,
        eye,
        swatch(look, open, layer),
        name.boxed(),
        badge(look, entry),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let current = open.current == Some(layer) || open.grip.is_some_and(|g| g.over == place);
    let fill = current.then(|| resolve(look.tokens).accent_soft);
    let row = container(line)
        .padding([SP.xs, SP.xs])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            ..Default::default()
        });
    mouse_area(row)
        .on_enter(GerberViewerMsg::RowEntered(place))
        .boxed()
}

fn below() -> iced::widget::tooltip::Position {
    iced::widget::tooltip::Position::Bottom
}

/// The layer's colour; a press opens its colour row.
fn swatch<'a>(look: Look<'a>, open: &OpenPackage, layer: usize) -> Element<'a, GerberViewerMsg> {
    let [r, g, b] = open.looks[layer].colour;
    let face = chip(
        look,
        Color::from_rgb8(r, g, b),
        GerberViewerMsg::SwatchPressed(layer),
    );
    hint_tooltip(
        face,
        look.t(ids::GERBER_VIEWER_COLOUR_TIP),
        look.tokens,
        below(),
    )
}

/// The colours a swatch offers, under its layer.
fn swatches<'a>(look: Look<'a>, layer: usize) -> Element<'a, GerberViewerMsg> {
    let mut line: Row<Element<'a, GerberViewerMsg>> = Row::new().spacing(SP.xs);
    for colour in SWATCHES {
        let [r, g, b] = colour;
        line = line.push(chip(
            look,
            Color::from_rgb8(r, g, b),
            GerberViewerMsg::ColourPicked { layer, colour },
        ));
    }
    container(line).padding([SP.xs, SP.l]).boxed()
}

/// A square of `colour` as a button that sends `on_press`.
fn chip<'a>(
    look: Look<'a>,
    colour: Color,
    on_press: GerberViewerMsg,
) -> Element<'a, GerberViewerMsg> {
    let side = TS.body * SWATCH_EMS;
    let line = ti(look.tokens.border);
    button(Space::new().width(side).height(side))
        .padding(0)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                color: line,
                width: crate::styles::HAIRLINE,
                radius: (side / 4.0).into(),
                ..Border::default()
            },
            ..Default::default()
        })
        .on_press(on_press)
        .boxed()
}

/// Where the name came from: X2 (hover for the file function), the job
/// file, or the file name.
fn badge<'a>(look: Look<'a>, entry: &PackageLayer) -> Element<'a, GerberViewerMsg> {
    let (label, tip) = match entry.name_source {
        NameSource::X2 => {
            let function = entry
                .function
                .as_ref()
                .map(FileFunction::as_x2)
                .unwrap_or_default();
            (
                ids::GERBER_VIEWER_BADGE_X2,
                t!(look.i18n, ids::GERBER_VIEWER_BADGE_X2_TIP, "function" => function),
            )
        }
        NameSource::JobFile => (
            ids::GERBER_VIEWER_BADGE_JOB,
            look.t(ids::GERBER_VIEWER_BADGE_JOB_TIP),
        ),
        NameSource::FileName => (
            ids::GERBER_VIEWER_BADGE_NAME,
            look.t(ids::GERBER_VIEWER_BADGE_NAME_TIP),
        ),
    };
    let pill = container(text(look.t(label)).size(TS.caption))
        .padding([0.0, SP.xs])
        .style(crate::styles::pill(look.tokens, PillTone::Neutral));
    hint_tooltip(pill.boxed(), tip, look.tokens, below())
}

/// A file that is not fabrication data: listed, greyed, never ignored.
fn not_read_row<'a>(look: Look<'a>, file: &str) -> Element<'a, GerberViewerMsg> {
    let ink = resolve(look.tokens).ink4;
    row![
        text(UserText::new(file.to_string()))
            .size(TS.dense)
            .color(ink)
            .width(Length::Fill)
            .boxed(),
        container(text(look.t(ids::GERBER_VIEWER_NOT_READ)).size(TS.caption))
            .padding([0.0, SP.xs])
            .style(crate::styles::pill(look.tokens, PillTone::Warn))
            .boxed(),
    ]
    .spacing(SP.xs)
    .padding([SP.xs, SP.xs])
    .align_y(Alignment::Center)
    .boxed()
}

/// A layer's name: from its function, else the label made from its file
/// name. Copper reads L1..Ln by position, as in the PCB editor.
pub(super) fn layer_name(look: Look<'_>, entry: &PackageLayer) -> signex_widgets::Label<'static> {
    match &entry.function {
        Some(function) => function_name(look, function),
        None => UserText::new(
            entry
                .file_label
                .clone()
                .unwrap_or_else(|| entry.image.file.clone()),
        )
        .into(),
    }
}

fn function_name(look: Look<'_>, function: &FileFunction) -> signex_widgets::Label<'static> {
    let sided = |top, bottom, side: &Side| match side {
        Side::Top => look.t(top),
        Side::Bottom => look.t(bottom),
    };
    let name: Localized = match function {
        FileFunction::Copper { number, side } => copper_name(look, *number, *side),
        FileFunction::Legend(side) => sided(
            ids::GERBER_VIEWER_LAYER_SILK_TOP,
            ids::GERBER_VIEWER_LAYER_SILK_BOTTOM,
            side,
        ),
        FileFunction::Soldermask(side) => sided(
            ids::GERBER_VIEWER_LAYER_MASK_TOP,
            ids::GERBER_VIEWER_LAYER_MASK_BOTTOM,
            side,
        ),
        FileFunction::Paste(side) => sided(
            ids::GERBER_VIEWER_LAYER_PASTE_TOP,
            ids::GERBER_VIEWER_LAYER_PASTE_BOTTOM,
            side,
        ),
        FileFunction::Profile { .. } => look.t(ids::GERBER_VIEWER_LAYER_OUTLINE),
        FileFunction::Drill { kind, from, to, .. } => drill_name(look, *kind, *from, *to),
        FileFunction::Other(value) => return UserText::new(value.clone()).into(),
    };
    name.into()
}

fn copper_name(look: Look<'_>, number: u32, side: CopperSide) -> Localized {
    let layer = t!(look.i18n, ids::GERBER_VIEWER_LAYER_COPPER, "number" => number.to_string());
    match side {
        CopperSide::Top => {
            t!(look.i18n, ids::GERBER_VIEWER_LAYER_COPPER_TOP, "layer" => layer.into_string())
        }
        CopperSide::Bottom => {
            t!(look.i18n, ids::GERBER_VIEWER_LAYER_COPPER_BOTTOM, "layer" => layer.into_string())
        }
        CopperSide::Inner => layer,
    }
}

fn drill_name(look: Look<'_>, kind: DrillKind, from: u32, to: u32) -> Localized {
    let copper = |n: u32| {
        t!(look.i18n, ids::GERBER_VIEWER_LAYER_COPPER, "number" => n.to_string()).into_string()
    };
    match kind {
        DrillKind::Pth => look.t(ids::GERBER_VIEWER_LAYER_DRILL_PLATED),
        DrillKind::Npth => look.t(ids::GERBER_VIEWER_LAYER_DRILL_NON_PLATED),
        DrillKind::Blind | DrillKind::Buried => t!(
            look.i18n,
            ids::GERBER_VIEWER_LAYER_DRILL_SPAN,
            "from" => copper(from),
            "to" => copper(to)
        ),
    }
}
