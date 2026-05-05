//! Clean-room schematic runtime used by `signex-app` during Milestone F.
//!
//! This module intentionally avoids importing `signex_render::schematic`.
//! It provides a local rendering, hit-test, and overlay surface that matches
//! the app's runtime contract while the full `signex-renderer` cutover lands.

use iced::advanced::text as advanced_text;
use iced::alignment;
use iced::widget::canvas;
use iced::{Color, Rectangle, Vector};
use signex_types::schematic::{
    Aabb, FillType, HAlign, Label, LabelType, Point, SchDrawing, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, TextProp, TextNote, VAlign,
};
use signex_types::theme::{CanvasColors, Color as ThemeColor};
use std::collections::{HashMap, HashSet};

pub type SchematicRenderSnapshot = SchematicSheet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderInvalidation(u32);

impl RenderInvalidation {
    pub const NONE: Self = Self(0);
    pub const SYMBOLS: Self = Self(1 << 0);
    pub const WIRES: Self = Self(1 << 1);
    pub const LABELS: Self = Self(1 << 2);
    pub const TEXT_NOTES: Self = Self(1 << 3);
    pub const BUSES: Self = Self(1 << 4);
    pub const BUS_ENTRIES: Self = Self(1 << 5);
    pub const JUNCTIONS: Self = Self(1 << 6);
    pub const NO_CONNECTS: Self = Self(1 << 7);
    pub const CHILD_SHEETS: Self = Self(1 << 8);
    pub const DRAWINGS: Self = Self(1 << 9);
    pub const LIB_SYMBOLS: Self = Self(1 << 10);
    pub const PAPER: Self = Self(1 << 11);
    pub const FULL: Self = Self(u32::MAX);
}

impl std::ops::BitOr for RenderInvalidation {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for RenderInvalidation {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenTransform {
    pub offset_x: f32,
    pub offset_y: f32,
    pub scale: f32,
}

impl ScreenTransform {
    #[inline]
    pub fn world_to_screen(&self, world: (f64, f64)) -> iced::Point {
        iced::Point::new(
            world.0 as f32 * self.scale + self.offset_x,
            world.1 as f32 * self.scale + self.offset_y,
        )
    }
}

pub trait SchematicSheetExt {
    fn symbol_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)>;
    fn symbol_reference_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)>;
    fn symbol_value_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)>;
}

impl SchematicSheetExt for SchematicSheet {
    fn symbol_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)> {
        self.symbols
            .iter()
            .find(|s| s.uuid == uuid)
            .map(|s| (s.position.x, s.position.y))
    }

    fn symbol_reference_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)> {
        self.symbols
            .iter()
            .find(|s| s.uuid == uuid)
            .and_then(|s| s.ref_text.as_ref())
            .map(|t| (t.position.x, t.position.y))
    }

    fn symbol_value_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)> {
        self.symbols
            .iter()
            .find(|s| s.uuid == uuid)
            .and_then(|s| s.val_text.as_ref())
            .map(|t| (t.position.x, t.position.y))
    }
}

#[derive(Debug, Default, Clone)]
pub struct SchematicRenderCache {
    sheet: Option<SchematicSheet>,
    preview: Option<SchematicSheet>,
}

impl SchematicRenderCache {
    pub fn from_sheet(sheet: &SchematicSheet) -> Self {
        Self {
            sheet: Some(sheet.clone()),
            preview: None,
        }
    }

    pub fn update_from_sheet(&mut self, sheet: &SchematicSheet, _invalidation: RenderInvalidation) {
        self.sheet = Some(sheet.clone());
        self.preview = None;
    }

    pub fn snapshot(&self) -> &SchematicSheet {
        self.sheet
            .as_ref()
            .expect("SchematicRenderCache::snapshot called before initialization")
    }

    pub fn prepared_preview(&self) -> Option<&SchematicSheet> {
        self.preview.as_ref()
    }
}

#[inline]
pub fn instance_transform(symbol: &Symbol, local_point: &Point) -> (f64, f64) {
    let x = local_point.x;
    let y = -local_point.y;
    let rad = -symbol.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let mut rx = x * cos - y * sin;
    let mut ry = x * sin + y * cos;
    if symbol.mirror_y {
        rx = -rx;
    }
    if symbol.mirror_x {
        ry = -ry;
    }
    (rx + symbol.position.x, ry + symbol.position.y)
}

