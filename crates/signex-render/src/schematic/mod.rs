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

/// **Deprecated v0.12 extension trait** that exposes the v0.11
/// per-field accessors as inherent methods on `SchematicSheet`. Old
/// callers (e.g. `signex-app`'s field-drag preview) reach for
/// `sheet.symbol_position(uuid)` etc. — import this trait at the
/// call site to keep them compiling.
#[allow(deprecated)]
pub trait SchematicSheetExt {
    /// World-space position of the placed symbol identified by
    /// `uuid`. `None` when no such symbol exists.
    fn symbol_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)>;

    /// World-space position of the *reference* field on the placed
    /// symbol identified by `uuid`, or `None` when the symbol has no
    /// stored reference text or doesn't exist.
    fn symbol_reference_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)>;

    /// World-space position of the *value* field on the placed
    /// symbol identified by `uuid`.
    fn symbol_value_position(&self, uuid: uuid::Uuid) -> Option<(f64, f64)>;
}

#[allow(deprecated)]
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

mod util;

pub use viewport::Viewport;

// ---------------------------------------------------------------------------
// v0.11 → v0.12 compatibility aliases.
//
// The cleanroom rewrite redesigned the public API (Stage 1 Q2 = b).
// These aliases let the v0.11 surface keep compiling against
// `signex-app` while the consumer-side rename lands in a follow-up
// PR. The aliases are deprecated; remove them in v0.13 once
// `signex-app` no longer imports the old names.
// ---------------------------------------------------------------------------

/// **Deprecated v0.12 alias.** v0.11's `SchematicRenderSnapshot` was
/// equivalent to an owned [`signex_types::SchematicSheet`]; the alias
/// keeps the v0.11 type identifier compiling. Construct a wrapper
/// directly via `sheet.clone()` instead of the legacy
/// `SchematicRenderSnapshot::from_sheet(...)` helper.
#[deprecated(
    since = "0.12.0",
    note = "use SchematicSnapshot (borrow-based) or SchematicSheet (owned)"
)]
pub type SchematicRenderSnapshot = SchematicSheet;

/// **Deprecated v0.12 wrapper** for the v0.11 `SchematicRenderCache`.
///
/// Stores the iced canvas caches (background / content / overlay) and
/// — for v0.11 compat — a bundled sheet so callers that ask for
/// `cache.snapshot()` can retrieve the last sheet the cache rebuilt
/// against. New code should pass a fresh [`SchematicSnapshot`] into
/// [`render`] per frame.
#[derive(Default)]
#[deprecated(since = "0.12.0", note = "use RenderLayers")]
pub struct SchematicRenderCache {
    layers: RenderLayers,
    sheet: Option<SchematicSheet>,
    preview: Option<SchematicSheet>,
}

#[allow(deprecated)]
impl SchematicRenderCache {
    /// Build a cache from a sheet — stores a clone of the sheet so
    /// `cache.snapshot()` can be queried later.
    pub fn from_sheet(sheet: &SchematicSheet) -> Self {
        Self {
            layers: RenderLayers::default(),
            sheet: Some(sheet.clone()),
            preview: None,
        }
    }

    /// Update the cached sheet and clear the iced caches according to
    /// `invalidation`.
    pub fn update_from_sheet(&mut self, sheet: &SchematicSheet, invalidation: RenderInvalidation) {
        self.sheet = Some(sheet.clone());
        invalidation.clear_into(&self.layers);
    }

    /// The most recently snapshotted sheet — panics when the cache
    /// was constructed via `Default` and never received a sheet.
    /// Maintains v0.11's call shape (some callers rely on a `&Sheet`
    /// rather than `Option<&Sheet>`).
    pub fn snapshot(&self) -> &SchematicSheet {
        self.sheet
            .as_ref()
            .expect("RenderCache::snapshot called before from_sheet/update_from_sheet")
    }

    /// Optional ghost-preview snapshot for tools that paint a hover
    /// preview (placement, drag). v0.12: always `None`; consumers
    /// should paint previews directly on the overlay frame.
    pub fn prepared_preview(&self) -> Option<&SchematicSheet> {
        self.preview.as_ref()
    }

    /// Direct access to the layered iced canvas caches.
    pub fn layers(&self) -> &RenderLayers {
        &self.layers
    }
}

#[allow(deprecated)]
impl std::fmt::Debug for SchematicRenderCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchematicRenderCache")
            .finish_non_exhaustive()
    }
}

/// **Deprecated v0.12 alias.** Use [`Viewport`].
#[deprecated(since = "0.12.0", note = "use Viewport")]
pub type ScreenTransform = Viewport;

