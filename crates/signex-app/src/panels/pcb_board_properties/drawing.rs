//! The board illustration under Search (#2523, mockup row 93): line art of the board
//! outline from the board shape (its bounding box when the shape is not a
//! rectangle), the origin, the extra grids' origins in their colours, the
//! snap grid with its step and, while Visible Grid is on, the grid lines.
//! Width, Height, the snap step, Visible Grid and each grid card are linked
//! to their parts: hovering either side lights the other. Under it the size
//! row: the size read from the board shape and Edit board shape…, which
//! opens Board sketch. Nothing on the drawing edits.

use iced::widget::text::Wrapping;
use iced::widget::{Column, container, row};
use iced::{Alignment, Element, Length, Point, Rectangle, Size, Widget as _};
use iced_aw::Wrap;
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::{format_length_mm, length_field_text};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Paint, Part, Primitive, Style, TextLabel,
    TextRole, Tint, Weight, property_illustration,
};
use signex_widgets::text;
use uuid::Uuid;

use super::chrome::Form;
use super::info::shown_unit;
use super::{PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;

/// The Width tile and the width callout.
pub const BOARD_WIDTH: FieldId = FieldId::new("board.width");
/// The Height tile and the height callout.
pub const BOARD_HEIGHT: FieldId = FieldId::new("board.height");
/// The board's own grid card (its step) and the snap grid with its callout.
pub const SNAP_STEP: FieldId = FieldId::new("board.snap");
/// The Visible Grid switch; also the feature that draws the grid lines.
pub const VISIBLE_GRID: FieldId = FieldId::new("board.visible");

/// An extra grid's card and its origin cross.
pub fn grid_field(grid: Uuid) -> FieldId {
    FieldId::owned(format!("board.grid.{grid}"))
}

/// The view box, in the units of the approved drawing.
const VIEW: Size = Size::new(264.0, 120.0);
/// Where the board outline is fitted: left, top, right, bottom.
const BOARD_BOX: (f32, f32, f32, f32) = (84.0, 16.0, 222.0, 92.0);
/// Height of the widget, in captions: the view box at a 300 px dock.
const HEIGHT_EM: f32 = 11.0;
/// The snap grid sample, left of the board, and its dot pitch range.
const SNAP_AT: Point = Point::new(36.0, 48.0);
const SNAP_PITCH: (f32, f32) = (4.0, 10.0);
/// The 'Snap …' label starts at the left edge, so it stays clear of the
/// board.
const SNAP_LABEL_X: f32 = 2.0;
/// Visible grid lines are drawn at a multiple of the step at least this
/// far apart, and not more of them than this a side.
const GRID_MIN_PITCH: f32 = 8.0;
const GRID_MAX_LINES: usize = 40;
const GRID_MULTIPLES: [f64; 14] = [
    1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
];
/// Half the arm of an origin cross, and the radius of the origin mark.
const CROSS: f32 = 4.0;
const ORIGIN_R: f32 = 4.5;

/// The illustration (with a board outline) and the size row.
pub(super) fn view<'a>(form: &Form<'a>, board: &'a PcbBoardSummary) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs).width(Length::Fill);
    if let Some(extent) = board.extent_mm {
        let drawing = illustration(form, board, extent);
        let hover = &board.view.illustration_hover;
        let canvas = property_illustration(drawing, hover, form.tokens(), TS.caption)
            .map(|m| PanelMsg::PcbBoard(PcbBoardMsg::IllustrationHovered(m)))
            .boxed();
        col = col.push(canvas);
    }
    col = col.push(size_row(form, board));
    container(col)
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// 'Size W × H mm' (or 'Bounding box …', or no outline), '· from board
/// shape' and Edit board shape….
fn size_row<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let unit = shown_unit(form);
    let hot = form.is_hot(&BOARD_WIDTH) || form.is_hot(&BOARD_HEIGHT);
    let ink = if hot { form.accent() } else { form.faint() };
    let readout = text(size_readout(form.i18n, board, unit))
        .size(TS.caption)
        .color(ink)
        .wrapping(Wrapping::None);
    let readout = form.hover_zone(BOARD_WIDTH, readout.boxed());
    let mut size = row![readout].spacing(SP.s).align_y(Alignment::Center);
    if board.extent_mm.is_some() {
        size = size.push(
            text(form.t(ids::PCB_BOARD_SIZE_SOURCE))
                .size(TS.caption)
                .color(form.faint())
                .wrapping(Wrapping::None)
                .boxed(),
        );
    }
    let (press, tip) = if board.read_only {
        (None, form.t(ids::PCB_FOOTPRINT_READ_ONLY))
    } else {
        (
            Some(PanelMsg::PcbBoard(PcbBoardMsg::BoardShapeEditPressed)),
            form.t(ids::PCB_BOARD_EDIT_SHAPE_TIP),
        )
    };
    let edit = form.framed_action(form.t(ids::PCB_BOARD_EDIT_SHAPE), press, Some(tip));
    // The button goes under the readout when the dock is too narrow for both.
    let line = Wrap::new()
        .spacing(SP.m)
        .line_spacing(SP.xs)
        .push(container(size).padding([SP.xs, 0.0]).boxed())
        .push(edit);
    Element::from(line)
}