pub fn draw_power_port_preview(
    frame: &mut canvas::Frame,
    symbol: &Symbol,
    transform: &ScreenTransform,
    color: Color,
) {
    let center = transform.world_to_screen((symbol.position.x, symbol.position.y));
    let half_w = 7.0_f32;
    let half_h = 4.0_f32;
    let body = canvas::Path::new(|builder| {
        builder.move_to(iced::Point::new(center.x - half_w, center.y + half_h));
        builder.line_to(iced::Point::new(center.x - half_w, center.y - half_h));
        builder.line_to(iced::Point::new(center.x + half_w, center.y));
        builder.close();
    });
    frame.fill(&body, Color { a: color.a * 0.24, ..color });
    frame.stroke(
        &body,
        canvas::Stroke::default().with_width(1.2).with_color(color),
    );

    if !symbol.reference.is_empty() {
        frame.fill_text(canvas::Text {
            content: symbol.reference.clone(),
            position: iced::Point::new(center.x + 9.0, center.y - 4.0),
            color,
            size: iced::Pixels(11.0),
            ..canvas::Text::default()
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_schematic(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    wire_color_overrides: Option<&HashMap<uuid::Uuid, ThemeColor>>,
) {
    draw_wires(
        frame,
        snapshot,
        transform,
        colors,
        bounds,
        focus_set,
        wire_color_overrides,
    );
    draw_buses(frame, snapshot, transform, colors, bounds, focus_set);
    draw_junctions(frame, snapshot, transform, colors, bounds, focus_set);
    draw_no_connects(frame, snapshot, transform, colors, bounds, focus_set);
    draw_symbols(frame, snapshot, transform, colors, bounds, focus_set);
    draw_child_sheets(frame, snapshot, transform, colors, bounds, focus_set);
    draw_drawings(frame, snapshot, transform, colors, bounds, focus_set);
    draw_labels(frame, snapshot, transform, colors, bounds, focus_set);
    draw_text_notes(frame, snapshot, transform, colors, bounds, focus_set);
}

pub mod text {
    use super::*;

    pub fn expand_char_escapes(text: &str) -> String {
        text.to_string()
    }

    pub fn escape_for_standard(text: &str) -> String {
        text.to_string()
    }

    pub fn draw_text_note_preview(
        frame: &mut canvas::Frame,
        note: &TextNote,
        transform: &ScreenTransform,
        color: Color,
    ) {
        let pos = transform.world_to_screen((note.position.x, note.position.y));
        let size_px = text_size_px(note.font_size, transform.scale);
        draw_rotated_text(
            frame,
            &note.text,
            pos,
            note.rotation,
            size_px,
            color,
            note.justify_h,
            note.justify_v,
        );
    }
}

pub mod label {
    use super::*;

    pub fn draw_label_preview(
        frame: &mut canvas::Frame,
        label: &Label,
        transform: &ScreenTransform,
        stroke_color: Color,
        fill_color: Color,
    ) {
        super::draw_label_impl(frame, label, transform, stroke_color, Some(fill_color));
    }
}

pub mod selection {
    use super::*;

    pub fn draw_selection_overlay(
        frame: &mut canvas::Frame,
        snapshot: &SchematicRenderSnapshot,
        selected: &[SelectedItem],
        transform: &ScreenTransform,
    ) {
        let stroke = Color::from_rgba(0.95, 0.95, 1.0, 0.95);
        let fill = Color::from_rgba(0.65, 0.72, 1.0, 0.12);

        for item in selected {
            if let Some(bbox) = item_aabb(snapshot, item) {
                let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
                let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
                let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
                let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());

                if size.width <= 2.0 && size.height <= 2.0 {
                    let marker = canvas::Path::circle(rect_min, 5.5);
                    frame.fill(&marker, fill);
                    frame.stroke(
                        &marker,
                        canvas::Stroke::default().with_width(1.4).with_color(stroke),
                    );
                } else {
                    let path = canvas::Path::rectangle(rect_min, size);
                    frame.fill(&path, fill);
                    frame.stroke(
                        &path,
                        canvas::Stroke::default().with_width(1.2).with_color(stroke),
                    );
                }
            }
        }
    }
}

pub mod hit_test {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum SelectionMode {
        #[default]
        Inside,
        Touching,
        Single,
    }

    pub fn hit_test(
        snapshot: &SchematicRenderSnapshot,
        world_x: f64,
        world_y: f64,
    ) -> Option<SelectedItem> {
        let point = Point::new(world_x, world_y);
        hit_test_items(snapshot, point)
            .into_iter()
            .next()
    }

    pub fn hit_test_polygon(
        snapshot: &SchematicRenderSnapshot,
        polygon: &[(f64, f64)],
    ) -> Vec<SelectedItem> {
        if polygon.len() < 3 {
            return Vec::new();
        }

        let mut out = Vec::new();
        for item in collect_item_bounds(snapshot) {
            if point_in_polygon((item.anchor.x, item.anchor.y), polygon) {
                out.push(item.item);
            }
        }
        out
    }

    pub fn hit_test_rect_mode(
        snapshot: &SchematicRenderSnapshot,
        rect: &Aabb,
        mode: SelectionMode,
    ) -> Vec<SelectedItem> {
        let mut out = Vec::new();

        for item in collect_item_bounds(snapshot) {
            let inside = rect.contains(item.bbox.min_x, item.bbox.min_y)
                && rect.contains(item.bbox.max_x, item.bbox.max_y);
            let touching = aabb_overlaps(rect, &item.bbox);

            let keep = match mode {
                SelectionMode::Inside | SelectionMode::Single => inside,
                SelectionMode::Touching => touching,
            };

            if keep {
                out.push(item.item);
            }
        }

        out
    }

    fn hit_test_items(snapshot: &SchematicRenderSnapshot, point: Point) -> Vec<SelectedItem> {
        let mut out = Vec::new();

        for item in collect_item_bounds(snapshot).into_iter().rev() {
            let hit = match item.item.kind {
                SelectedKind::Wire => hit_wire(snapshot, item.item.uuid, point),
                SelectedKind::Bus => hit_bus(snapshot, item.item.uuid, point),
                _ => item.bbox.expand(0.25).contains(point.x, point.y),
            };
            if hit {
                out.push(item.item);
            }
        }

        out
    }

    fn hit_wire(snapshot: &SchematicRenderSnapshot, uuid: uuid::Uuid, point: Point) -> bool {
        snapshot
            .wires
            .iter()
            .find(|wire| wire.uuid == uuid)
            .is_some_and(|wire| {
                let tolerance = wire.stroke_width.max(0.15).max(0.25);
                point_to_segment_distance(point, wire.start, wire.end) <= tolerance
            })
    }

    fn hit_bus(snapshot: &SchematicRenderSnapshot, uuid: uuid::Uuid, point: Point) -> bool {
        snapshot
            .buses
            .iter()
            .find(|bus| bus.uuid == uuid)
            .is_some_and(|bus| point_to_segment_distance(point, bus.start, bus.end) <= 0.55)
    }
}

#[derive(Debug, Clone, Copy)]
struct ItemBound {
    item: SelectedItem,
    bbox: Aabb,
    anchor: Point,
}

fn collect_item_bounds(snapshot: &SchematicRenderSnapshot) -> Vec<ItemBound> {
    let mut out = Vec::new();

    for symbol in &snapshot.symbols {
        let item = SelectedItem::new(symbol.uuid, SelectedKind::Symbol);
        out.push(ItemBound {
            item,
            bbox: symbol_body_aabb(symbol),
            anchor: symbol.position,
        });

        if let Some(ref_text) = symbol.ref_text.as_ref() {
            out.push(ItemBound {
                item: SelectedItem::new(symbol.uuid, SelectedKind::SymbolRefField),
                bbox: text_prop_aabb(symbol, &symbol.reference, ref_text),
                anchor: ref_text.position,
            });
        }
        if let Some(val_text) = symbol.val_text.as_ref() {
            out.push(ItemBound {
                item: SelectedItem::new(symbol.uuid, SelectedKind::SymbolValField),
                bbox: text_prop_aabb(symbol, &symbol.value, val_text),
                anchor: val_text.position,
            });
        }
    }

    for wire in &snapshot.wires {
        out.push(ItemBound {
            item: SelectedItem::new(wire.uuid, SelectedKind::Wire),
            bbox: Aabb::new(wire.start.x, wire.start.y, wire.end.x, wire.end.y)
                .expand(wire.stroke_width.max(0.15)),
            anchor: Point::new((wire.start.x + wire.end.x) * 0.5, (wire.start.y + wire.end.y) * 0.5),
        });
    }

    for bus in &snapshot.buses {
        out.push(ItemBound {
            item: SelectedItem::new(bus.uuid, SelectedKind::Bus),
            bbox: Aabb::new(bus.start.x, bus.start.y, bus.end.x, bus.end.y).expand(0.45),
            anchor: Point::new((bus.start.x + bus.end.x) * 0.5, (bus.start.y + bus.end.y) * 0.5),
        });
    }

    for bus_entry in &snapshot.bus_entries {
        let end = Point::new(
            bus_entry.position.x + bus_entry.size.0,
            bus_entry.position.y + bus_entry.size.1,
        );
        out.push(ItemBound {
            item: SelectedItem::new(bus_entry.uuid, SelectedKind::BusEntry),
            bbox: Aabb::new(bus_entry.position.x, bus_entry.position.y, end.x, end.y),
            anchor: Point::new(
                (bus_entry.position.x + end.x) * 0.5,
                (bus_entry.position.y + end.y) * 0.5,
            ),
        });
    }

    for junction in &snapshot.junctions {
        out.push(ItemBound {
            item: SelectedItem::new(junction.uuid, SelectedKind::Junction),
            bbox: Aabb::new(
                junction.position.x - 0.5,
                junction.position.y - 0.5,
                junction.position.x + 0.5,
                junction.position.y + 0.5,
            ),
            anchor: junction.position,
        });
    }

    for no_connect in &snapshot.no_connects {
        out.push(ItemBound {
            item: SelectedItem::new(no_connect.uuid, SelectedKind::NoConnect),
            bbox: Aabb::new(
                no_connect.position.x - 0.5,
                no_connect.position.y - 0.5,
                no_connect.position.x + 0.5,
                no_connect.position.y + 0.5,
            ),
            anchor: no_connect.position,
        });
    }

    for label in &snapshot.labels {
        out.push(ItemBound {
            item: SelectedItem::new(label.uuid, SelectedKind::Label),
            bbox: label_aabb(label),
            anchor: label.position,
        });
    }

    for note in &snapshot.text_notes {
        out.push(ItemBound {
            item: SelectedItem::new(note.uuid, SelectedKind::TextNote),
            bbox: note_aabb(note),
            anchor: note.position,
        });
    }

    for child in &snapshot.child_sheets {
        out.push(ItemBound {
            item: SelectedItem::new(child.uuid, SelectedKind::ChildSheet),
            bbox: Aabb::new(
                child.position.x,
                child.position.y,
                child.position.x + child.size.0,
                child.position.y + child.size.1,
            ),
            anchor: Point::new(
                child.position.x + child.size.0 * 0.5,
                child.position.y + child.size.1 * 0.5,
            ),
        });

        for pin in &child.pins {
            out.push(ItemBound {
                item: SelectedItem::new(pin.uuid, SelectedKind::SheetPin),
                bbox: Aabb::new(
                    pin.position.x - 0.8,
                    pin.position.y - 0.8,
                    pin.position.x + 0.8,
                    pin.position.y + 0.8,
                ),
                anchor: pin.position,
            });
        }
    }

    for drawing in &snapshot.drawings {
        let uuid = match drawing {
            SchDrawing::Line { uuid, .. }
            | SchDrawing::Rect { uuid, .. }
            | SchDrawing::Circle { uuid, .. }
            | SchDrawing::Arc { uuid, .. }
            | SchDrawing::Polyline { uuid, .. } => *uuid,
        };
        let bbox = drawing_aabb(drawing);
        out.push(ItemBound {
            item: SelectedItem::new(uuid, SelectedKind::Drawing),
            bbox,
            anchor: Point::new((bbox.min_x + bbox.max_x) * 0.5, (bbox.min_y + bbox.max_y) * 0.5),
        });
    }

    out
}

fn item_aabb(snapshot: &SchematicRenderSnapshot, item: &SelectedItem) -> Option<Aabb> {
    collect_item_bounds(snapshot)
        .into_iter()
        .find(|entry| entry.item == *item)
        .map(|entry| entry.bbox)
}

fn draw_wires(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    wire_color_overrides: Option<&HashMap<uuid::Uuid, ThemeColor>>,
) {
    for wire in &snapshot.wires {
        let p0 = transform.world_to_screen((wire.start.x, wire.start.y));
        let p1 = transform.world_to_screen((wire.end.x, wire.end.y));
        if !line_visible(p0, p1, bounds) {
            continue;
        }

        let base_color = wire_color_overrides
            .and_then(|map| map.get(&wire.uuid))
            .map(to_iced)
            .unwrap_or_else(|| to_iced(&colors.wire));
        let color = focus_color(base_color, focus_set, wire.uuid);
        let width = mm_to_px(wire.stroke_width.max(0.15), transform.scale);
        let path = canvas::Path::line(p0, p1);
        frame.stroke(
            &path,
            canvas::Stroke::default().with_width(width).with_color(color),
        );
    }
}

fn draw_buses(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for bus in &snapshot.buses {
        let p0 = transform.world_to_screen((bus.start.x, bus.start.y));
        let p1 = transform.world_to_screen((bus.end.x, bus.end.y));
        if !line_visible(p0, p1, bounds) {
            continue;
        }
        let color = focus_color(to_iced(&colors.bus), focus_set, bus.uuid);
        let path = canvas::Path::line(p0, p1);
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_width(mm_to_px(0.45, transform.scale))
                .with_color(color),
        );
    }
}

fn draw_junctions(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for junction in &snapshot.junctions {
        let center = transform.world_to_screen((junction.position.x, junction.position.y));
        if !point_visible(center, bounds, 6.0) {
            continue;
        }
        let color = focus_color(to_iced(&colors.junction), focus_set, junction.uuid);
        let radius_mm = (junction.diameter * 0.5).max(0.35);
        let circle = canvas::Path::circle(center, mm_to_px(radius_mm, transform.scale));
        frame.fill(&circle, color);
    }
}

fn draw_no_connects(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for item in &snapshot.no_connects {
        let center = transform.world_to_screen((item.position.x, item.position.y));
        if !point_visible(center, bounds, 10.0) {
            continue;
        }
        let color = focus_color(to_iced(&colors.body), focus_set, item.uuid);
        let len = mm_to_px(0.7, transform.scale).max(3.0);
        let a = canvas::Path::line(
            iced::Point::new(center.x - len, center.y - len),
            iced::Point::new(center.x + len, center.y + len),
        );
        let b = canvas::Path::line(
            iced::Point::new(center.x - len, center.y + len),
            iced::Point::new(center.x + len, center.y - len),
        );
        let stroke = canvas::Stroke::default().with_width(1.2).with_color(color);
        frame.stroke(&a, stroke);
        frame.stroke(&b, stroke);
    }
}

fn draw_symbols(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for symbol in &snapshot.symbols {
        let bbox = symbol_body_aabb(symbol);
        let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
        let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }

        let stroke_color = focus_color(to_iced(&colors.body), focus_set, symbol.uuid);
        let fill_color = focus_color(to_iced(&colors.body_fill), focus_set, symbol.uuid);
        let rect = canvas::Path::rectangle(rect_min, size);
        frame.fill(&rect, fill_color);
        frame.stroke(
            &rect,
            canvas::Stroke::default().with_width(1.1).with_color(stroke_color),
        );

        if !symbol.reference.is_empty() {
            let p = transform.world_to_screen((symbol.position.x, symbol.position.y - 3.5));
            draw_rotated_text(
                frame,
                &symbol.reference,
                p,
                symbol.rotation,
                text_size_px(1.05, transform.scale),
                stroke_color,
                HAlign::Center,
                VAlign::Bottom,
            );
        }
        if !symbol.value.is_empty() {
            let p = transform.world_to_screen((symbol.position.x, symbol.position.y + 3.6));
            draw_rotated_text(
                frame,
                &symbol.value,
                p,
                symbol.rotation,
                text_size_px(1.05, transform.scale),
                focus_color(to_iced(&colors.value), focus_set, symbol.uuid),
                HAlign::Center,
                VAlign::Top,
            );
        }
    }
}

fn draw_child_sheets(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for sheet in &snapshot.child_sheets {
        let min = transform.world_to_screen((sheet.position.x, sheet.position.y));
        let max = transform.world_to_screen((sheet.position.x + sheet.size.0, sheet.position.y + sheet.size.1));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }
        let color = focus_color(to_iced(&colors.global_label), focus_set, sheet.uuid);
        let rect = canvas::Path::rectangle(rect_min, size);
        frame.stroke(
            &rect,
            canvas::Stroke::default().with_width(1.0).with_color(color),
        );
        draw_rotated_text(
            frame,
            &sheet.name,
            iced::Point::new(rect_min.x + 6.0, rect_min.y + 6.0),
            0.0,
            text_size_px(1.05, transform.scale),
            color,
            HAlign::Left,
            VAlign::Top,
        );

        for pin in &sheet.pins {
            let center = transform.world_to_screen((pin.position.x, pin.position.y));
            let mark = canvas::Path::circle(center, 3.0);
            frame.fill(&mark, Color { a: 0.3, ..color });
        }
    }
}