/// **Deprecated v0.12 helper.** Apply a placed symbol's transform to
/// a library-space point. New code should use
/// `SymbolTransform::from_symbol(sym).apply(point)`.
#[deprecated(
    since = "0.12.0",
    note = "use SymbolTransform::from_symbol(sym).apply(point)"
)]
pub fn instance_transform(symbol: &Symbol, local_point: &Point) -> (f64, f64) {
    let world = SymbolTransform::from_symbol(symbol).apply(*local_point);
    (world.x, world.y)
}

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
    /// All three layers dirty. v0.11 also exposed this as `FULL`.
    #[inline]
    pub const fn all() -> Self {
        Self {
            background: true,
            content: true,
            overlay: true,
        }
    }

    /// **Deprecated v0.12 alias** for [`Self::all`].
    pub const FULL: Self = Self {
        background: true,
        content: true,
        overlay: true,
    };

    /// **Deprecated v0.12 alias** for the default (all-clean) state.
    pub const NONE: Self = Self {
        background: false,
        content: false,
        overlay: false,
    };

    // v0.11 per-primitive invalidation flags. The new API is 3-layer
    // (background/content/overlay) instead, so all per-primitive flags
    // alias to "content layer dirty" — the safest, smallest superset
    // that preserves correctness without splitting layers further.

    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag — now
    /// triggers the full content-layer rebuild.
    pub const PAPER: Self = Self::FULL;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const WIRES: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const SYMBOLS: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const LIB_SYMBOLS: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const LABELS: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const JUNCTIONS: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const NO_CONNECTS: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const TEXT_NOTES: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const DRAWINGS: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const CHILD_SHEETS: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const BUS_ENTRIES: Self = Self::CONTENT_ONLY;
    /// **Deprecated v0.12 alias.** v0.11 per-primitive flag.
    pub const BUSES: Self = Self::CONTENT_ONLY;

    /// Helper used by the per-primitive aliases above.
    pub const CONTENT_ONLY: Self = Self {
        background: false,
        content: true,
        overlay: false,
    };

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

impl std::ops::BitOr for RenderInvalidation {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self {
            background: self.background | rhs.background,
            content: self.content | rhs.content,
            overlay: self.overlay | rhs.overlay,
        }
    }
}

impl std::ops::BitOrAssign for RenderInvalidation {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.background |= rhs.background;
        self.content |= rhs.content;
        self.overlay |= rhs.overlay;
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
            label_style: crate::LabelStyle::Standard,
            multisheet_style: crate::MultisheetStyle::Standard,
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
/// canvas size (for frustum culling), and a few render-time helpers so
/// primitive functions don't need six-argument signatures.
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    pub snapshot: &'a SchematicSnapshot<'a>,
    pub viewport: &'a Viewport,
    pub canvas_size: iced::Size,
}

impl<'a> RenderContext<'a> {
    /// Build a context for a frame. `canvas_size` comes from the iced
    /// canvas widget's bounds. Defaults to `(0, 0)` when called from
    /// non-canvas contexts (e.g. a unit test); primitives gracefully
    /// skip culling in that case.
    #[inline]
    pub fn new(snapshot: &'a SchematicSnapshot<'a>, viewport: &'a Viewport) -> Self {
        Self {
            snapshot,
            viewport,
            canvas_size: iced::Size::ZERO,
        }
    }

    /// Build a context with an explicit canvas size.
    #[inline]
    pub fn with_size(
        snapshot: &'a SchematicSnapshot<'a>,
        viewport: &'a Viewport,
        canvas_size: iced::Size,
    ) -> Self {
        Self {
            snapshot,
            viewport,
            canvas_size,
        }
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

    /// World-space rectangle currently visible — what every primitive
    /// frustum-culls against. When `canvas_size` is zero (test
    /// fixtures) we return an "everything visible" `Aabb` so primitives
    /// don't accidentally skip rendering.
    pub fn visible_world_bounds(&self) -> signex_types::schematic::Aabb {
        if self.canvas_size.width <= 0.0 || self.canvas_size.height <= 0.0 {
            return signex_types::schematic::Aabb::new(
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::INFINITY,
            );
        }
        self.viewport.visible_world_bounds(self.canvas_size)
    }
}

// ---------------------------------------------------------------------------
// Selection mode
// ---------------------------------------------------------------------------

/// Which selection rule applies to a hit-test query.
///
/// - `Single` — single click; topmost item at the cursor wins.
/// - `Inside` — left-to-right drag (Altium "Inside" / AutoCAD
///   "enclosing"); only items fully inside the query box are returned.
/// - `Touching` — right-to-left drag (Altium "Touching" / AutoCAD
///   "crossing"); items overlapping the query box are returned.
///
/// The two box modes match the Altium / AutoCAD convention documented
/// in `docs/UX_REFERENCE_ALTIUM.md::3.1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionMode {
    Single,
    /// Left-to-right drag — fully-enclosing selection.
    #[default]
    Inside,
    /// Right-to-left drag — overlap-touching selection.
    Touching,
}

// ---------------------------------------------------------------------------
// Public render entry
// ---------------------------------------------------------------------------

