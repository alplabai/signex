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
use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{
    Aabb, DrawingPlacement, FillType, HAlign, Label, LabelType, Point, SchDrawing, SchematicSheet,
    SelectedItem, SelectedKind, Symbol, TextNote, TextProp, VAlign, circumcircle,
    field_effective_style, text_aabb_with_width, text_height_mm,
};
use signex_types::theme::{CanvasColors, Color as ThemeColor};
use std::collections::{HashMap, HashSet};

mod drawings;
pub mod hit_test;
pub mod label;
pub mod lock_indicator;
pub mod overlay;
mod scene_cache;
pub mod selection;
mod snapshot;
mod symbol;
pub(crate) use symbol::symbols_without_geometry;
pub mod text;
mod text_width;
mod wire_style;

pub use scene_cache::ContentSceneCache;
pub(crate) use scene_cache::SceneInputs;
pub use snapshot::build_renderer_snapshot;
pub use wire_style::{
    WireClassStyles, WireStyling, drawn_wire_width_mm, junction_color, wire_color,
};

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
    /// Engine revision the snapshot was last brought level with (`0` when
    /// unknown). Changes with every edit, undo and redo.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

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
        // Placements are edited under the same patch (#1583).
        dst.drawing_placements.clone_from(&src.drawing_placements);
    }
    if invalidation.intersects(R::LIB_SYMBOLS) {
        dst.lib_symbols.clone_from(&src.lib_symbols);
    }
    if invalidation.intersects(R::PAPER) {
        dst.paper_size.clone_from(&src.paper_size);
        dst.title_block.clone_from(&src.title_block);
        dst.root_sheet_page.clone_from(&src.root_sheet_page);
        dst.frame = src.frame;
    }
}

pub fn draw_power_port_preview(
    frame: &mut canvas::Frame,
    symbol: &Symbol,
    transform: &ScreenTransform,
    color: Color,
    colors: &CanvasColors,
) {
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
            size_mm: (11.0 * signex_gfx::primitive::text::MM_PER_EM / transform.scale.max(0.001))
                .max(0.1),
            color: to_rgba(color),
            bold: false,
            italic: false,
            rotation_rad: 0.0,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            family: None,
        });
    }

    // The body is the placed port's own lowering, so the ghost shows the
    // shape the click will commit.
    let (primitives, bbox) = symbol::symbol_geometry_of(None, symbol);
    let mut wires = Vec::new();
    let mut arcs = Vec::new();
    let mut polygons = Vec::new();
    let style = symbol::SymbolStyle {
        id: 0,
        stroke: color,
        fill: Color {
            a: color.a * 0.24,
            ..color
        },
        pin: color,
        // The ghost keeps its own colour even for an unknown part.
        missing: color,
        scale: transform.scale,
        family: None,
    };
    let mut sink = symbol::SymbolSink {
        wires: &mut wires,
        arcs: &mut arcs,
        polygons: &mut polygons,
        texts: &mut parameter_texts,
    };
    if primitives.is_empty() {
        symbol::push_fallback_box(&bbox, &symbol.lib_id, &style, &mut sink);
    } else {
        symbol::push_primitives(&primitives, &style, &mut sink);
    }

    let snapshot = RendererSnapshot {
        wires,
        junctions: Vec::new(),
        arcs,
        polygons,
        labels: Vec::new(),
        pin_texts: Vec::new(),
        reference_value_texts: Vec::new(),
        parameter_texts,
        overlays: OverlayInputs::default(),
        erc_markers: Vec::new(),
        drawings: Vec::new(),
        drawings_front: Vec::new(),
        wire_color_overrides: HashMap::new(),
        wire_line_styles: HashMap::new(),
    };

    draw_renderer_snapshot(
        frame,
        &snapshot,
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::LINES | DirtyFlags::POLYGONS | DirtyFlags::TEXT,
        transform,
    );
}