fn draw_drawings(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for drawing in &snapshot.drawings {
        let uuid = match drawing {
            SchDrawing::Line { uuid, .. }
            | SchDrawing::Rect { uuid, .. }
            | SchDrawing::Circle { uuid, .. }
            | SchDrawing::Arc { uuid, .. }
            | SchDrawing::Polyline { uuid, .. } => *uuid,
        };
        let bbox = drawing_aabb(drawing);
        let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
        let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }

        let base_color = focus_color(to_iced(&colors.body), focus_set, uuid);

        match drawing {
            SchDrawing::Line {
                start,
                end,
                width,
                stroke_color,
                ..
            } => {
                let stroke = resolve_stroke_color(stroke_color, base_color);
                let path = canvas::Path::line(
                    transform.world_to_screen((start.x, start.y)),
                    transform.world_to_screen((end.x, end.y)),
                );
                frame.stroke(
                    &path,
                    canvas::Stroke::default()
                        .with_width(mm_to_px(width.max(0.15), transform.scale))
                        .with_color(stroke),
                );
            }
            SchDrawing::Rect {
                start,
                end,
                width,
                fill,
                stroke_color,
                ..
            } => {
                let p0 = transform.world_to_screen((start.x, start.y));
                let p1 = transform.world_to_screen((end.x, end.y));
                let min = iced::Point::new(p0.x.min(p1.x), p0.y.min(p1.y));
                let size = iced::Size::new((p1.x - p0.x).abs(), (p1.y - p0.y).abs());
                let path = canvas::Path::rectangle(min, size);
                if let Some(fill_color) = fill_color_for(*fill, stroke_color, colors) {
                    frame.fill(&path, fill_color);
                }
                frame.stroke(
                    &path,
                    canvas::Stroke::default()
                        .with_width(mm_to_px(width.max(0.15), transform.scale))
                        .with_color(resolve_stroke_color(stroke_color, base_color)),
                );
            }
            SchDrawing::Circle {
                center,
                radius,
                width,
                fill,
                stroke_color,
                ..
            } => {
                let c = transform.world_to_screen((center.x, center.y));
                let path = canvas::Path::circle(c, mm_to_px(*radius, transform.scale).max(0.7));
                if let Some(fill_color) = fill_color_for(*fill, stroke_color, colors) {
                    frame.fill(&path, fill_color);
                }
                frame.stroke(
                    &path,
                    canvas::Stroke::default()
                        .with_width(mm_to_px(width.max(0.15), transform.scale))
                        .with_color(resolve_stroke_color(stroke_color, base_color)),
                );
            }
            SchDrawing::Arc {
                start,
                mid,
                end,
                width,
                stroke_color,
                ..
            } => {
                if let Some((cx, cy, r)) = circumcircle(
                    (start.x, start.y),
                    (mid.x, mid.y),
                    (end.x, end.y),
                ) {
                    let center = transform.world_to_screen((cx, cy));
                    let a0 = (start.y - cy).atan2(start.x - cx);
                    let am = (mid.y - cy).atan2(mid.x - cx);
                    let a1 = (end.y - cy).atan2(end.x - cx);
                    let (start_angle, end_angle) = if arc_sweeps_through_mid(a0, am, a1) {
                        (a0, a1)
                    } else {
                        (a1, a0)
                    };
                    let path = canvas::Path::new(|builder| {
                        builder.arc(canvas::path::Arc {
                            center,
                            radius: mm_to_px(r, transform.scale).max(0.8),
                            start_angle: iced::Radians(start_angle as f32),
                            end_angle: iced::Radians(end_angle as f32),
                        });
                    });
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_width(mm_to_px(width.max(0.15), transform.scale))
                            .with_color(resolve_stroke_color(stroke_color, base_color)),
                    );
                } else {
                    let p0 = transform.world_to_screen((start.x, start.y));
                    let p1 = transform.world_to_screen((mid.x, mid.y));
                    let p2 = transform.world_to_screen((end.x, end.y));
                    let path = canvas::Path::new(|builder| {
                        builder.move_to(p0);
                        builder.line_to(p1);
                        builder.line_to(p2);
                    });
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_width(mm_to_px(width.max(0.15), transform.scale))
                            .with_color(resolve_stroke_color(stroke_color, base_color)),
                    );
                }
            }
            SchDrawing::Polyline {
                points,
                width,
                fill,
                stroke_color,
                ..
            } => {
                if points.len() < 2 {
                    continue;
                }
                let path = canvas::Path::new(|builder| {
                    let mut it = points.iter();
                    if let Some(first) = it.next() {
                        builder.move_to(transform.world_to_screen((first.x, first.y)));
                    }
                    for point in it {
                        builder.line_to(transform.world_to_screen((point.x, point.y)));
                    }
                    if !matches!(fill, FillType::None) {
                        builder.close();
                    }
                });
                if let Some(fill_color) = fill_color_for(*fill, stroke_color, colors) {
                    frame.fill(&path, fill_color);
                }
                frame.stroke(
                    &path,
                    canvas::Stroke::default()
                        .with_width(mm_to_px(width.max(0.15), transform.scale))
                        .with_color(resolve_stroke_color(stroke_color, base_color)),
                );
            }
        }
    }
}

