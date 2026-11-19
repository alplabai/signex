//! Board Information (#2353, #2523 m93): sizes and counts read from the
//! board as a grid of stat tiles, three to a row (two in a narrow dock).
//! Width and Height come from the board shape and light up with their
//! callouts; the component counts select those components; Unrouted and
//! DRC Violations take the warning tint above zero, and the DRC count opens
//! the results. The layer and primitive counts sit in a closed fold under
//! the tiles. Reports sits in the section header; Run DRC is on the Health
//! Check tab.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Row, Space, container, responsive, row};
use iced::{Alignment, Background, Border, Element, Length, Size, Theme, Widget as _};
use signex_i18n::{LocalNumber, Localized, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::{format_length_mm, mm_per};
use signex_widgets::property_illustration::FieldId;
use signex_widgets::{Label, button, text};

use super::chrome::{Form, Head, Shown, is_open, mono, toggle_key};
use super::drawing::{BOARD_HEIGHT, BOARD_WIDTH};
use super::{ComponentSide, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};
use crate::styles::TypeScale;
use crate::styles::tokens::resolve;

const TS: TypeScale = TypeScale::DEFAULT;

/// The fold of layer and primitive counts, closed by default.
const MORE_KEY: &str = "pcb_board_info_more";
/// A tile narrower than this many captions loses its value: below it the
/// grid takes two tiles a row instead of three.
const TILE_MIN_EM: f32 = 7.5;

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let ctx = form.ctx;
    let labels = tiles(form, board)
        .into_iter()
        .map(|tile| tile.label)
        .collect::<Vec<_>>();
    let refs: Vec<&Localized> = labels.iter().collect();
    // The column count follows the width, so the grid is laid out per size.
    let grid = responsive(move |size: Size| {
        let form = Form::new(ctx, &board.view.search, &board.view.illustration_hover);
        tile_grid(&form, tiles(&form, board), size.width)
    })
    .height(Length::Shrink);
    let mut rows = vec![form.row(&refs, grid.boxed())];
    rows.push(more(form, board));
    let reports = form.action(
        form.t(ids::PCB_BOARD_REPORTS),
        Some(PanelMsg::PcbBoard(PcbBoardMsg::ReportsOpened)),
        None,
    );
    form.section_with(
        "pcb_board_info",
        ids::PCB_BOARD_SECTION_INFO,
        Head::open().summary(reports),
        rows,
    )
}

/// What a tile does when pressed, and how it is tinted.
enum Look {
    Plain,
    /// A count that does something: accent value, the whole tile pressable.
    Link(PanelMsg, Localized),
}

/// One stat tile: a small label over its value.
struct Tile<'a> {
    label: Localized,
    value: Label<'a>,
    look: Look,
    /// Warning tint (a count above zero that needs work).
    warn: bool,
    /// The illustration field it shows, lit with its callout.
    field: Option<FieldId>,
    tip: Option<Localized>,
}

fn tile<'a>(form: &Form<'a>, label: MessageId, value: impl Into<Label<'a>>) -> Tile<'a> {
    Tile {
        label: form.t(label),
        value: value.into(),
        look: Look::Plain,
        warn: false,
        field: None,
        tip: None,
    }
}

impl Tile<'_> {
    fn link(mut self, msg: PanelMsg, tip: Localized) -> Self {
        self.look = Look::Link(msg, tip);
        self
    }

    fn field(mut self, field: FieldId) -> Self {
        self.field = Some(field);
        self
    }

    fn tip(mut self, tip: Localized) -> Self {
        self.tip = Some(tip);
        self
    }
}