/// The widget rectangle in the frame's own space. `canvas::Program::draw`
/// hands over the window-space layout rect (non-zero origin when a panel sits
/// left of or above the canvas) but the frame is local, so culling must use
/// the size alone.
#[must_use]
pub fn frame_local_bounds(bounds: Rectangle) -> Rectangle {
    Rectangle::with_size(bounds.size())
}

/// Designators drawn in place of `reference`, by symbol uuid (#1151).
pub type Designators = HashMap<uuid::Uuid, String>;

/// What the drawn scene shows instead of the document's own values. Each
/// map is keyed by item uuid; an item not in a map draws as stored.
#[derive(Default, Clone, Copy)]
pub struct SceneOverrides<'a> {
    /// How wires and junctions are styled: the Active Bar pen colour, then
    /// the net-class style, by wire / junction uuid.
    pub wire_styling: Option<WireStyling<'a>>,
    /// Designators of the placement the tab was opened through (#1151), by
    /// symbol uuid; only those that differ from the symbol's `reference`.
    pub designators: Option<&'a HashMap<uuid::Uuid, String>>,
    /// Resolves `${TOKEN}` special strings in text notes (#1080).
    pub special_strings: Option<&'a crate::canvas::SpecialStrings>,
}

pub fn render_schematic(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    overrides: SceneOverrides<'_>,
) {
    render_schematic_with_renderer(
        frame,
        snapshot,
        transform,
        colors,
        frame_local_bounds(bounds),
        focus_set,
        overrides,
    );
}

fn render_schematic_with_renderer(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    overrides: SceneOverrides<'_>,
) {
    if let Some(scene) =
        build_schematic_scene(snapshot, transform, colors, bounds, focus_set, overrides)
    {
        draw_built_scene(frame, &scene, transform);
    }
}

/// Lower `snapshot` to a world-space [`Scene`]; `None` when nothing is drawn.
/// Items outside `bounds` (screen space under `transform`) are culled.
pub(crate) fn build_schematic_scene(
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    overrides: SceneOverrides<'_>,
) -> Option<Scene> {
    let renderer_snapshot =
        build_renderer_snapshot(snapshot, transform, colors, bounds, focus_set, overrides);
    if renderer_snapshot.wires.is_empty()
        && renderer_snapshot.junctions.is_empty()
        && renderer_snapshot.arcs.is_empty()
        && renderer_snapshot.polygons.is_empty()
        && renderer_snapshot.drawings.is_empty()
        && renderer_snapshot.drawings_front.is_empty()
        && renderer_snapshot.labels.is_empty()
        && renderer_snapshot.pin_texts.is_empty()
        && renderer_snapshot.reference_value_texts.is_empty()
        && renderer_snapshot.parameter_texts.is_empty()
    {
        return None;
    }

    let mut scene = Scene::default();
    SchematicRenderer::build_scene(
        &renderer_snapshot,
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::LINES
            | DirtyFlags::CIRCLES
            | DirtyFlags::ARCS
            | DirtyFlags::POLYGONS
            | DirtyFlags::DRAWINGS
            | DirtyFlags::TEXT,
        &mut scene,
    );
    Some(scene)
}

/// Readability limits on schematic text, in logical pixels. The one source for
/// the CPU replay below and for `scene_shader::SchematicSurface`, so the two
/// paths clamp glyphs identically.
pub(crate) const SCHEMATIC_TEXT_SIZE: signex_gfx::primitive::text::TextSizePolicy =
    signex_gfx::primitive::text::TextSizePolicy::new(6.0, 64.0);

/// The replay options of the schematic CPU path at `transform`.
fn schematic_draw_options(
    transform: &ScreenTransform,
) -> crate::renderer_scene_canvas::SceneDrawOptions {
    crate::renderer_scene_canvas::SceneDrawOptions {
        scale_px_per_mm: transform.scale,
        min_stroke_px: signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_PX,
        text_min_px: SCHEMATIC_TEXT_SIZE.min_px,
        text_max_px: SCHEMATIC_TEXT_SIZE.max_px,
    }
}