fn draw_labels(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for label in &snapshot.labels {
        let screen = transform.world_to_screen((label.position.x, label.position.y));
        if !point_visible(screen, bounds, 22.0) {
            continue;
        }
        let color = focus_color(label_color(label, colors), focus_set, label.uuid);
        draw_label_impl(frame, label, transform, color, None);
    }
}

fn draw_text_notes(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
) {
    for note in &snapshot.text_notes {
        let pos = transform.world_to_screen((note.position.x, note.position.y));
        if !point_visible(pos, bounds, 28.0) {
            continue;
        }
        let color = focus_color(to_iced(&colors.value), focus_set, note.uuid);
        let size_px = text_size_px(note.font_size, transform.scale);
        draw_rotated_text(
            frame,
            &note.text,
            pos,
            note.rotation,
            size_px,
            color,
            note.justify_h,
            note.justify_v,
        );
    }
}

fn draw_label_impl(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    stroke_color: Color,
    fill_override: Option<Color>,
) {
    let pos = transform.world_to_screen((label.position.x, label.position.y));
    let size_px = text_size_px(label.font_size, transform.scale);

    if matches!(label.label_type, LabelType::Global | LabelType::Hierarchical) {
        let glyph_w = (label.text.chars().count().max(1) as f32) * (size_px * 0.58);
        let half_h = size_px * 0.62;
        let point = size_px * 0.52;
        let w = glyph_w + size_px * 0.65;
        let local = [
            (-point, 0.0_f32),
            (0.0, -half_h),
            (w, -half_h),
            (w, half_h),
            (0.0, half_h),
        ];

        let rad = (label.rotation as f32).to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let verts: Vec<iced::Point> = local
            .iter()
            .map(|(x, y)| iced::Point::new(pos.x + x * cos - y * sin, pos.y + x * sin + y * cos))
            .collect();

        let path = canvas::Path::new(|builder| {
            builder.move_to(verts[0]);
            for vertex in &verts[1..] {
                builder.line_to(*vertex);
            }
            builder.close();
        });
        if let Some(fill_color) = fill_override {
            frame.fill(&path, fill_color);
        }
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_width(1.0)
                .with_color(stroke_color),
        );

        draw_rotated_text(
            frame,
            &label.text,
            pos,
            label.rotation,
            size_px,
            stroke_color,
            HAlign::Center,
            VAlign::Center,
        );
    } else {
        draw_rotated_text(
            frame,
            &label.text,
            pos,
            label.rotation,
            size_px,
            stroke_color,
            label.justify,
            label.justify_v,
        );
    }
}

