//! Board Information (#2353): sizes and counts read from the board, laid
//! out as small titled cards of label-over-value tiles (#2386). The
//! component counts select those components; the DRC count opens the
//! design rule check results, and Run DRC runs one when none has run.

use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, container, row};
use iced::{Alignment, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{LocalNumber, Localized, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::{format_length_mm, mm_per};
use signex_widgets::{Label, text};

use super::chrome::{Form, Shown};
use super::{ComponentSide, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, RADIUS, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let rows: Vec<Shown<'a>> = vec![
        board_size(form, board),
        components(form, board),
        layers(form, board),
        nets(form, board),
        primitives(form, board),
        drc(form),
        reports(form),
    ];
    form.section("pcb_board_info", ids::PCB_BOARD_SECTION_INFO, rows)
}

/// One figure of a card: a small label over its value.
struct Tile<'a> {
    label: Localized,
    value: Element<'a, PanelMsg>,
}

fn tile<'a>(form: &Form<'a>, label: MessageId, value: Element<'a, PanelMsg>) -> Tile<'a> {
    Tile {
        label: form.t(label),
        value,
    }
}

/// A read-only value in the mono face, on one line.
fn value<'a>(form: &Form<'a>, shown: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    text(shown)
        .size(TS.body)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .color(form.ink())
        .wrapping(Wrapping::None)
        .boxed()
}

/// What a card shows under its tiles, and the labels the search finds in it.
type Extra<'a> = Option<(Element<'a, PanelMsg>, Vec<Localized>)>;

/// A small card titled `caption` holding `lines` of tiles side by side, and
/// `extra` under them; the search finds it by its caption and every label.
fn card<'a>(
    form: &Form<'a>,
    caption: MessageId,
    lines: Vec<Vec<Tile<'a>>>,
    extra: Extra<'a>,
) -> Shown<'a> {
    let caption = form.t(caption);
    let mut keys: Vec<Localized> = vec![caption.clone()];
    let muted = form.muted();
    let mut body = Column::new().spacing(SP.s).width(Length::Fill).push(
        text(caption)
            .size(TS.caption)
            .color(resolve(form.tokens()).ink4)
            .wrapping(Wrapping::None),
    );
    for line in lines {
        let mut tiles: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m).width(Length::Fill);
        for Tile { label, value } in line {
            keys.push(label.clone());
            let figure = Column::new()
                .spacing(SP.xs)
                .push(
                    text(label)
                        .size(TS.caption)
                        .color(muted)
                        .wrapping(Wrapping::None),
                )
                .push(value);
            tiles = tiles.push(container(figure).width(Length::FillPortion(1)));
        }
        body = body.push(tiles);
    }
    if let Some((extra, labels)) = extra {
        body = body.push(extra);
        keys.extend(labels);
    }
    let edge = resolve(form.tokens()).line_soft;
    let framed = container(body)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: edge,
                radius: RADIUS.into(),
            },
            ..container::Style::default()
        });
    let element = container(framed)
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed();
    let refs: Vec<&Localized> = keys.iter().collect();
    form.row(&refs, element)
}

fn count(form: &Form<'_>, n: usize) -> Localized {
    t!(form.i18n, ids::PCB_BOARD_COUNT, "count" => n)
}

fn length(form: &Form<'_>, mm: f64) -> Localized {
    measure(form, format_length_mm(mm, shown_unit(form)))
}

fn area(form: &Form<'_>, mm2: f64) -> Localized {
    let unit = shown_unit(form);
    let per = mm_per(unit);
    measure(form, format!("{:.2} {unit}²", mm2 / (per * per)))
}

/// A length or area from Signex's own formatter, unit included.
fn measure(form: &Form<'_>, text: String) -> Localized {
    t!(form.i18n, ids::PCB_BOARD_MEASURE, "value" => text)
}

/// The panel's unit (`super::shown_unit`).
pub(super) fn shown_unit(form: &Form<'_>) -> Unit {
    super::shown_unit(form.ctx.unit)
}

