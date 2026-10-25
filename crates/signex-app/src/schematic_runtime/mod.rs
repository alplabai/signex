//! Local schematic runtime used by `signex-app`.
//!
//! This module keeps schematic rendering, hit-test, and overlay behavior
//! self-contained inside the app runtime contract.

use iced::widget::canvas;
use iced::{Color, Rectangle};
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::schematic::{
    ArcInput, JunctionInput, OverlayCircleInput, OverlayInputs, OverlayLineInput,
    OverlayPolygonInput, PolygonInput, SchematicRenderer, SchematicSnapshot as RendererSnapshot,
    TextInput, ViewRenderer, WireInput,
};
use signex_renderer::theme::ResolvedTheme;
use signex_types::schematic::{
    Aabb, FillType, HAlign, Label, LabelType, Point, SchDrawing, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, TextNote, TextProp, VAlign, circumcircle, field_effective_style,
};
use signex_types::theme::{CanvasColors, Color as ThemeColor};
use std::collections::{HashMap, HashSet};

pub mod hit_test;
pub mod label;
pub mod overlay;
pub mod selection;
mod snapshot;
pub mod text;

pub use snapshot::build_renderer_snapshot;

pub type SchematicRenderSnapshot = SchematicSheet;

/// The family a text item draws in: its own, else the sheet default, checked
/// against the installed fonts (`None` is the bundled canvas default).
fn resolve_family(item: Option<&str>, document: Option<&str>) -> Option<std::sync::Arc<str>> {
    crate::fonts::resolve::resolve_installed(item, document)
}

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
    pub const NO_ERC_DIRECTIVES: Self = Self(1 << 12);
    pub const FULL: Self = Self(u32::MAX);

    /// True when any bit of `mask` is also set in `self`.
    pub fn intersects(self, mask: Self) -> bool {
        self.0 & mask.0 != 0
    }
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
    /// Engine `revision()` the snapshot was last brought level with; `0`
    /// when unknown, which never matches a real revision.
    revision: u64,
}

impl SchematicRenderCache {
    pub fn from_sheet(sheet: &SchematicSheet) -> Self {
        Self::from_sheet_at(sheet, 0)
    }

    /// Snapshot `sheet` as it stood at engine revision `revision`.
    pub fn from_sheet_at(sheet: &SchematicSheet, revision: u64) -> Self {
        Self {
            sheet: Some(sheet.clone()),
            preview: None,
            revision,
        }
    }