fn label_color(label: &Label, colors: &CanvasColors) -> Color {
    match label.label_type {
        LabelType::Net => to_iced(&colors.net_label),
        LabelType::Global => to_iced(&colors.global_label),
        LabelType::Hierarchical => to_iced(&colors.hier_label),
        LabelType::Power => to_iced(&colors.power),
    }
}

fn symbol_body_aabb(symbol: &Symbol) -> Aabb {
    let half_w = 4.0;
    let half_h = 2.8;
    Aabb::new(
        symbol.position.x - half_w,
        symbol.position.y - half_h,
        symbol.position.x + half_w,
        symbol.position.y + half_h,
    )
}

fn text_prop_aabb(symbol: &Symbol, text: &str, prop: &TextProp) -> Aabb {
    let chars = text.chars().count().max(1) as f64;
    let h = prop.font_size.max(signex_types::schematic::SCHEMATIC_TEXT_MM);
    let w = h * 0.6 * chars;
    let (x, y) = instance_transform(symbol, &prop.position);
    Aabb::new(x - w * 0.5, y - h * 0.5, x + w * 0.5, y + h * 0.5)
}

fn note_aabb(note: &TextNote) -> Aabb {
    let h = note.font_size.max(signex_types::schematic::SCHEMATIC_TEXT_MM);
    let w = h * 0.6 * note.text.chars().count().max(1) as f64;
    Aabb::new(
        note.position.x - w * 0.5,
        note.position.y - h * 0.5,
        note.position.x + w * 0.5,
        note.position.y + h * 0.5,
    )
}

