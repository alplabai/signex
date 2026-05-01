//! Signex schematic renderer — public API + per-primitive sub-modules.
//!
//! This module is the result of the v0.12 cleanroom rewrite. It is built
//! against [`signex_types::schematic`] domain types and the rendering
//! rules in `docs/RENDERING_RULES.md`; no third-party EDA source code,
//! file-format spec, or contaminated agent skill was consulted during its
//! design or implementation.
//!
//! # Architecture overview
//!
//! The renderer follows a layered, snapshot-driven design:
//!
//! 1. The caller (typically `signex-app::canvas`) builds a
//!    [`SchematicSnapshot`] each frame — a borrow-only bundle of the
//!    sheet, theme, selection, and render options. Snapshots are cheap.
//! 2. The caller passes a [`Viewport`] (world ↔ screen transform) and a
//!    mutable [`RenderLayers`] cache.
//! 3. [`render`] walks the snapshot and fills the cache layers' iced
//!    [`Frame`]s in three passes (background → content → overlay).
//! 4. Selection / hover / preview overlays go in the *overlay* layer
//!    only, so an ordinary click never invalidates the (more expensive)
//!    content layer.
//! 5. Hit testing builds a [`HitIndex`] from the snapshot — a spatial
//!    hash that gives O(k) lookup where k is the bucket population near
//!    the cursor.
//!
//! Per-primitive draw functions live in sibling modules (`wire`, `bus`,
//! `bus_entry`, `junction`, `no_connect`, `text`, `drawing`, `pin`,
//! `label`, `symbol`). Each primitive only depends on
//! [`RenderContext`], its own domain type, and the spec.
//!
//! # Spec citations
//!
//! - `docs/RENDERING_RULES.md::sch-labels` — label flag geometry.
//! - `docs/RENDERING_RULES.md::field-rotation-and-justify` — field
//!   rotation folding and justify-flip on parent transform.
//! - `docs/RENDERING_RULES.md::pin-shape-decorators` — IEEE-Std-91
//!   decorator catalog used by [`pin`].
//! - `docs/UX_REFERENCE_ALTIUM.md` — Altium parity baseline.

use signex_types::schematic::{Aabb, Point, SchematicSheet, SelectedItem, Symbol};
use signex_types::theme::CanvasColors;

pub mod bus;
pub mod bus_entry;
pub mod drawing;
pub mod field_style;
pub mod hit_test;
pub mod junction;
pub mod label;
pub mod no_connect;
pub mod pin;
pub mod selection;
pub mod symbol;
pub mod text;
pub mod viewport;
pub mod wire;

pub use viewport::Viewport;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors the schematic renderer surfaces at module boundaries.
///
/// Library code returns `Result<_, RenderError>` for fallible entries;
/// `signex-app` decides whether to surface a Signal-AI diagnostic, log
/// to the messages panel, or quietly skip the offending element.
///
/// `#[non_exhaustive]` because new variants are expected (e.g. when the
/// PCB renderer reintroduces itself or when symbol-pinning detects
/// drift). Callers must `match` with a `_` fallthrough.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// A `Symbol::lib_id` does not appear in
    /// `SchematicSheet::lib_symbols`. The renderer skips the symbol;
    /// this variant exists so consumers can attach context (e.g. an ERC
    /// violation) without parsing log strings.
    #[error("missing library symbol for lib_id `{0}`")]
    MissingLibSymbol(String),

    /// A transform produced a NaN/Inf coordinate. Returned instead of
    /// rendering NaN-corrupt geometry that iced silently turns into
    /// hour-glass paths.
    #[error("malformed transform: {0}")]
    MalformedTransform(&'static str),

    /// The snapshot has no sheet to render (caller passed a placeholder).
    #[error("empty snapshot")]
    EmptySnapshot,
}

// ---------------------------------------------------------------------------
// Render-cache invalidation
// ---------------------------------------------------------------------------

/// Tracks which of the three cache layers must be rebuilt on the next
/// render pass. Carrying this as a struct of bools (instead of a single
/// `dirty: bool`) lets a tool that only changed selection invalidate
/// the [`RenderLayers::overlay`] cache without forcing the heavier
/// [`RenderLayers::content`] cache to retessellate.
///
/// Use [`Self::all`] when something fundamental (theme change, sheet
/// swap) requires every layer to redraw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use]
pub struct RenderInvalidation {
    /// Static layer — page outline, paper background, optional title
    /// block. Invalidated by theme change, paper-size change, or the
    /// first render of a sheet.
    pub background: bool,
    /// Document content — wires, symbols, labels, drawings.
    /// Invalidated by document edits.
    pub content: bool,
    /// Transient overlay — selection rectangles, ghosts, hover hints,
    /// ERC marker pins. Invalidated by every selection / hover change.
    pub overlay: bool,
}