/// What the size row reads: 'Size W × H mm' for a rectangular outline,
/// 'Bounding box W × H mm' for any other, and that there is no outline.
pub fn size_readout(i18n: &I18n, board: &PcbBoardSummary, unit: Unit) -> Localized {
    let unit = super::shown_unit(unit);
    let Some((w, h)) = board.size_mm else {
        return t!(i18n, ids::PCB_BOARD_INFO_NO_OUTLINE);
    };
    let id = if board.outline_is_rectangle {
        ids::PCB_BOARD_SIZE_READOUT
    } else {
        ids::PCB_BOARD_SIZE_BOUNDING
    };
    t!(
        i18n,
        id,
        "width" => length_field_text(w, unit),
        "height" => length_field_text(h, unit),
        "unit" => unit.to_string()
    )
}

/// Maps model mm (y down) into the view box.
#[derive(Clone, Copy)]
struct Map {
    scale: f64,
    dx: f64,
    dy: f64,
}

impl Map {
    fn fit(extent: (f64, f64, f64, f64)) -> Self {
        let (x0, y0, x1, y1) = extent;
        let (bx0, by0, bx1, by1) = BOARD_BOX;
        let w = (x1 - x0).max(1e-6);
        let h = (y1 - y0).max(1e-6);
        let scale = (f64::from(bx1 - bx0) / w).min(f64::from(by1 - by0) / h);
        let cx = f64::from(bx0 + bx1) / 2.0;
        let cy = f64::from(by0 + by1) / 2.0;
        Self {
            scale,
            dx: cx - (x0 + x1) / 2.0 * scale,
            dy: cy - (y0 + y1) / 2.0 * scale,
        }
    }

    fn point(self, x: f64, y: f64) -> Point {
        Point::new(
            (self.dx + x * self.scale) as f32,
            (self.dy + y * self.scale) as f32,
        )
    }
}

/// Whether `p` lies in the view box, so a mark there is drawn.
fn in_view(p: Point) -> bool {
    (0.0..=VIEW.width).contains(&p.x) && (0.0..=VIEW.height).contains(&p.y)
}

fn label(
    at: Point,
    label: impl Into<signex_widgets::property_illustration::Text>,
    anchor: Anchor,
    role: TextRole,
) -> TextLabel {
    TextLabel {
        at,
        text: label.into(),
        anchor,
        role,
    }
}

/// `mm` in the panel's unit, with the unit.
fn measure(form: &Form<'_>, mm: f64) -> String {
    format_length_mm(mm, shown_unit(form))
}