fn label_aabb(label: &Label) -> Aabb {
    let h = label.font_size.max(signex_types::schematic::SCHEMATIC_TEXT_MM);
    let mut w = h * 0.6 * label.text.chars().count().max(1) as f64;
    if matches!(label.label_type, LabelType::Global | LabelType::Hierarchical) {
        w += h * 1.2;
    }
    Aabb::new(
        label.position.x - w * 0.5,
        label.position.y - h,
        label.position.x + w * 0.5,
        label.position.y + h,
    )
}

fn drawing_aabb(drawing: &SchDrawing) -> Aabb {
    match drawing {
        SchDrawing::Line { start, end, .. } | SchDrawing::Rect { start, end, .. } => {
            Aabb::new(start.x, start.y, end.x, end.y)
        }
        SchDrawing::Circle { center, radius, .. } => Aabb::new(
            center.x - radius,
            center.y - radius,
            center.x + radius,
            center.y + radius,
        ),
        SchDrawing::Arc { start, mid, end, .. } => {
            if let Some((cx, cy, r)) = circumcircle((start.x, start.y), (mid.x, mid.y), (end.x, end.y)) {
                Aabb::new(cx - r, cy - r, cx + r, cy + r)
            } else {
                Aabb::new(start.x, start.y, end.x, end.y).union(&Aabb::new(mid.x, mid.y, mid.x, mid.y))
            }
        }
        SchDrawing::Polyline { points, .. } => {
            if let Some(first) = points.first() {
                let mut bbox = Aabb::new(first.x, first.y, first.x, first.y);
                for point in points.iter().skip(1) {
                    bbox = bbox.union(&Aabb::new(point.x, point.y, point.x, point.y));
                }
                bbox
            } else {
                Aabb::new(0.0, 0.0, 0.0, 0.0)
            }
        }
    }
}