impl RenderInvalidation {
    /// All three layers dirty.
    #[inline]
    pub const fn all() -> Self {
        Self {
            background: true,
            content: true,
            overlay: true,
        }
    }

    /// Only the overlay layer is dirty — the typical hot path when the
    /// user changes selection, hovers, or moves a placement preview.
    #[inline]
    pub const fn overlay_only() -> Self {
        Self {
            background: false,
            content: false,
            overlay: true,
        }
    }

    /// True when at least one layer needs work.
    #[inline]
    pub const fn any(&self) -> bool {
        self.background || self.content || self.overlay
    }

    /// Apply the dirty flags to a [`RenderLayers`] by clearing the
    /// caches that need to redraw.
    #[inline]
    pub fn clear_into(&self, layers: &RenderLayers) {
        if self.background {
            layers.background.clear();
        }
        if self.content {
            layers.content.clear();
        }
        if self.overlay {
            layers.overlay.clear();
        }
    }
}

/// Three-layer render cache — keep one per active schematic tab.
///
/// The caches are iced [`canvas::Cache`](iced::widget::canvas::Cache)
/// instances; clearing one schedules its `draw` closure to re-run on the
/// next iced redraw cycle. See [`RenderInvalidation::clear_into`] for
/// the typical update pattern.
#[derive(Default)]
pub struct RenderLayers {
    pub background: iced::widget::canvas::Cache,
    pub content: iced::widget::canvas::Cache,
    pub overlay: iced::widget::canvas::Cache,
}

impl std::fmt::Debug for RenderLayers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderLayers").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Render options
// ---------------------------------------------------------------------------

/// Render-time toggles that don't belong to the document. Construct a
/// fresh `RenderOptions` per frame from `signex-app::state`; defaults
/// match Altium parity baseline.
///
/// `Eq` is intentionally NOT derived because of the `f32` field; use
/// `PartialEq` for testing equality across an epsilon if needed.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct RenderOptions {
    /// Power-port glyph style.
    pub power_port_style: crate::PowerPortStyle,
    /// Hier / global label flag style.
    pub label_style: crate::LabelStyle,
    /// Multi-sheet hierarchical sheet rendering style.
    pub multisheet_style: crate::MultisheetStyle,
    /// Visible grid style.
    pub grid_style: crate::GridStyle,
    /// `true` while the user has F5-toggled the per-net colour overlay.
    pub net_color_overlay: bool,
    /// AutoFocus dim level for unrelated objects when a selection is
    /// active. `1.0` means full opacity (effect off); `0.4` is the
    /// Altium parity default.
    pub autofocus_dim: f32,
}

impl Default for RenderOptions {
    #[inline]
    fn default() -> Self {
        Self {
            power_port_style: crate::PowerPortStyle::Altium,
            label_style: crate::LabelStyle::Classic,
            multisheet_style: crate::MultisheetStyle::Classic,
            grid_style: crate::GridStyle::Dots,
            net_color_overlay: false,
            autofocus_dim: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

/// Frozen-frame input to a render pass. Borrows the underlying sheet
/// and selection slice — does not allocate.
///
/// The snapshot is rebuilt every iced redraw, so its constructor is
/// expected to be cheap. Helpers like [`Self::lib_symbol`] are O(1)
/// `HashMap` lookups.
#[derive(Debug, Clone, Copy)]
pub struct SchematicSnapshot<'a> {
    pub sheet: &'a SchematicSheet,
    pub theme: &'a CanvasColors,
    pub selection: &'a [SelectedItem],
    pub options: RenderOptions,
}

impl<'a> SchematicSnapshot<'a> {
    /// Build a snapshot for the given sheet + theme. Selection defaults
    /// to empty; chain [`Self::with_selection`] to fill it.
    #[inline]
    pub fn new(sheet: &'a SchematicSheet, theme: &'a CanvasColors) -> Self {
        Self {
            sheet,
            theme,
            selection: &[],
            options: RenderOptions::default(),
        }
    }

    #[inline]
    pub fn with_selection(mut self, sel: &'a [SelectedItem]) -> Self {
        self.selection = sel;
        self
    }

    #[inline]
    pub fn with_options(mut self, options: RenderOptions) -> Self {
        self.options = options;
        self
    }

    /// O(1) lookup of the library symbol referenced by a placed
    /// `Symbol::lib_id`. Returns `None` when the lib_id is missing
    /// from the sheet's `lib_symbols` map (the renderer maps that to
    /// [`RenderError::MissingLibSymbol`]).
    #[inline]
    pub fn lib_symbol(&self, lib_id: &str) -> Option<&'a signex_types::schematic::LibSymbol> {
        self.sheet.lib_symbols.get(lib_id)
    }
}

// ---------------------------------------------------------------------------
// Symbol transform
// ---------------------------------------------------------------------------

/// World-space placement of a parent symbol — used by [`pin::draw_pin`]
/// and [`field_style`] when they need to fold a child's library-space
/// rotation/mirror with the parent's transform.
///
/// The transform is `Y-up library` → `Y-down schematic`: a library-space
/// pin at `(0, +pin_length)` lands at world position
/// `(0, -pin_length)` relative to the parent body when the parent has
/// no rotation or mirror.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolTransform {
    pub origin: Point,
    pub rotation_deg: f64,
    pub mirror_x: bool,
    pub mirror_y: bool,
}