/// Width and Height, Area and Thickness, then the parts' area and density;
/// one muted "No board outline" line in place of the outline's figures.
fn board_size<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    let stats = &board.stats;
    let thickness = tile(
        form,
        ids::PCB_BOARD_INFO_THICKNESS,
        value(form, length(form, board.thickness_mm)),
    );
    let mut lines = Vec::new();
    let mut extra = None;
    match board.size_mm {
        Some((w, h)) => {
            // Two to a line, so a figure keeps its width in a narrow dock.
            lines.push(vec![
                tile(
                    form,
                    ids::PCB_BOARD_INFO_WIDTH,
                    value(form, length(form, w)),
                ),
                tile(
                    form,
                    ids::PCB_BOARD_INFO_HEIGHT,
                    value(form, length(form, h)),
                ),
            ]);
            let mut second = Vec::new();
            if let Some(a) = stats.area_mm2 {
                second.push(tile(
                    form,
                    ids::PCB_BOARD_INFO_AREA,
                    value(form, area(form, a)),
                ));
            }
            second.push(thickness);
            lines.push(second);
        }
        None => {
            let none = text(form.t(ids::PCB_BOARD_INFO_NO_OUTLINE))
                .size(TS.body)
                .color(form.muted())
                .wrapping(Wrapping::None);
            extra = Some((none.boxed(), Vec::new()));
            lines.push(vec![thickness]);
        }
    }
    let mut parts = vec![tile(
        form,
        ids::PCB_BOARD_INFO_COMPONENTS_AREA,
        value(form, area(form, stats.components_area_mm2)),
    )];
    if let Some(density) = stats.density() {
        let percent = t!(
            form.i18n,
            ids::PCB_BOARD_PERCENT,
            "value" => LocalNumber((density * 10_000.0).round() / 100.0)
        );
        parts.push(tile(
            form,
            ids::PCB_BOARD_INFO_DENSITY,
            value(form, percent),
        ));
    }
    lines.push(parts);
    card(form, ids::PCB_BOARD_INFO_SIZE, lines, extra)
}

/// Total, Top and Bottom on one row; a count above zero selects them.
fn components<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    let stats = &board.stats;
    let tiles: Vec<Tile<'a>> = [
        (
            ids::PCB_BOARD_INFO_TOTAL,
            stats.components(),
            ComponentSide::All,
        ),
        (
            ids::PCB_BOARD_INFO_TOP,
            stats.components_top,
            ComponentSide::Top,
        ),
        (
            ids::PCB_BOARD_INFO_BOTTOM,
            stats.components_bottom,
            ComponentSide::Bottom,
        ),
    ]
    .into_iter()
    .map(|(label, n, side)| {
        let shown = if n > 0 {
            form.link(
                count(form, n),
                PanelMsg::PcbBoard(PcbBoardMsg::ComponentsSelected(side)),
                form.t(ids::PCB_BOARD_INFO_SELECT_TIP),
            )
        } else {
            value(form, count(form, n))
        };
        tile(form, label, shown)
    })
    .collect();
    card(form, ids::PCB_BOARD_INFO_COMPONENTS, vec![tiles], None)
}

fn layers<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    let tiles = vec![
        tile(
            form,
            ids::PCB_BOARD_INFO_TOTAL,
            value(form, count(form, board.layers)),
        ),
        tile(
            form,
            ids::PCB_BOARD_INFO_SIGNAL,
            value(form, count(form, board.copper_layers)),
        ),
    ];
    card(form, ids::PCB_BOARD_INFO_LAYERS, vec![tiles], None)
}