fn point_to_segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len2 = dx * dx + dy * dy;
    if len2 <= f64::EPSILON {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    let px = a.x + t * dx;
    let py = a.y + t * dy;
    ((p.x - px).powi(2) + (p.y - py).powi(2)).sqrt()
}

fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    let (x, y) = point;
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];
        let intersects = ((yi > y) != (yj > y))
            && (x < (xj - xi) * (y - yi) / ((yj - yi).abs().max(1e-9) * (if yj >= yi { 1.0 } else { -1.0 })) + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn text_size_px(font_size_mm: f64, zoom: f32) -> f32 {
    let size_mm = font_size_mm.max(signex_types::schematic::SCHEMATIC_TEXT_MM);
    let em_mm = size_mm / 0.72;
    (em_mm * zoom as f64).clamp(6.0, 64.0) as f32
}

fn mm_to_px(mm: f64, scale: f32) -> f32 {
    (mm.max(0.0) as f32 * scale).max(0.6)
}

fn to_iced(color: &ThemeColor) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0)
}

fn focus_color(base: Color, focus_set: Option<&HashSet<uuid::Uuid>>, uuid: uuid::Uuid) -> Color {
    if let Some(set) = focus_set
        && !set.contains(&uuid)
    {
        return Color {
            a: (base.a * 0.26).clamp(0.0, 1.0),
            ..base
        };
    }
    base
}