/// The tiles in reading order.
fn tiles<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Vec<Tile<'a>> {
    let stats = &board.stats;
    let from_shape = || form.t(ids::PCB_BOARD_INFO_FROM_SHAPE);
    let mut tiles = Vec::new();
    if let Some((w, h)) = board.size_mm {
        tiles.push(
            tile(form, ids::PCB_BOARD_INFO_WIDTH, length(form, w))
                .field(BOARD_WIDTH)
                .tip(from_shape()),
        );
        tiles.push(
            tile(form, ids::PCB_BOARD_INFO_HEIGHT, length(form, h))
                .field(BOARD_HEIGHT)
                .tip(from_shape()),
        );
    }
    tiles.push(tile(
        form,
        ids::PCB_BOARD_INFO_THICKNESS,
        length(form, board.thickness_mm),
    ));
    if let Some(a) = stats.area_mm2 {
        tiles.push(tile(form, ids::PCB_BOARD_INFO_AREA, area(form, a)));
    }
    tiles.push(tile(
        form,
        ids::PCB_BOARD_INFO_COMPONENTS_AREA,
        area(form, stats.components_area_mm2),
    ));
    if let Some(density) = stats.density() {
        let percent = t!(
            form.i18n,
            ids::PCB_BOARD_PERCENT,
            "value" => LocalNumber((density * 10_000.0).round() / 100.0)
        );
        tiles.push(tile(form, ids::PCB_BOARD_INFO_DENSITY, percent));
    }
    for (label, n, side) in [
        (
            ids::PCB_BOARD_INFO_COMPONENTS,
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
    ] {
        let shown = tile(form, label, count(form, n));
        tiles.push(if n > 0 {
            shown.link(
                PanelMsg::PcbBoard(PcbBoardMsg::ComponentsSelected(side)),
                form.t(ids::PCB_BOARD_INFO_SELECT_TIP),
            )
        } else {
            shown
        });
    }
    // The PCB panel's net count, so the two always agree.
    let nets = form.ctx.pcb_nets.as_ref().map_or(0, |s| s.nets.len());
    tiles.push(tile(form, ids::PCB_BOARD_INFO_NETS, count(form, nets)));
    tiles.push(unrouted(form, board));
    tiles.push(drc(form));
    tiles
}

/// Nets with a connection left to route; the not-counted state until the
/// ratsnest of this revision is worked out.
fn unrouted<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Tile<'a> {
    let label = ids::PCB_BOARD_INFO_UNROUTED_NETS;
    match board.unrouted_nets {
        Some(n) => {
            let mut shown = tile(form, label, count(form, n));
            shown.warn = n > 0;
            if n > 0 {
                shown.tip = Some(t!(form.i18n, ids::PCB_BOARD_INFO_UNROUTED_TIP, "count" => n));
            }
            shown
        }
        None => tile(form, label, form.t(ids::PCB_BOARD_UNSET))
            .tip(form.t(ids::PCB_BOARD_INFO_NOT_COUNTED)),
    }
}

/// DRC Violations: the count, which opens the results and takes the warning
/// tint above zero, or Not run.
fn drc<'a>(form: &Form<'a>) -> Tile<'a> {
    let label = ids::PCB_BOARD_INFO_DRC;
    match drc_violations(form) {
        Some(n) if n > 0 => {
            let mut shown = tile(form, label, count(form, n)).link(
                PanelMsg::PcbBoard(PcbBoardMsg::DrcResultsOpened),
                form.t(ids::PCB_BOARD_INFO_DRC_TIP),
            );
            shown.warn = true;
            shown
        }
        Some(n) => tile(form, label, count(form, n)),
        None => tile(form, label, form.t(ids::PCB_BOARD_INFO_DRC_NOT_RUN)),
    }
}

/// The tiles, `columns` to a row, the last row filled out with space.
fn tile_grid<'a>(form: &Form<'a>, tiles: Vec<Tile<'a>>, width: f32) -> Element<'a, PanelMsg> {
    let gap = SP.xs * 2.0;
    let inner = width - 2.0 * SP.m;
    let columns = if inner >= 3.0 * TS.caption * TILE_MIN_EM + 2.0 * gap {
        3
    } else {
        2
    };
    let mut grid = Column::new().spacing(gap).width(Length::Fill);
    let mut tiles = tiles.into_iter().peekable();
    while tiles.peek().is_some() {
        let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(gap).width(Length::Fill);
        for slot in 0..columns {
            line = match tiles.next() {
                Some(tile) => line.push(stat_tile(form, tile)),
                None if slot > 0 => line.push(Space::new().width(Length::FillPortion(1)).boxed()),
                None => line,
            };
        }
        grid = grid.push(line);
    }
    container(grid)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// One tile: hairline frame, label over value; a link tile is one button,
/// a warn tile has the soft warning fill, a hot tile the accent ring.
fn stat_tile<'a>(form: &Form<'a>, tile: Tile<'a>) -> Element<'a, PanelMsg> {
    let Tile {
        label,
        value,
        look,
        warn,
        field,
        tip,
    } = tile;
    let tokens = form.tokens();
    let r = resolve(tokens);
    let hot = field.as_ref().is_some_and(|f| form.is_hot(f));
    let value_ink = if warn {
        crate::styles::ti(tokens.warning)
    } else if matches!(look, Look::Link(..)) {
        r.accent_text
    } else {
        form.ink()
    };
    let label = text(label)
        .size(TS.caption)
        .color(if hot { r.accent_text } else { r.ink4 })
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End);
    let medium = iced::Font {
        weight: iced::font::Weight::Medium,
        ..mono()
    };
    let value = text(value)
        .size(TS.body)
        .font(medium)
        .color(value_ink)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End);
    let body = Column::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(label)
        .push(value);
    let edge = if hot {
        crate::styles::ti(tokens.accent)
    } else if warn {
        iced::Color::TRANSPARENT
    } else {
        r.line_soft
    };
    let fill = if warn {
        r.warn_soft
    } else {
        crate::styles::ti(tokens.panel_bg)
    };
    let framed = container(body)
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: crate::styles::HAIRLINE,
                color: edge,
                radius: crate::styles::RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let (shown, tip) = match look {
        Look::Plain => (framed.boxed(), tip),
        Look::Link(msg, link_tip) => {
            let pressable = button(framed)
                .padding(0)
                .width(Length::Fill)
                .on_press(msg)
                .style(|_: &Theme, _| button::Style::default());
            (pressable.boxed(), tip.or(Some(link_tip)))
        }
    };
    let shown = match tip {
        Some(tip) => form.tip(shown, tip),
        None => shown,
    };
    let shown = match field {
        Some(field) => form.hover_zone(field, shown),
        None => shown,
    };
    container(shown).width(Length::FillPortion(1)).boxed()
}