/// **Deprecated v0.12 alias** for [`render`] using the v0.11 7-argument
/// signature. The `_canvas_bounds`, `_focus_ref`, and
/// `_color_overrides` arguments are accepted for source compatibility
/// and currently ignored — the new render path bakes theme into the
/// snapshot. Consumers should migrate to [`render`] over time.
#[allow(deprecated)]
#[deprecated(
    since = "0.12.0",
    note = "use schematic::render with a SchematicSnapshot"
)]
pub fn render_schematic(
    frame: &mut iced::widget::canvas::Frame,
    sheet: &SchematicSheet,
    viewport: &Viewport,
    theme: &CanvasColors,
    _canvas_bounds: iced::Rectangle,
    _focus_ref: Option<&std::collections::HashSet<uuid::Uuid>>,
    _color_overrides: Option<&std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>>,
) {
    let snapshot = SchematicSnapshot::new(sheet, theme);
    let _ = render(frame, &snapshot, viewport);
}

/// **Deprecated v0.12 shim** of the old `draw_power_port_preview`
/// helper. v0.11 painted a translucent symbol ghost during placement;
/// v0.12's tooling paints previews directly on the overlay frame, so
/// this shim is a no-op pending a Wave-7 placement-tool refactor.
#[deprecated(
    since = "0.12.0",
    note = "ghost previews paint directly on the overlay frame"
)]
pub fn draw_power_port_preview(
    _frame: &mut iced::widget::canvas::Frame,
    _symbol: &Symbol,
    _viewport: &Viewport,
    _color: iced::Color,
) {
    // Intentional no-op for v0.12 — see doc.
}

/// Render every primitive into a single iced canvas frame.
///
/// `signex-app::canvas` calls this from each of its three cache draw
/// closures (with the cache layer determining whether background /
/// content / overlay is being filled). This single-entry helper paints
/// **everything** — useful for one-shot exports (e.g. the print-preview
/// or PDF backend) where layered caching isn't needed.
///
/// # Example
///
/// ```ignore
/// let snapshot = SchematicSnapshot::new(&sheet, &theme);
/// schematic::render(frame, &snapshot, &viewport)?;
/// ```
pub fn render(
    frame: &mut iced::widget::canvas::Frame,
    snapshot: &SchematicSnapshot<'_>,
    viewport: &Viewport,
) -> Result<(), RenderError> {
    if snapshot.sheet.symbols.is_empty()
        && snapshot.sheet.wires.is_empty()
        && snapshot.sheet.labels.is_empty()
    {
        // Empty sheet — nothing to paint, but not an error.
    }
    let ctx = RenderContext::new(snapshot, viewport);

    // Render order — bottom layer first. Hit-test reverses this so
    // the topmost item wins under a click. See
    // `hit_test::HitIndex::build` for the parallel order.
    for d in &snapshot.sheet.drawings {
        drawing::draw_drawing(frame, d, &ctx);
    }
    for b in &snapshot.sheet.buses {
        bus::draw_bus(frame, b, &ctx);
    }
    for w in &snapshot.sheet.wires {
        wire::draw_wire(frame, w, &ctx);
    }
    for e in &snapshot.sheet.bus_entries {
        bus_entry::draw_bus_entry(frame, e, &ctx);
    }
    for s in &snapshot.sheet.symbols {
        match snapshot.lib_symbol(&s.lib_id) {
            Some(lib) => symbol::draw_symbol(frame, s, lib, &ctx),
            None => {
                // Surface the missing lib symbol but don't abort the
                // whole frame — partial render is preferable to a
                // blank canvas.
                return Err(RenderError::MissingLibSymbol(s.lib_id.clone()));
            }
        }
    }
    for j in &snapshot.sheet.junctions {
        junction::draw_junction(frame, j, &ctx);
    }
    for nc in &snapshot.sheet.no_connects {
        no_connect::draw_no_connect(frame, nc, &ctx);
    }
    for l in &snapshot.sheet.labels {
        label::draw_label(frame, l, &ctx);
    }
    for n in &snapshot.sheet.text_notes {
        text::draw_text_note(frame, n, &ctx);
    }

    selection::render_selection_overlay(frame, snapshot, viewport);

    Ok(())
}

// ---------------------------------------------------------------------------
// Public hit-test entry
// ---------------------------------------------------------------------------

pub use hit_test::HitIndex;

/// Single-click hit test. Returns the topmost item under `point_world`
/// per the render-order Z-rule (latest-rendered = topmost; see
/// [`hit_test`] for the exact order).
///
/// `tolerance_world` is in millimetres; typical UI value is the screen
/// hit pad converted via the active [`Viewport`]'s
/// [`world_per_pixel`](Viewport::world_per_pixel).
pub fn hit_test_point(
    index: &HitIndex,
    snapshot: &SchematicSnapshot<'_>,
    point_world: Point,
    tolerance_world: f64,
) -> Option<SelectedItem> {
    hit_test::point(index, snapshot, point_world, tolerance_world)
}

/// Box hit test for left-/right-drag selection.
pub fn hit_test_box(
    index: &HitIndex,
    snapshot: &SchematicSnapshot<'_>,
    box_world: Aabb,
    mode: SelectionMode,
) -> Vec<SelectedItem> {
    hit_test::box_query(index, snapshot, box_world, mode)
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