fn aabb_overlaps(a: &Aabb, b: &Aabb) -> bool {
    !(a.max_x < b.min_x || a.min_x > b.max_x || a.max_y < b.min_y || a.min_y > b.max_y)
}

fn line_visible(p0: iced::Point, p1: iced::Point, bounds: Rectangle) -> bool {
    let min_x = p0.x.min(p1.x);
    let max_x = p0.x.max(p1.x);
    let min_y = p0.y.min(p1.y);
    let max_y = p0.y.max(p1.y);
    !(max_x < -8.0
        || max_y < -8.0
        || min_x > bounds.width + 8.0
        || min_y > bounds.height + 8.0)
}

fn rect_visible(min: iced::Point, size: iced::Size, bounds: Rectangle) -> bool {
    !(min.x + size.width < -8.0
        || min.y + size.height < -8.0
        || min.x > bounds.width + 8.0
        || min.y > bounds.height + 8.0)
}

fn point_visible(p: iced::Point, bounds: Rectangle, pad: f32) -> bool {
    p.x >= -pad && p.y >= -pad && p.x <= bounds.width + pad && p.y <= bounds.height + pad
}

fn resolve_stroke_color(stroke_color: &Option<signex_types::schematic::StrokeColor>, fallback: Color) -> Color {
    stroke_color
        .map(|color| Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0))
        .unwrap_or(fallback)
}

fn fill_color_for(
    fill: FillType,
    stroke_color: &Option<signex_types::schematic::StrokeColor>,
    colors: &CanvasColors,
) -> Option<Color> {
    match fill {
        FillType::None => None,
        FillType::Outline => Some(resolve_stroke_color(stroke_color, to_iced(&colors.body))),
        FillType::Background => Some(to_iced(&colors.body_fill)),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rotated_text(
    frame: &mut canvas::Frame,
    text: &str,
    position: iced::Point,
    rotation_deg: f64,
    size_px: f32,
    color: Color,
    h_align: HAlign,
    v_align: VAlign,
) {
    if text.is_empty() {
        return;
    }

    let align_x = match h_align {
        HAlign::Left => advanced_text::Alignment::Left,
        HAlign::Center => advanced_text::Alignment::Center,
        HAlign::Right => advanced_text::Alignment::Right,
    };
    let align_y = match v_align {
        VAlign::Top => alignment::Vertical::Top,
        VAlign::Center => alignment::Vertical::Center,
        VAlign::Bottom => alignment::Vertical::Bottom,
    };

    let base = canvas::Text {
        content: text.to_string(),
        position: iced::Point::ORIGIN,
        color,
        size: iced::Pixels(size_px),
        align_x,
        align_y,
        ..canvas::Text::default()
    };

    let rad = rotation_deg.to_radians() as f32;
    if rad.abs() < f32::EPSILON {
        let mut placed = base;
        placed.position = position;
        frame.fill_text(placed);
        return;
    }

    frame.with_save(|inner| {
        inner.translate(Vector::new(position.x, position.y));
        inner.rotate(iced::Radians(rad));
        inner.fill_text(base);
    });
}

fn circumcircle(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64, f64)> {
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None;
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let radius = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    Some((ux, uy, radius))
}

fn arc_sweeps_through_mid(a0: f64, am: f64, a1: f64) -> bool {
    let two_pi = 2.0 * std::f64::consts::PI;
    let normalize = |a: f64| (a - a0).rem_euclid(two_pi);
    normalize(am) < normalize(a1)
}
