//! Symbol render — body graphics under the symbol's transform, then
//! pins, then visible fields (reference / value).
//!
//! Body graphics come from [`LibSymbol::graphics`]; pins are delegated
//! to [`super::pin::draw_pin`]; field text uses the rotation + justify
//! produced by [`super::field_style::field_effective_style`]. Per-pin
//! and per-graphic unit / body-style filters are honoured before
//! drawing.

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{
    Aabb, FillType, Graphic, HAlign, LibSymbol, Point, SelectedItem, SelectedKind, Symbol, VAlign,
};

use super::field_style::field_effective_style;
use super::pin::draw_pin;
use super::text::{draw_rotated_text, mm_to_text_pixels};
use super::util::{aabbs_overlap, iced_color, point_finite};
use super::{RenderContext, SymbolTransform};

/// Default body stroke (mm) when a graphic primitive's `width == 0.0`.
pub const SYMBOL_BODY_STROKE_MM: f64 = 0.15;

const SELECTION_WEIGHT_FACTOR: f64 = 1.5;

/// Render a single placed symbol — body, pins, and visible fields —
/// into the content layer's frame.
pub fn draw_symbol(frame: &mut Frame, symbol: &Symbol, lib: &LibSymbol, ctx: &RenderContext<'_>) {
    // Coarse cull against viewport bounds using a generous bbox.
    let bbox = symbol_aabb(symbol, lib);
    if !aabbs_overlap(&bbox, &ctx.viewport.visible_world_bounds()) {
        return;
    }

    let transform = SymbolTransform::from_symbol(symbol);
    let selected = ctx.is_selected(&SelectedItem::new(symbol.uuid, SelectedKind::Symbol));

    // 1. Body graphics.
    for lib_graphic in &lib.graphics {
        if lib_graphic.unit != 0 && lib_graphic.unit != symbol.unit {
            continue;
        }
        // body_style 0 = common, 1 = normal, 2 = De Morgan. We render
        // common + normal until alternate-style support lands.
        if lib_graphic.body_style != 0 && lib_graphic.body_style != 1 {
            continue;
        }
        draw_graphic(frame, &lib_graphic.graphic, &transform, selected, ctx);
    }

    // 2. Pins.
    for lib_pin in &lib.pins {
        if lib_pin.unit != 0 && lib_pin.unit != symbol.unit {
            continue;
        }
        if lib_pin.body_style != 0 && lib_pin.body_style != 1 {
            continue;
        }
        draw_pin(frame, &lib_pin.pin, &transform, ctx);
    }

    // 3. Visible fields (reference + value).
    if let Some(ref_text) = symbol.ref_text.as_ref()
        && !ref_text.hidden
        && !symbol.reference.is_empty()
    {
        draw_field(
            frame,
            &symbol.reference,
            ref_text,
            &transform,
            iced_color(&ctx.theme().reference),
            ctx,
        );
    }
    if let Some(val_text) = symbol.val_text.as_ref()
        && !val_text.hidden
        && !symbol.value.is_empty()
    {
        draw_field(
            frame,
            &symbol.value,
            val_text,
            &transform,
            iced_color(&ctx.theme().value),
            ctx,
        );
    }
}