/// Replay a built scene through the live `transform`.
pub(crate) fn draw_built_scene(
    frame: &mut canvas::Frame,
    scene: &Scene,
    transform: &ScreenTransform,
) {
    crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
        frame,
        scene,
        |point| transform.world_to_screen((point[0] as f64, point[1] as f64)),
        schematic_draw_options(transform),
    );
}

/// The CPU half of a GPU-mounted sheet: only the texts the GPU glyph atlas
/// cannot rotate, through the same transform and options as the full replay.
pub(crate) fn draw_rotated_text_fallback(
    frame: &mut canvas::Frame,
    scene: &Scene,
    transform: &ScreenTransform,
) {
    crate::renderer_scene_canvas::draw_rotated_texts(
        frame,
        scene,
        |point| transform.world_to_screen((point[0] as f64, point[1] as f64)),
        schematic_draw_options(transform),
    );
}

fn label_marker_polygon(
    label: &Label,
    stroke_color: Color,
    fill_color: [f32; 4],
    transform: &ScreenTransform,
    document_family: Option<&str>,
) -> PolygonInput {
    PolygonInput {
        vertices: label::marker_vertices(label, document_family)
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
    draw_built_scene(frame, &scene, transform);
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

/// `designators` size each designator's box from the text the canvas draws
/// (#1200), not the stored `reference`.
fn collect_item_bounds(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
) -> Vec<ItemBound> {
    collect_item_bounds_with(snapshot, true, designators)
}

/// `symbol_bodies = false` leaves out the symbol body entries, which are the
/// only ones that need the library lowering.
fn collect_item_bounds_with(
    snapshot: &SchematicRenderSnapshot,
    symbol_bodies: bool,
    designators: &Designators,
) -> Vec<ItemBound> {
    let mut out = Vec::new();
    let document_family = snapshot.font_family.as_deref();

    for symbol in &snapshot.symbols {
        if symbol_bodies {
            out.push(ItemBound {
                item: SelectedItem::new(symbol.uuid, SelectedKind::Symbol),
                bbox: symbol::symbol_bounds(snapshot, symbol),
                anchor: symbol.position,
            });
        }

        if let Some(ref_text) = symbol
            .ref_text
            .as_ref()
            .filter(|p| !p.hidden && !symbol.is_power)
        {
            out.push(ItemBound {
                item: SelectedItem::new(symbol.uuid, SelectedKind::SymbolRefField),
                bbox: text_prop_aabb(
                    designators.get(&symbol.uuid).unwrap_or(&symbol.reference),
                    ref_text,
                    symbol,
                    document_family,
                ),
                anchor: ref_text.position,
            });
        }
        if let Some(val_text) = symbol.val_text.as_ref().filter(|p| !p.hidden) {
            out.push(ItemBound {
                item: SelectedItem::new(symbol.uuid, SelectedKind::SymbolValField),
                bbox: text_prop_aabb(&symbol.value, val_text, symbol, document_family),
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
            bbox: label_aabb(label, document_family),
            anchor: label.position,
        });
    }

    for note in &snapshot.text_notes {
        out.push(ItemBound {
            item: SelectedItem::new(note.uuid, SelectedKind::TextNote),
            bbox: note_aabb(note, document_family),
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

/// The bounding box of each `selected` item, aligned with `selected`; `None`
/// for an item the sheet no longer holds.
///
/// A symbol body is the only bound that costs a library lowering, so a
/// selected symbol is looked up directly and no body is lowered for any other
/// item. The other kinds share one pass over the sheet and each symbol index
/// is built once, so the cost is the sheet size plus the selection size
/// rather than their product.
fn selected_aabbs(
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    designators: &Designators,
) -> Vec<Option<Aabb>> {
    let symbols = std::cell::OnceCell::new();
    let others = std::cell::OnceCell::new();
    selected
        .iter()
        .map(|item| {
            if item.kind == SelectedKind::Symbol {
                let index: &HashMap<uuid::Uuid, &Symbol> = symbols.get_or_init(|| {
                    let mut index = HashMap::with_capacity(snapshot.symbols.len());
                    for symbol in &snapshot.symbols {
                        index.entry(symbol.uuid).or_insert(symbol);
                    }
                    index
                });
                return index
                    .get(&item.uuid)
                    .map(|symbol| symbol::symbol_bounds(snapshot, symbol));
            }
            let index: &HashMap<SelectedItem, Aabb> = others.get_or_init(|| {
                let mut index = HashMap::new();
                for entry in collect_item_bounds_with(snapshot, false, designators) {
                    index.entry(entry.item).or_insert(entry.bbox);
                }
                index
            });
            index.get(item).copied()
        })
        .collect()
}

fn label_color(label: &Label, colors: &CanvasColors) -> Color {
    match label.label_type {
        LabelType::Net => to_iced(&colors.net_label),
        LabelType::Global => to_iced(&colors.global_label),
        LabelType::Hierarchical => to_iced(&colors.hier_label),
        LabelType::Power => to_iced(&colors.power),
    }
}

fn text_prop_aabb(
    text: &str,
    prop: &TextProp,
    symbol: &Symbol,
    document_family: Option<&str>,
) -> Aabb {
    let (rot, h_align, v_align) = field_effective_style(prop, symbol);
    drawn_text_aabb(
        text,
        resolve_family(prop.font_family.as_deref(), document_family).as_deref(),
        prop.font_size,
        (prop.position.x, prop.position.y),
        rot,
        h_align,
        v_align,
    )
}

/// `text_aabb` for a text drawn in `family`: the box geometry is the shared
/// `signex_types` one, only the width is measured in the drawn face.
fn drawn_text_aabb(
    text: &str,
    family: Option<&str>,
    font_size: f64,
    anchor: (f64, f64),
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
) -> Aabb {
    let chars = text.chars().count().max(1) as f64;
    let h = text_height_mm(font_size);
    let w = text_width::text_width_mm(text, family, h, h * 0.6 * chars);
    text_aabb_with_width(w, font_size, anchor, rotation_deg, h_align, v_align)
}

fn note_aabb(note: &TextNote, document_family: Option<&str>) -> Aabb {
    drawn_text_aabb(
        &note.text,
        resolve_family(note.font_family.as_deref(), document_family).as_deref(),
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
fn label_aabb(label: &Label, document_family: Option<&str>) -> Aabb {
    if label::is_flag(label) {
        let mut verts = label::marker_vertices(label, document_family).into_iter();
        let (x0, y0) = verts.next().expect("marker has vertices");
        return verts.fold(Aabb::new(x0, y0, x0, y0), |b, (x, y)| {
            b.union(&Aabb::new(x, y, x, y))
        });
    }
    drawn_text_aabb(
        &label.text,
        resolve_family(label.font_family.as_deref(), document_family).as_deref(),
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
    !(max_x < bounds.x - 8.0
        || max_y < bounds.y - 8.0
        || min_x > bounds.x + bounds.width + 8.0
        || min_y > bounds.y + bounds.height + 8.0)
}

fn rect_visible(min: iced::Point, size: iced::Size, bounds: Rectangle) -> bool {
    !(min.x + size.width < bounds.x - 8.0
        || min.y + size.height < bounds.y - 8.0
        || min.x > bounds.x + bounds.width + 8.0
        || min.y > bounds.y + bounds.height + 8.0)
}

fn point_visible(p: iced::Point, bounds: Rectangle, pad: f32) -> bool {
    p.x >= bounds.x - pad
        && p.y >= bounds.y - pad
        && p.x <= bounds.x + bounds.width + pad
        && p.y <= bounds.y + bounds.height + pad
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
mod drawings_tests;
#[cfg(test)]
mod font_geometry_tests;
#[cfg(test)]
mod sample_layout_tests;
#[cfg(test)]
mod scene_cache_tests;
#[cfg(test)]
mod selection_bounds_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_style_render_tests;