impl SymbolTransform {
    /// Build from a placed `Symbol`.
    #[inline]
    pub fn from_symbol(symbol: &Symbol) -> Self {
        Self {
            origin: symbol.position,
            rotation_deg: symbol.rotation,
            mirror_x: symbol.mirror_x,
            mirror_y: symbol.mirror_y,
        }
    }

    /// Apply transform to a library-space point. The library convention
    /// is Y-up; the schematic is Y-down, so we negate `y` first.
    #[must_use]
    pub fn apply(&self, local: Point) -> Point {
        let x = local.x;
        let y = -local.y;
        let rad = -self.rotation_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let mut rx = x * cos - y * sin;
        let mut ry = x * sin + y * cos;
        if self.mirror_y {
            rx = -rx;
        }
        if self.mirror_x {
            ry = -ry;
        }
        Point::new(rx + self.origin.x, ry + self.origin.y)
    }

    /// Compose a child rotation (degrees, clockwise positive in
    /// schematic-screen space) with the parent's rotation + mirror so
    /// the rendered angle ends up correct.
    #[must_use]
    pub fn apply_angle(&self, child_deg: f64) -> f64 {
        let mut r = self.rotation_deg + child_deg;
        if self.mirror_x {
            r = -r + 180.0;
        }
        if self.mirror_y {
            r = -r;
        }
        r.rem_euclid(360.0)
    }
}

// ---------------------------------------------------------------------------
// Render context
// ---------------------------------------------------------------------------

/// Per-frame, per-primitive context. Bundles the snapshot, viewport,
/// and a few render-time helpers so primitive functions don't need
/// six-argument signatures.
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    pub snapshot: &'a SchematicSnapshot<'a>,
    pub viewport: &'a Viewport,
}

impl<'a> RenderContext<'a> {
    #[inline]
    pub fn new(snapshot: &'a SchematicSnapshot<'a>, viewport: &'a Viewport) -> Self {
        Self { snapshot, viewport }
    }

    /// Convenience: theme palette for the active sheet.
    #[inline]
    pub fn theme(&self) -> &'a CanvasColors {
        self.snapshot.theme
    }

    /// Convenience: options for the active sheet.
    #[inline]
    pub fn options(&self) -> RenderOptions {
        self.snapshot.options
    }

    /// `true` if `item` is in the snapshot's selection set.
    pub fn is_selected(&self, item: &SelectedItem) -> bool {
        self.snapshot.selection.contains(item)
    }
}

// ---------------------------------------------------------------------------
// Selection mode
// ---------------------------------------------------------------------------

/// Which selection rule applies to a hit-test query.
///
/// - `Single` — single click; topmost item at the cursor wins.
/// - `Enclosing` — left-to-right drag; only items fully inside the
///   query box are returned.
/// - `Crossing` — right-to-left drag; items overlapping the query
///   box are returned.
///
/// The two box modes match the Altium / AutoCAD convention documented
/// in `docs/UX_REFERENCE_ALTIUM.md::3.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionMode {
    Single,
    Enclosing,
    Crossing,
}

// ---------------------------------------------------------------------------
// Public render entry
// ---------------------------------------------------------------------------

/// Render the snapshot into the three-layer cache, respecting the
/// invalidation flags. This is the public entry called by
/// `signex-app::canvas` once per iced redraw.
///
/// On `Ok(())`, every dirty layer was repainted; on
/// [`RenderError::EmptySnapshot`], the caller should clear the canvas
/// itself; other errors are recoverable per their docs.
///
/// # Example
///
/// ```ignore
/// let snapshot = SchematicSnapshot::new(&sheet, &theme);
/// let invalidation = RenderInvalidation::all();
/// schematic::render(&layers, &snapshot, &viewport, invalidation)?;
/// ```
pub fn render(
    _layers: &RenderLayers,
    _snapshot: &SchematicSnapshot<'_>,
    _viewport: &Viewport,
    _invalidation: RenderInvalidation,
) -> Result<(), RenderError> {
    todo!("Wave 2-5 fill in: orchestrate per-primitive draws across the three cache layers")
}