fn nets<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    // The PCB panel's net count, so the two always agree.
    let total = form.ctx.pcb_nets.as_ref().map_or(0, |s| s.nets.len());
    let unrouted = match board.unrouted_nets {
        Some(n) => value(form, count(form, n)),
        None => form.tip(
            value(form, form.t(ids::PCB_BOARD_UNSET)),
            form.t(ids::PCB_BOARD_INFO_NOT_COUNTED),
        ),
    };
    let tiles = vec![
        tile(
            form,
            ids::PCB_BOARD_INFO_TOTAL,
            value(form, count(form, total)),
        ),
        tile(form, ids::PCB_BOARD_INFO_UNROUTED, unrouted),
        tile(
            form,
            ids::PCB_BOARD_INFO_HIDDEN,
            value(form, count(form, board.hidden_nets)),
        ),
    ];
    card(form, ids::PCB_BOARD_INFO_NETS, vec![tiles], None)
}

/// The kinds the board holds, as name | count pairs two to a row. Kinds the
/// board cannot hold yet are left out.
fn primitives<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    let stats = &board.stats;
    let pairs = [
        (ids::PCB_BOARD_INFO_PADS, stats.pads),
        (ids::PCB_BOARD_INFO_TRACKS, stats.tracks),
        (ids::PCB_BOARD_INFO_VIAS, stats.vias),
        (ids::PCB_BOARD_INFO_POLYGONS, stats.polygons),
        (ids::PCB_BOARD_INFO_FILLS, stats.fills),
        (ids::PCB_BOARD_INFO_REGIONS, stats.regions),
        (ids::PCB_BOARD_INFO_KEEPOUTS, stats.keepouts),
        (ids::PCB_BOARD_INFO_STRINGS, stats.strings),
        (ids::PCB_BOARD_INFO_HOLES, stats.pad_holes + stats.via_holes),
    ];
    let mut keys = Vec::new();
    let mut grid = Column::new().spacing(SP.xs).width(Length::Fill);
    for chunk in pairs.chunks(2) {
        let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m).width(Length::Fill);
        for &(label, n) in chunk {
            let label = form.t(label);
            keys.push(label.clone());
            let pair = row![
                container(
                    text(label)
                        .size(TS.body)
                        .color(form.muted())
                        .wrapping(Wrapping::None)
                )
                .width(Length::Fill),
                value(form, count(form, n)),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center);
            line = line.push(container(pair).width(Length::FillPortion(1)));
        }
        if chunk.len() == 1 {
            line = line.push(iced::widget::Space::new().width(Length::FillPortion(1)));
        }
        grid = grid.push(line);
    }
    card(
        form,
        ids::PCB_BOARD_INFO_PRIMITIVES,
        Vec::new(),
        Some((grid.boxed(), keys)),
    )
}

/// DRC Violations: the count, which opens the results, or Not run with Run
/// DRC beside it.
fn drc<'a>(form: &Form<'a>) -> Shown<'a> {
    let label = form.t(ids::PCB_BOARD_INFO_DRC);
    let shown = match drc_violations(form) {
        Some(n) => form.link(
            count(form, n),
            PanelMsg::PcbBoard(PcbBoardMsg::DrcResultsOpened),
            form.t(ids::PCB_BOARD_INFO_DRC_TIP),
        ),
        None => row![
            value(form, form.t(ids::PCB_BOARD_INFO_DRC_NOT_RUN)),
            form.action(
                form.t(ids::PCB_BOARD_HEALTH_RUN_DRC),
                Some(PanelMsg::RunDrc),
                None,
            ),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed(),
    };
    form.row(&[&label], form.field(&label, shown))
}

/// Reports, at the end of the section; no board report exists yet.
fn reports<'a>(form: &Form<'a>) -> Shown<'a> {
    let reports = form.t(ids::PCB_BOARD_REPORTS);
    let button = form.action(
        reports.clone(),
        None,
        Some(form.t(ids::PCB_BOARD_REPORTS_UNAVAILABLE)),
    );
    let button = container(button).padding([SP.s, SP.l]).boxed();
    form.row(&[&reports], button)
}

/// The violations of the board's last design rule check; `None` before one.
pub(super) fn drc_violations(form: &Form<'_>) -> Option<usize> {
    let drc = form.ctx.pcb_drc.as_ref().filter(|d| d.ran)?;
    Some(drc.rows.len() + drc.hidden)
}