    /// Refresh the cached snapshot from `sheet`. Only the collections named by
    /// `invalidation` are re-cloned, and only when the snapshot matched the
    /// engine just before the edit (`base_revision`, the engine's
    /// `base_revision()`). `FULL`, an empty cache, a different document
    /// (uuid mismatch) or a stale snapshot (an engine edit that never
    /// refreshed this slot) re-clones everything. `revision` is the engine's
    /// current `revision()`.
    pub fn update_from_sheet(
        &mut self,
        sheet: &SchematicSheet,
        invalidation: RenderInvalidation,
        base_revision: u64,
        revision: u64,
    ) {
        self.preview = None;
        let in_step = self.revision == base_revision;
        match self.sheet.as_mut() {
            Some(cached)
                if in_step
                    && invalidation != RenderInvalidation::FULL
                    && cached.uuid == sheet.uuid =>
            {
                copy_invalidated(cached, sheet, invalidation);
            }
            _ => self.sheet = Some(sheet.clone()),
        }
        self.revision = revision;
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

/// Copy the sheet collections selected by `invalidation` from `src` into `dst`.
/// `unknown`, `version`, `generator` and `generator_version` are skipped: they are not drawn.
fn copy_invalidated(
    dst: &mut SchematicSheet,
    src: &SchematicSheet,
    invalidation: RenderInvalidation,
) {
    use RenderInvalidation as R;
    if invalidation.intersects(R::SYMBOLS) {
        dst.symbols.clone_from(&src.symbols);
    }
    if invalidation.intersects(R::WIRES) {
        dst.wires.clone_from(&src.wires);
    }
    if invalidation.intersects(R::LABELS) {
        dst.labels.clone_from(&src.labels);
    }
    if invalidation.intersects(R::TEXT_NOTES) {
        dst.text_notes.clone_from(&src.text_notes);
    }
    if invalidation.intersects(R::BUSES) {
        dst.buses.clone_from(&src.buses);
    }
    if invalidation.intersects(R::BUS_ENTRIES) {
        dst.bus_entries.clone_from(&src.bus_entries);
    }
    if invalidation.intersects(R::JUNCTIONS) {
        dst.junctions.clone_from(&src.junctions);
    }
    if invalidation.intersects(R::NO_CONNECTS) {
        dst.no_connects.clone_from(&src.no_connects);
    }
    if invalidation.intersects(R::NO_ERC_DIRECTIVES) {
        dst.no_erc_directives.clone_from(&src.no_erc_directives);
    }
    if invalidation.intersects(R::CHILD_SHEETS) {
        dst.child_sheets.clone_from(&src.child_sheets);
    }
    if invalidation.intersects(R::DRAWINGS) {
        dst.drawings.clone_from(&src.drawings);
    }
    if invalidation.intersects(R::LIB_SYMBOLS) {
        dst.lib_symbols.clone_from(&src.lib_symbols);
    }
    if invalidation.intersects(R::PAPER) {
        dst.paper_size.clone_from(&src.paper_size);
        dst.title_block.clone_from(&src.title_block);
        dst.root_sheet_page.clone_from(&src.root_sheet_page);
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
    colors: &CanvasColors,
) {
    let half_w = screen_px_to_world_mm(7.0, transform.scale) as f32;
    let half_h = screen_px_to_world_mm(4.0, transform.scale) as f32;
    let cx = symbol.position.x as f32;
    let cy = symbol.position.y as f32;

    let mut parameter_texts = Vec::new();
    if !symbol.reference.is_empty() {
        parameter_texts.push(TextInput {
            content: symbol.reference.clone(),
            position: [
                cx + screen_px_to_world_mm(9.0, transform.scale) as f32,
                cy - screen_px_to_world_mm(4.0, transform.scale) as f32,
            ],
            size_mm: (11.0 * 0.72 / transform.scale.max(0.001)).max(0.1),
            color: to_rgba(color),
            bold: false,
            italic: false,
            rotation_rad: 0.0,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            family: None,
        });
    }

    let snapshot = RendererSnapshot {
        wires: Vec::new(),
        junctions: Vec::new(),
        arcs: Vec::new(),
        polygons: vec![PolygonInput {
            vertices: vec![
                [cx - half_w, cy + half_h],
                [cx - half_w, cy - half_h],
                [cx + half_w, cy],
            ],
            fill_color: to_rgba(Color {
                a: color.a * 0.24,
                ..color
            }),
            stroke_color: Some(to_rgba(color)),
            stroke_width_mm: stroke_world_mm(
                signex_types::schematic::SCHEMATIC_RENDER_POWER_PORT_STROKE_PX,
                transform.scale,
            ),
        }],
        labels: Vec::new(),
        pin_texts: Vec::new(),
        reference_value_texts: Vec::new(),
        parameter_texts,
        overlays: OverlayInputs::default(),
        erc_markers: Vec::new(),
        wire_color_overrides: HashMap::new(),
    };

    draw_renderer_snapshot(
        frame,
        &snapshot,
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::POLYGONS | DirtyFlags::TEXT,
        transform,
    );
}

pub fn render_schematic(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    wire_color_overrides: Option<&HashMap<uuid::Uuid, ThemeColor>>,
) {
    render_schematic_with_renderer(
        frame,
        snapshot,
        transform,
        colors,
        bounds,
        focus_set,
        wire_color_overrides,
    );
}

fn render_schematic_with_renderer(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    wire_color_overrides: Option<&HashMap<uuid::Uuid, ThemeColor>>,
) {
    let renderer_snapshot = build_renderer_snapshot(
        snapshot,
        transform,
        colors,
        bounds,
        focus_set,
        wire_color_overrides,
    );
    if renderer_snapshot.wires.is_empty()
        && renderer_snapshot.junctions.is_empty()
        && renderer_snapshot.arcs.is_empty()
        && renderer_snapshot.polygons.is_empty()
        && renderer_snapshot.labels.is_empty()
        && renderer_snapshot.pin_texts.is_empty()
        && renderer_snapshot.reference_value_texts.is_empty()
        && renderer_snapshot.parameter_texts.is_empty()
    {
        return;
    }

    draw_renderer_snapshot(
        frame,
        &renderer_snapshot,
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::LINES
            | DirtyFlags::CIRCLES
            | DirtyFlags::ARCS
            | DirtyFlags::POLYGONS
            | DirtyFlags::TEXT,
        transform,
    );
}

fn label_marker_polygon(
    label: &Label,
    stroke_color: Color,
    fill_color: [f32; 4],
    transform: &ScreenTransform,
) -> PolygonInput {
    PolygonInput {
        vertices: label::marker_vertices(label)
            .iter()
            .map(|&(x, y)| [x as f32, y as f32])
            .collect(),
        fill_color,
        stroke_color: Some(to_rgba(stroke_color)),
        stroke_width_mm: stroke_world_mm(
            signex_types::schematic::SCHEMATIC_RENDER_LABEL_GLYPH_STROKE_PX,
            transform.scale,
        ),
    }
}

fn renderer_id(uuid: uuid::Uuid) -> u64 {
    uuid.as_u128() as u64
}

fn draw_renderer_snapshot(
    frame: &mut canvas::Frame,
    snapshot: &RendererSnapshot,
    theme: &ResolvedTheme,
    dirty: DirtyFlags,
    transform: &ScreenTransform,
) {
    let mut scene = Scene::default();
    SchematicRenderer::build_scene(snapshot, theme, dirty, &mut scene);
    crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
        frame,
        &scene,
        |point| transform.world_to_screen((point[0] as f64, point[1] as f64)),
        crate::renderer_scene_canvas::SceneDrawOptions {
            scale_px_per_mm: transform.scale,
            min_stroke_px: signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_PX,
            text_min_px: 6.0,
            text_max_px: 64.0,
        },
    );
}

fn to_rgba(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn stroke_world_mm(base_width_px_at_100: f32, scale: f32) -> f32 {
    (stroke_px_at_zoom(base_width_px_at_100, scale) / scale.max(0.001))
        .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM as f32)
}

fn screen_px_to_world_mm(px: f32, scale: f32) -> f64 {
    (px / scale.max(0.001)) as f64
}

fn circle_vertices(center: [f64; 2], radius: f32, segments: usize) -> Vec<[f32; 2]> {
    let segment_count = segments.max(12);
    let cx = center[0] as f32;
    let cy = center[1] as f32;
    let r = radius.max(0.01);

    (0..segment_count)
        .map(|step| {
            let theta = (step as f32 / segment_count as f32) * std::f32::consts::TAU;
            [cx + theta.cos() * r, cy + theta.sin() * r]
        })
        .collect()
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

        if let Some(ref_text) = symbol.ref_text.as_ref().filter(|p| !p.hidden) {
            out.push(ItemBound {
                item: SelectedItem::new(symbol.uuid, SelectedKind::SymbolRefField),
                bbox: text_prop_aabb(&symbol.reference, ref_text, symbol),
                anchor: ref_text.position,
            });
        }
        if let Some(val_text) = symbol.val_text.as_ref().filter(|p| !p.hidden) {
            out.push(ItemBound {
                item: SelectedItem::new(symbol.uuid, SelectedKind::SymbolValField),
                bbox: text_prop_aabb(&symbol.value, val_text, symbol),
                anchor: val_text.position,
            });
        }
    }

    for wire in &snapshot.wires {
        out.push(ItemBound {
            item: SelectedItem::new(wire.uuid, SelectedKind::Wire),
            bbox: Aabb::new(wire.start.x, wire.start.y, wire.end.x, wire.end.y).expand(
                wire.stroke_width
                    .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM),
            ),
            anchor: Point::new(
                (wire.start.x + wire.end.x) * 0.5,
                (wire.start.y + wire.end.y) * 0.5,
            ),
        });
    }