fn illustration(
    form: &Form<'_>,
    board: &PcbBoardSummary,
    extent: (f64, f64, f64, f64),
) -> Illustration {
    let map = Map::fit(extent);
    let (x0, y0, x1, y1) = extent;
    let top_left = map.point(x0, y0);
    let bottom_right = map.point(x1, y1);
    let mut drawing = Illustration::new(VIEW.width, VIEW.height)
        .height_em(HEIGHT_EM)
        .feature(VISIBLE_GRID, form.ctx.grid_visible);
    if let Some(step) = board.grid_mm.filter(|s| *s > 0.0) {
        drawing = drawing.parts(grid_lines(board, map, extent, step));
    }
    let outline = Rectangle::new(
        top_left,
        Size::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y),
    );
    drawing = drawing.part(
        Part::new(
            Primitive::RoundedRect {
                rect: outline,
                radius: 3.0,
            },
            Style::edge(),
        )
        .linked_all([BOARD_WIDTH, BOARD_HEIGHT]),
    );
    drawing = drawing.parts(grid_origins(board, map));
    drawing = drawing.parts(origin(form, board, map));
    drawing = drawing.parts(size_callouts(form, board, top_left, bottom_right));
    if let Some(step) = board.grid_mm.filter(|s| *s > 0.0) {
        drawing = drawing.parts(snap_sample(form, step));
    }
    drawing
}

/// The visible grid over the board box, at a multiple of the step counted
/// from the board origin; drawn only while Visible Grid is on.
fn grid_lines(
    board: &PcbBoardSummary,
    map: Map,
    extent: (f64, f64, f64, f64),
    step: f64,
) -> Vec<Part> {
    let (x0, y0, x1, y1) = extent;
    let Some(pitch) = GRID_MULTIPLES
        .iter()
        .map(|n| n * step)
        .find(|pitch| pitch * map.scale >= f64::from(GRID_MIN_PITCH))
    else {
        return Vec::new();
    };
    let origin = board.options.origin.at;
    let first = |lo: f64, at: f64| at + ((lo - at) / pitch).ceil() * pitch;
    let style = Style::outline(Ink::Rule, Weight::Hairline);
    let line = |from: Point, to: Point| {
        Part::new(Primitive::Line { from, to }, style)
            .linked_all([VISIBLE_GRID, SNAP_STEP])
            .requires(VISIBLE_GRID)
    };
    let mut parts = Vec::new();
    let mut x = first(x0, origin.x);
    while x < x1 && parts.len() < GRID_MAX_LINES {
        parts.push(line(map.point(x, y0), map.point(x, y1)));
        x += pitch;
    }
    let mut y = first(y0, origin.y);
    let across = parts.len();
    while y < y1 && parts.len() < across + GRID_MAX_LINES {
        parts.push(line(map.point(x0, y), map.point(x1, y)));
        y += pitch;
    }
    parts
}

/// A cross in each extra grid's colour at its origin, with its name.
fn grid_origins(board: &PcbBoardSummary, map: Map) -> Vec<Part> {
    let mut parts = Vec::new();
    for grid in &board.options.grids {
        let at = map.point(grid.origin.x, grid.origin.y);
        if !in_view(at) {
            continue;
        }
        let field = grid_field(grid.uuid);
        let paint = Paint::Document(crate::render_config::to_iced(&grid.color));
        let style = Style::outline(Ink::Secondary, Weight::Regular).with_stroke_paint(paint);
        let arm = |dx: f32, dy: f32| Primitive::Line {
            from: Point::new(at.x - dx, at.y - dy),
            to: Point::new(at.x + dx, at.y + dy),
        };
        parts.push(Part::new(arm(CROSS, 0.0), style).linked(field.clone()));
        parts.push(Part::new(arm(0.0, CROSS), style).linked(field.clone()));
        let name = label(
            Point::new(at.x + CROSS + 1.0, at.y - CROSS - 1.0),
            UserText::new(grid.name.clone()),
            Anchor::Start,
            TextRole::Note,
        );
        parts.push(Part::label(name, Ink::Faint).linked(field));
    }
    parts
}