// ---------------------------------------------------------------------------
// Public hit-test entry
// ---------------------------------------------------------------------------

/// Spatial-hash hit index — built once per snapshot version and queried
/// many times per frame. Keeps hit-test cost roughly O(k) where k is
/// the bucket population near the cursor (Q9 (c) improvement).
///
/// Build with [`HitIndex::build`]; query with [`hit_test_point`] or
/// [`hit_test_box`]. The index borrows nothing — it stores Aabbs and
/// `SelectedItem` keys, so it can outlive the snapshot it was built
/// from as long as the underlying UUIDs stay valid.
#[derive(Debug, Default, Clone)]
#[must_use]
pub struct HitIndex {
    // Wave 4 fills the field set; kept opaque for now.
    _todo: (),
}

impl HitIndex {
    /// Build the spatial-hash index for a snapshot's primitives.
    pub fn build(_snapshot: &SchematicSnapshot<'_>) -> Self {
        todo!("Wave 4 fills in: walk the sheet's primitives, bucket by world-space bbox")
    }

    /// World-space bounding box of an indexed `SelectedItem`. `None`
    /// when the item is no longer in the underlying sheet.
    pub fn aabb_of(&self, _item: &SelectedItem) -> Option<Aabb> {
        todo!("Wave 4 fills in")
    }
}

/// Single-click hit test. Returns the topmost item under `point_world`
/// per the file-order Z-rule (latest-in-vector wins; see
/// `signex-engine::Command::ReorderObjects`).
///
/// `tolerance_world` is in millimetres; typical UI value is the screen
/// hit pad converted via the active [`Viewport`]'s
/// [`world_per_pixel`](Viewport::world_per_pixel).
pub fn hit_test_point(
    _index: &HitIndex,
    _snapshot: &SchematicSnapshot<'_>,
    _point_world: Point,
    _tolerance_world: f64,
) -> Option<SelectedItem> {
    todo!("Wave 4 fills in")
}

/// Box hit test for left-/right-drag selection.
pub fn hit_test_box(
    _index: &HitIndex,
    _snapshot: &SchematicSnapshot<'_>,
    _box_world: Aabb,
    _mode: SelectionMode,
) -> Vec<SelectedItem> {
    todo!("Wave 4 fills in")
}

// ---------------------------------------------------------------------------
// Tests — public-API smoke
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Point;

    /// The public-API skeleton compiles and the `Default` constructors
    /// produce a non-panicking baseline. Wave 2 sub-agents extend this
    /// with per-primitive smoke tests.
    #[test]
    fn render_options_default_compiles() {
        let opts = RenderOptions::default();
        assert!((opts.autofocus_dim - 1.0).abs() < f32::EPSILON);
        assert!(!opts.net_color_overlay);
    }

    #[test]
    fn render_invalidation_overlay_only_is_minimal() {
        let inv = RenderInvalidation::overlay_only();
        assert!(inv.any());
        assert!(!inv.background);
        assert!(!inv.content);
        assert!(inv.overlay);
    }

    #[test]
    fn symbol_transform_identity_passes_point_through() {
        let xform = SymbolTransform {
            origin: Point::new(0.0, 0.0),
            rotation_deg: 0.0,
            mirror_x: false,
            mirror_y: false,
        };
        // Library Y-up → schematic Y-down inverts y even at identity.
        let p = xform.apply(Point::new(2.0, 3.0));
        assert!((p.x - 2.0).abs() < 1e-9);
        assert!((p.y - (-3.0)).abs() < 1e-9);
    }

    #[test]
    fn symbol_transform_quarter_turn_rotates_point() {
        let xform = SymbolTransform {
            origin: Point::new(0.0, 0.0),
            rotation_deg: 90.0,
            mirror_x: false,
            mirror_y: false,
        };
        // Library (1, 0) — point at +x, no y. A symbol rotated 90° CCW
        // *as the user sees it* moves the +x axis to the screen-up
        // direction. Screen is Y-down, so screen-up is `y < 0`. The
        // point therefore lands at `(0, -1)`.
        let p = xform.apply(Point::new(1.0, 0.0));
        assert!(p.x.abs() < 1e-9, "x = {}", p.x);
        assert!((p.y - (-1.0)).abs() < 1e-9, "y = {}", p.y);
    }
}