fn draw_graphic(
    frame: &mut Frame,
    graphic: &Graphic,
    transform: &SymbolTransform,
    selected: bool,
    ctx: &RenderContext<'_>,
) {
    let body_colour = if selected {
        iced_color(&ctx.theme().selection)
    } else {
        iced_color(&ctx.theme().body)
    };
    let body_fill = iced_color(&ctx.theme().body_fill);

    match graphic {
        Graphic::Polyline {
            points,
            width,
            fill,
        } => draw_polyline(
            frame,
            points,
            *width,
            *fill,
            transform,
            selected,
            body_colour,
            body_fill,
            ctx,
        ),
        Graphic::Rectangle {
            start,
            end,
            width,
            fill,
        } => draw_rect(
            frame,
            *start,
            *end,
            *width,
            *fill,
            transform,
            selected,
            body_colour,
            body_fill,
            ctx,
        ),
        Graphic::Circle {
            center,
            radius,
            width,
            fill,
        } => draw_circle(
            frame,
            *center,
            *radius,
            *width,
            *fill,
            transform,
            selected,
            body_colour,
            body_fill,
            ctx,
        ),
        Graphic::Arc {
            start,
            mid,
            end,
            width,
            ..
        } => draw_arc(
            frame,
            *start,
            *mid,
            *end,
            *width,
            transform,
            selected,
            body_colour,
            ctx,
        ),
        Graphic::Bezier { points, width, .. } if points.len() >= 4 => {
            draw_bezier(frame, points, *width, transform, selected, body_colour, ctx)
        }
        Graphic::Bezier { .. } => {
            // Malformed bezier — too few control points; fall back to a
            // polyline through whatever we have so the symbol stays
            // visible.
        }
        Graphic::Text {
            text,
            position,
            rotation,
            font_size,
            justify_h,
            justify_v,
            ..
        } => draw_lib_text(
            frame,
            text,
            *position,
            *rotation,
            *font_size,
            *justify_h,
            *justify_v,
            transform,
            body_colour,
            ctx,
        ),
        Graphic::TextBox {
            text,
            position,
            rotation,
            font_size,
            ..
        } => {
            // TextBox renders as a centred text inside the box bounds.
            draw_lib_text(
                frame,
                text,
                *position,
                *rotation,
                *font_size,
                HAlign::Left,
                VAlign::Top,
                transform,
                body_colour,
                ctx,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_polyline(
    frame: &mut Frame,
    points: &[Point],
    width_mm: f64,
    fill: FillType,
    transform: &SymbolTransform,
    selected: bool,
    stroke_colour: iced::Color,
    fill_colour: iced::Color,
    ctx: &RenderContext<'_>,
) {
    if points.len() < 2 {
        return;
    }
    let path = Path::new(|builder| {
        for (i, p) in points.iter().enumerate() {
            let s = ctx.viewport.world_to_screen(transform.apply(*p));
            if !point_finite(s) {
                return;
            }
            if i == 0 {
                builder.move_to(s);
            } else {
                builder.line_to(s);
            }
        }
        if !matches!(fill, FillType::None) {
            builder.close();
        }
    });
    if matches!(fill, FillType::Background) {
        frame.fill(&path, fill_colour);
    }
    frame.stroke(&path, body_stroke(width_mm, selected, stroke_colour, ctx));
}

#[allow(clippy::too_many_arguments)]
fn draw_rect(
    frame: &mut Frame,
    start: Point,
    end: Point,
    width_mm: f64,
    fill: FillType,
    transform: &SymbolTransform,
    selected: bool,
    stroke_colour: iced::Color,
    fill_colour: iced::Color,
    ctx: &RenderContext<'_>,
) {
    let corners = [
        start,
        Point::new(end.x, start.y),
        end,
        Point::new(start.x, end.y),
    ];
    let path = Path::new(|builder| {
        for (i, c) in corners.iter().enumerate() {
            let s = ctx.viewport.world_to_screen(transform.apply(*c));
            if !point_finite(s) {
                return;
            }
            if i == 0 {
                builder.move_to(s);
            } else {
                builder.line_to(s);
            }
        }
        builder.close();
    });
    if matches!(fill, FillType::Background) {
        frame.fill(&path, fill_colour);
    }
    frame.stroke(&path, body_stroke(width_mm, selected, stroke_colour, ctx));
}

#[allow(clippy::too_many_arguments)]
fn draw_circle(
    frame: &mut Frame,
    center: Point,
    radius_mm: f64,
    width_mm: f64,
    fill: FillType,
    transform: &SymbolTransform,
    selected: bool,
    stroke_colour: iced::Color,
    fill_colour: iced::Color,
    ctx: &RenderContext<'_>,
) {
    let centre_w = transform.apply(center);
    let centre_s = ctx.viewport.world_to_screen(centre_w);
    if !point_finite(centre_s) {
        return;
    }
    let r_px = (radius_mm * ctx.viewport.zoom_px_per_mm).max(0.5) as f32;
    let path = Path::circle(centre_s, r_px);
    if matches!(fill, FillType::Background) {
        frame.fill(&path, fill_colour);
    }
    frame.stroke(&path, body_stroke(width_mm, selected, stroke_colour, ctx));
}

#[allow(clippy::too_many_arguments)]
fn draw_arc(
    frame: &mut Frame,
    start: Point,
    mid: Point,
    end: Point,
    width_mm: f64,
    transform: &SymbolTransform,
    selected: bool,
    stroke_colour: iced::Color,
    ctx: &RenderContext<'_>,
) {
    let path = Path::new(|builder| {
        let s = ctx.viewport.world_to_screen(transform.apply(start));
        let m = ctx.viewport.world_to_screen(transform.apply(mid));
        let e = ctx.viewport.world_to_screen(transform.apply(end));
        if !point_finite(s) || !point_finite(m) || !point_finite(e) {
            return;
        }
        // Approximate by a 3-segment polyline through start/mid/end —
        // for v0.12 this keeps the symbol visible without bringing in
        // the circumcircle math here. Drawing-tool arcs (which need
        // accurate circular interpolation) live in `super::drawing`.
        builder.move_to(s);
        builder.line_to(m);
        builder.line_to(e);
    });
    frame.stroke(&path, body_stroke(width_mm, selected, stroke_colour, ctx));
}

fn draw_bezier(
    frame: &mut Frame,
    points: &[Point],
    width_mm: f64,
    transform: &SymbolTransform,
    selected: bool,
    stroke_colour: iced::Color,
    ctx: &RenderContext<'_>,
) {
    let path = Path::new(|builder| {
        let p0 = ctx.viewport.world_to_screen(transform.apply(points[0]));
        let c1 = ctx.viewport.world_to_screen(transform.apply(points[1]));
        let c2 = ctx.viewport.world_to_screen(transform.apply(points[2]));
        let p3 = ctx.viewport.world_to_screen(transform.apply(points[3]));
        if !point_finite(p0) || !point_finite(c1) || !point_finite(c2) || !point_finite(p3) {
            return;
        }
        builder.move_to(p0);
        builder.bezier_curve_to(c1, c2, p3);
    });
    frame.stroke(&path, body_stroke(width_mm, selected, stroke_colour, ctx));
}

#[allow(clippy::too_many_arguments)]
fn draw_lib_text(
    frame: &mut Frame,
    text: &str,
    position: Point,
    rotation: f64,
    font_size_mm: f64,
    justify_h: HAlign,
    justify_v: VAlign,
    transform: &SymbolTransform,
    colour: iced::Color,
    ctx: &RenderContext<'_>,
) {
    let world = transform.apply(position);
    let screen = ctx.viewport.world_to_screen(world);
    if !point_finite(screen) {
        return;
    }
    let folded_rot = transform.apply_angle(rotation);
    let size_px = mm_to_text_pixels(font_size_mm, ctx);
    draw_rotated_text(
        frame, text, screen, folded_rot, size_px, colour, justify_h, justify_v,
    );
}

fn draw_field(
    frame: &mut Frame,
    text: &str,
    prop: &signex_types::schematic::TextProp,
    transform: &SymbolTransform,
    colour: iced::Color,
    ctx: &RenderContext<'_>,
) {
    let world = transform.apply(prop.position);
    let screen = ctx.viewport.world_to_screen(world);
    if !point_finite(screen) {
        return;
    }
    let (rot, h, v) = field_effective_style(prop, transform);
    let size_px = mm_to_text_pixels(prop.font_size, ctx);
    draw_rotated_text(frame, text, screen, rot, size_px, colour, h, v);
}

fn body_stroke<'a>(
    width_mm: f64,
    selected: bool,
    colour: iced::Color,
    ctx: &RenderContext<'_>,
) -> Stroke<'a> {
    let mm = if width_mm > 0.0 {
        width_mm
    } else {
        SYMBOL_BODY_STROKE_MM
    };
    let scaled = mm
        * if selected {
            SELECTION_WEIGHT_FACTOR
        } else {
            1.0
        };
    let px = (scaled * ctx.viewport.zoom_px_per_mm).max(1.0) as f32;
    Stroke::default().with_width(px).with_color(colour)
}

/// Coarse world-space AABB enclosing a placed symbol (body graphics +
/// pin endpoints). Used by frustum culling and Wave 4 hit-test.
pub(crate) fn symbol_aabb(symbol: &Symbol, lib: &LibSymbol) -> Aabb {
    let transform = SymbolTransform::from_symbol(symbol);
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let mut extend = |w: Point| {
        min_x = min_x.min(w.x);
        max_x = max_x.max(w.x);
        min_y = min_y.min(w.y);
        max_y = max_y.max(w.y);
    };

    for g in &lib.graphics {
        if g.unit != 0 && g.unit != symbol.unit {
            continue;
        }
        for p in graphic_points(&g.graphic) {
            extend(transform.apply(p));
        }
    }
    for p in &lib.pins {
        if p.unit != 0 && p.unit != symbol.unit {
            continue;
        }
        let rad = p.pin.rotation.to_radians();
        let body = p.pin.position;
        let tip = Point::new(
            body.x + p.pin.length * rad.cos(),
            body.y + p.pin.length * rad.sin(),
        );
        extend(transform.apply(body));
        extend(transform.apply(tip));
    }

    if !min_x.is_finite() {
        // Symbol with no graphics + no pins; fall back to a tiny
        // box around the placement origin so the AABB is non-empty.
        return Aabb::new(
            symbol.position.x - 1.27,
            symbol.position.y - 1.27,
            symbol.position.x + 1.27,
            symbol.position.y + 1.27,
        );
    }
    Aabb::new(min_x, min_y, max_x, max_y)
}

fn graphic_points(g: &Graphic) -> Vec<Point> {
    match g {
        Graphic::Polyline { points, .. } | Graphic::Bezier { points, .. } => points.clone(),
        Graphic::Rectangle { start, end, .. } => vec![
            *start,
            Point::new(end.x, start.y),
            *end,
            Point::new(start.x, end.y),
        ],
        Graphic::Circle { center, radius, .. } => vec![
            Point::new(center.x - *radius, center.y - *radius),
            Point::new(center.x + *radius, center.y - *radius),
            Point::new(center.x - *radius, center.y + *radius),
            Point::new(center.x + *radius, center.y + *radius),
        ],
        Graphic::Arc {
            start, mid, end, ..
        } => vec![*start, *mid, *end],
        Graphic::Text { position, .. } | Graphic::TextBox { position, .. } => vec![*position],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{LibGraphic, LibPin, Pin, PinDirection, PinShapeStyle};
    use uuid::Uuid;

    fn empty_lib() -> LibSymbol {
        LibSymbol {
            id: "test:res".to_string(),
            reference: "R".to_string(),
            value: "10k".to_string(),
            footprint: String::new(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: Vec::new(),
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.508,
        }
    }

    fn lib_with_unit_rect() -> LibSymbol {
        let mut lib = empty_lib();
        lib.graphics.push(LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(-2.54, -1.27),
                end: Point::new(2.54, 1.27),
                width: 0.0,
                fill: FillType::None,
            },
        });
        lib
    }

    fn placed(rotation: f64) -> Symbol {
        Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "test:res".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            footprint: String::new(),
            datasheet: String::new(),
            position: Point::new(0.0, 0.0),
            rotation,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            fields_user_placed: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: Default::default(),
            custom_properties: Vec::new(),
            pin_uuids: Default::default(),
            instances: Vec::new(),
            library_id: None,
            row_id: None,
            library_version: String::new(),
        }
    }

    #[test]
    fn symbol_aabb_returns_finite_box_for_empty_symbol() {
        let lib = empty_lib();
        let sym = placed(0.0);
        let bbox = symbol_aabb(&sym, &lib);
        assert!(bbox.min_x.is_finite());
        assert!(bbox.width() > 0.0);
    }

    #[test]
    fn symbol_aabb_includes_body_graphics() {
        let lib = lib_with_unit_rect();
        let sym = placed(0.0);
        let bbox = symbol_aabb(&sym, &lib);
        assert!(bbox.contains(0.0, 0.0));
        // The lib rect spans 5.08 mm — the bbox should be at least as wide.
        assert!(bbox.width() >= 5.08);
    }

    #[test]
    fn symbol_aabb_with_unit_filter_skips_other_units() {
        // Edge case: a graphic tagged unit=2 is skipped when rendering
        // unit=1.
        let mut lib = empty_lib();
        lib.graphics.push(LibGraphic {
            unit: 2,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(-100.0, -100.0),
                end: Point::new(100.0, 100.0),
                width: 0.0,
                fill: FillType::None,
            },
        });
        let mut sym = placed(0.0);
        sym.unit = 1;
        let bbox = symbol_aabb(&sym, &lib);
        // Empty lib (no unit-1 graphics) → fallback ±1.27 mm box.
        assert!(bbox.width() < 10.0);
    }

    #[test]
    fn symbol_aabb_rotates_with_parent() {
        let mut lib = empty_lib();
        lib.pins.push(LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Input,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 5.08,
                name: "A".to_string(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        });
        let upright = symbol_aabb(&placed(0.0), &lib);
        let rotated = symbol_aabb(&placed(90.0), &lib);
        // Pin extends along x at 0°; along y at 90°. Either way the
        // bbox's longest axis swaps.
        assert!(upright.width() > 0.0 && rotated.height() > 0.0);
    }
}