    for bus in &snapshot.buses {
        out.push(ItemBound {
            item: SelectedItem::new(bus.uuid, SelectedKind::Bus),
            bbox: Aabb::new(bus.start.x, bus.start.y, bus.end.x, bus.end.y)
                .expand(signex_types::schematic::SCHEMATIC_RENDER_BUS_STROKE_MM),
            anchor: Point::new(
                (bus.start.x + bus.end.x) * 0.5,
                (bus.start.y + bus.end.y) * 0.5,
            ),
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

    for directive in &snapshot.no_erc_directives {
        out.push(ItemBound {
            item: SelectedItem::new(directive.uuid, SelectedKind::NoErcDirective),
            bbox: Aabb::new(
                directive.position.x - 0.5,
                directive.position.y - 0.5,
                directive.position.x + 0.5,
                directive.position.y + 0.5,
            ),
            anchor: directive.position,
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
            anchor: Point::new(
                (bbox.min_x + bbox.max_x) * 0.5,
                (bbox.min_y + bbox.max_y) * 0.5,
            ),
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

fn text_prop_aabb(text: &str, prop: &TextProp, symbol: &Symbol) -> Aabb {
    let (rot, h_align, v_align) = field_effective_style(prop, symbol);
    aligned_text_aabb(
        text,
        prop.font_size,
        (prop.position.x, prop.position.y),
        rot,
        h_align,
        v_align,
    )
}

/// Bounding box of `text` anchored at `anchor`. Mirrors the renderer
/// (`anchored_top_left_px`): the alignment offset is computed in the text's
/// local frame, then rotated with the text about the anchor (y-down).
fn aligned_text_aabb(
    text: &str,
    font_size: f64,
    anchor: (f64, f64),
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
) -> Aabb {
    let chars = text.chars().count().max(1) as f64;
    let h = font_size.max(signex_types::schematic::SCHEMATIC_TEXT_MM);
    let w = h * 0.6 * chars;
    let ox = match h_align {
        HAlign::Left => 0.0,
        HAlign::Center => -w * 0.5,
        HAlign::Right => -w,
    };
    let oy = match v_align {
        VAlign::Top => 0.0,
        VAlign::Center => -h * 0.5,
        VAlign::Bottom => -h,
    };
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let mut bbox: Option<Aabb> = None;
    for (lx, ly) in [(ox, oy), (ox + w, oy), (ox, oy + h), (ox + w, oy + h)] {
        let x = anchor.0 + lx * cos - ly * sin;
        let y = anchor.1 + lx * sin + ly * cos;
        bbox = Some(match bbox {
            None => Aabb::new(x, y, x, y),
            Some(b) => Aabb::new(
                b.min_x.min(x),
                b.min_y.min(y),
                b.max_x.max(x),
                b.max_y.max(y),
            ),
        });
    }
    bbox.expect("four corners")
}

fn note_aabb(note: &TextNote) -> Aabb {
    aligned_text_aabb(
        &note.text,
        note.font_size,
        (note.position.x, note.position.y),
        note.rotation,
        note.justify_h,
        note.justify_v,
    )
}

/// Hit/selection box of a label, derived from the same geometry the draw
/// path uses so the two cannot disagree: the flag outline for global and
/// hierarchical labels, the justified text box for the rest.
fn label_aabb(label: &Label) -> Aabb {
    if label::is_flag(label) {
        let mut verts = label::marker_vertices(label).into_iter();
        let (x0, y0) = verts.next().expect("marker has vertices");
        return verts.fold(Aabb::new(x0, y0, x0, y0), |b, (x, y)| {
            b.union(&Aabb::new(x, y, x, y))
        });
    }
    aligned_text_aabb(
        &label.text,
        label.font_size,
        (label.position.x, label.position.y),
        label.rotation,
        label.justify,
        label.justify_v,
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
        SchDrawing::Arc {
            start, mid, end, ..
        } => {
            if let Some((cx, cy, r)) = circumcircle(*start, *mid, *end) {
                Aabb::new(cx - r, cy - r, cx + r, cy + r)
            } else {
                Aabb::new(start.x, start.y, end.x, end.y)
                    .union(&Aabb::new(mid.x, mid.y, mid.x, mid.y))
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
    signex_sketch::geom::point_to_segment_distance((p.x, p.y), (a.x, a.y), (b.x, b.y))
}

fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    let polygon: Vec<signex_sketch::geom::Point2> = polygon.iter().map(|&p| p.into()).collect();
    signex_sketch::geom::point_in_polygon(point, &polygon)
}

fn stroke_px_at_zoom(base_width_px_at_100: f32, scale: f32) -> f32 {
    let zoom_factor = (scale / signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE).max(0.0);
    let scaled = base_width_px_at_100 * zoom_factor;
    let max_stroke = base_width_px_at_100
        * signex_types::schematic::SCHEMATIC_RENDER_STROKE_MAX_SCALE_MULTIPLIER;
    scaled.clamp(
        signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_PX,
        max_stroke,
    )
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
    !(max_x < -8.0 || max_y < -8.0 || min_x > bounds.width + 8.0 || min_y > bounds.height + 8.0)
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

/// Culls text by its extent, not just its anchor: the anchor may sit off
/// screen while a long label or flag still reaches into view. The reach is the
/// longest side of the hit box in every direction, which also covers rotation
/// and one-sided flags.
fn text_anchor_visible(
    anchor: iced::Point,
    bounds: Rectangle,
    pad: f32,
    bbox: &Aabb,
    scale: f32,
) -> bool {
    let reach = (bbox.width().max(bbox.height()) as f32) * scale;
    point_visible(anchor, bounds, pad + reach)
}

fn resolve_stroke_color(
    stroke_color: &Option<signex_types::schematic::StrokeColor>,
    fallback: Color,
) -> Color {
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

/// Does the CCW span from `a0` to `a1` contain `am`?
///
/// The rule that decides which of the two arcs through three clicked points
/// is the one the user meant. `pub` so the in-progress preview can ask the
/// same question the commit path asks — a preview that orders its endpoints
/// by a second copy of this rule can show the complementary arc to the one it
/// is about to create.
pub fn arc_sweeps_through_mid(a0: f64, am: f64, a1: f64) -> bool {
    let two_pi = 2.0 * std::f64::consts::PI;
    let normalize = |a: f64| (a - a0).rem_euclid(two_pi);
    normalize(am) < normalize(a1)
}

#[cfg(test)]
mod tests;