/// The board origin: a ring with two filled opposite quarters, and '0, 0'.
fn origin(form: &Form<'_>, board: &PcbBoardSummary, map: Map) -> Vec<Part> {
    let at = board.options.origin.at;
    let c = map.point(at.x, at.y);
    if !in_view(c) {
        return Vec::new();
    }
    let ring = Part::new(
        Primitive::Circle {
            center: c,
            radius: ORIGIN_R,
        },
        Style::outline(Ink::Secondary, Weight::Regular),
    )
    .linked(SNAP_STEP);
    // Quarters from the centre: up to right, and down to left.
    let quarter = |sx: f32, sy: f32| {
        let mut points = vec![c];
        for k in 0..=4 {
            let a = std::f32::consts::FRAC_PI_2 * k as f32 / 4.0;
            points.push(Point::new(
                c.x + sx * ORIGIN_R * a.sin(),
                c.y + sy * ORIGIN_R * a.cos(),
            ));
        }
        Part::new(
            Primitive::Polyline {
                points,
                is_closed: true,
            },
            Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
        )
        .linked(SNAP_STEP)
    };
    let zero = t!(form.i18n, ids::PCB_BOARD_POINT, "x" => "0", "y" => "0");
    let name = label(
        Point::new(c.x + ORIGIN_R + 1.0, c.y + ORIGIN_R + 3.0),
        zero,
        Anchor::Start,
        TextRole::Note,
    );
    vec![
        ring,
        quarter(1.0, -1.0),
        quarter(-1.0, 1.0),
        Part::label(name, Ink::Faint).linked(SNAP_STEP),
    ]
}

/// The width callout under the board, '{W} · from board shape', and the
/// height callout at its right, with the height over its top end.
fn size_callouts(
    form: &Form<'_>,
    board: &PcbBoardSummary,
    top_left: Point,
    bottom_right: Point,
) -> Vec<Part> {
    let Some((w, h)) = board.size_mm else {
        return Vec::new();
    };
    let (left, top, right, bottom) = (top_left.x, top_left.y, bottom_right.x, bottom_right.y);
    let width_text = t!(form.i18n, ids::PCB_BOARD_CALLOUT_WIDTH, "value" => measure(form, w));
    let below = bottom + 9.0;
    let width = Callout::new(Point::new(left, below), Point::new(right, below))
        .label(width_text)
        .label_at(
            Point::new((left + right) / 2.0, below + 10.0),
            Anchor::Middle,
        )
        .extension(
            Point::new(left, bottom + 2.0),
            Point::new(left, below + 3.0),
        )
        .extension(
            Point::new(right, bottom + 2.0),
            Point::new(right, below + 3.0),
        );
    let beside = right + 8.0;
    let height_text = t!(form.i18n, ids::PCB_BOARD_MEASURE, "value" => measure(form, h));
    let height = Callout::new(Point::new(beside, bottom), Point::new(beside, top))
        .label(height_text)
        .label_at(Point::new(beside + 3.0, top - 7.0), Anchor::End)
        .extension(Point::new(right + 2.0, top), Point::new(beside + 3.0, top))
        .extension(
            Point::new(right + 2.0, bottom),
            Point::new(beside + 3.0, bottom),
        );
    vec![
        Part::callout(width).linked(BOARD_WIDTH),
        Part::callout(height).linked(BOARD_HEIGHT),
    ]
}

/// The snap grid as a patch of dots left of the board, with one pitch
/// dimensioned 'Snap {step}'.
fn snap_sample(form: &Form<'_>, step: f64) -> Vec<Part> {
    let (lo, hi) = SNAP_PITCH;
    // A finer step draws closer dots, within a readable range.
    let pitch = ((step / 0.1) as f32 * 7.0).clamp(lo, hi);
    let dot = Style::filled(Paint::Ink(Ink::Faint), Tint::Solid);
    let mut parts = Vec::new();
    for i in -2..=2 {
        for j in -2..=2 {
            let center = Point::new(SNAP_AT.x + i as f32 * pitch, SNAP_AT.y + j as f32 * pitch);
            parts.push(
                Part::new(
                    Primitive::Circle {
                        center,
                        radius: 0.9,
                    },
                    dot,
                )
                .linked(SNAP_STEP),
            );
        }
    }
    let below = SNAP_AT.y + 2.0 * pitch + 8.0;
    let value = measure(form, step);
    let callout = Callout::new(
        Point::new(SNAP_AT.x, below),
        Point::new(SNAP_AT.x + pitch, below),
    )
    .label(t!(form.i18n, ids::PCB_BOARD_CALLOUT_SNAP, "value" => value))
    .label_at(Point::new(SNAP_LABEL_X, below + 10.0), Anchor::Start);
    parts.push(Part::callout(callout).linked(SNAP_STEP));
    parts
}