/// The fold under the tiles: Layers and Signal, the hidden nets, and the
/// kinds the board holds as name | count pairs two to a row. Kinds the board
/// cannot hold yet are left out. Closed by default; a search opens it.
fn more<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    let stats = &board.stats;
    let pairs = [
        (ids::PCB_BOARD_INFO_LAYERS, board.layers),
        (ids::PCB_BOARD_INFO_SIGNAL, board.copper_layers),
        (ids::PCB_BOARD_INFO_PADS, stats.pads),
        (ids::PCB_BOARD_INFO_TRACKS, stats.tracks),
        (ids::PCB_BOARD_INFO_VIAS, stats.vias),
        (ids::PCB_BOARD_INFO_POLYGONS, stats.polygons),
        (ids::PCB_BOARD_INFO_FILLS, stats.fills),
        (ids::PCB_BOARD_INFO_REGIONS, stats.regions),
        (ids::PCB_BOARD_INFO_KEEPOUTS, stats.keepouts),
        (ids::PCB_BOARD_INFO_STRINGS, stats.strings),
        (ids::PCB_BOARD_INFO_HOLES, stats.pad_holes + stats.via_holes),
        (ids::PCB_BOARD_INFO_HIDDEN_NETS, board.hidden_nets),
    ];
    let title = form.t(ids::PCB_BOARD_INFO_MORE);
    let mut keys = vec![title.clone()];
    let open = form.is_searching() || is_open(form.ctx, MORE_KEY, true);
    let faint = form.faint();
    let header = button(
        row![
            signex_widgets::chevron::chevron(open, TS.caption, faint),
            text(title)
                .size(TS.caption)
                .color(faint)
                .wrapping(Wrapping::None),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .on_press(PanelMsg::ToggleSection(toggle_key(MORE_KEY, true)))
    .style(|_: &Theme, _| button::Style::default());
    let mut body: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).push(header.boxed());
    let mut grid = Column::new()
        .spacing(SP.xs)
        .padding([0.0, SP.m])
        .width(Length::Fill);
    for chunk in pairs.chunks(2) {
        let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m).width(Length::Fill);
        for &(label, n) in chunk {
            let label = form.t(label);
            keys.push(label.clone());
            line = line.push(key_value(form, label, count(form, n)));
        }
        if chunk.len() == 1 {
            line = line.push(Space::new().width(Length::FillPortion(1)).boxed());
        }
        grid = grid.push(line);
    }
    if open {
        body = body.push(
            container(grid)
                .padding(iced::Padding {
                    bottom: SP.s,
                    ..iced::Padding::ZERO
                })
                .boxed(),
        );
    }
    let edge = resolve(form.tokens()).line_strong;
    let fold = container(body)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: crate::styles::HAIRLINE,
                color: edge,
                radius: crate::styles::RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let fold = container(fold).padding([SP.xs, SP.l]).width(Length::Fill);
    let refs: Vec<&Localized> = keys.iter().collect();
    form.row(&refs, fold.boxed())
}

/// A name | count pair, half a fold row wide.
fn key_value<'a>(form: &Form<'a>, label: Localized, value: Localized) -> Element<'a, PanelMsg> {
    let pair = row![
        container(
            text(label)
                .size(TS.body)
                .color(form.muted())
                .wrapping(Wrapping::None)
        )
        .width(Length::Fill),
        form.value(value),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    container(pair).width(Length::FillPortion(1)).boxed()
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

/// The violations of the board's last design rule check; `None` before one.
pub(super) fn drc_violations(form: &Form<'_>) -> Option<usize> {
    let drc = form.ctx.pcb_drc.as_ref().filter(|d| d.ran)?;
    Some(drc.rows.len() + drc.hidden)
}
