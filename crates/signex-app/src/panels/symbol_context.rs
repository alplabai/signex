//! Symbol-editor panel context and its summary view-models.

use super::*;

/// Context handed to the right-dock Properties panel and the SCH-Library
/// left-dock panel when the active tab is a `.snxsym` standalone editor.
/// Mirrors the live `SymbolEditorState` but contains only the fields the
/// panels render — keeps `panel_ctx` cloneable and decouples panel code
/// from the canvas state struct.
#[derive(Debug, Clone)]
pub struct SymbolEditorPanelContext {
    /// File path of the open `.snxsym` (used as the tab key).
    pub path: std::path::PathBuf,
    /// The active symbol's `name` field (Altium "Design Item ID").
    pub symbol_name: String,
    /// Altium "Designator" (e.g. `U?`).
    pub symbol_designator: String,
    /// Altium "Comment" (e.g. `*` or a fixed value).
    pub symbol_comment: String,
    /// Free-text description.
    pub symbol_description: String,
    /// Altium "Component Type" — `Standard / Mechanical / Graphical
    /// / Net Tie / Standard (No BOM) / Jumper`.
    pub symbol_component_type: signex_library::ComponentType,
    /// Altium "Mirrored" toggle.
    pub symbol_mirrored: bool,
    /// Optional per-symbol Local Colors override (Fills / Lines /
    /// Pins). `None` = inherit from sheet palette.
    pub symbol_local_fill_color: Option<[u8; 4]>,
    pub symbol_local_line_color: Option<[u8; 4]>,
    pub symbol_local_pin_color: Option<[u8; 4]>,
    /// UUID of the active symbol — surfaced read-only on the panel.
    pub symbol_uuid: uuid::Uuid,
    /// Pin summaries for the active symbol — drives Properties panel
    /// pin selection.
    pub pins: Vec<SymbolPinSummary>,
    /// Graphic summaries for the active symbol — drives the SCH
    /// Library Graphics sub-list (click to select, populates the
    /// Properties panel).
    pub graphics: Vec<GraphicSummary>,
    /// What's currently selected on the canvas. Drives the right-dock
    /// Properties panel's mode (Pin / Field / Component default).
    pub selected: SymbolEditorSelection,
    /// Copy of the active editor's transient graphic-fill picker
    /// open-state so the pure view can expand a graphic's inline colour
    /// palette / HSV overlay. `None` = closed.
    pub graphic_fill_picker: Option<crate::app::GraphicFillPicker>,
    /// Copy of the active editor's transient local-colour picker
    /// open-state (Fills / Lines / Pins). `None` = closed.
    pub local_color_picker: Option<crate::app::LocalColorPicker>,
    /// All symbols in the open `.snxsym` container — feeds the SCH
    /// Library left-dock panel's components list.
    pub symbols_in_file: Vec<SymbolFileEntry>,
    /// Index into `symbols_in_file` for the symbol currently being
    /// edited. The SCH Library panel's row click rewrites this.
    pub active_idx: usize,
    /// Active sub-part of the currently-edited symbol — surfaced on
    /// the in-tab toolbar's `Part X / N` picker and the SCH Library
    /// panel's part tree-expander.
    pub active_part: u8,
    /// Highest declared `part_number` across the active symbol's
    /// pins (`1` for single-part components). Drives the tree-
    /// expander row count under the active symbol and clamps the
    /// in-tab toolbar's right arrow.
    pub active_max_part: u8,
    /// Whether any pin on the active symbol carries `part_number == 0`
    /// (Altium "Part Zero" — shared across every part). Drives the
    /// optional "Part 0 (shared)" tree-expander row.
    pub active_has_part_zero: bool,
    /// Per-`.snxlib` display settings (sheet color, grid, unit) —
    /// rendered on the Properties panel's "None" branch as Altium
    /// Document Options. Resolved by `runtime.rs` from the
    /// containing library; falls back to defaults for lone-file
    /// edits.
    pub display: SymbolDisplayOptions,
}

/// Sheet / grid / unit + library identity surfaced on the
/// Properties panel as Altium "Document Options" when nothing is
/// selected on the symbol canvas. Mirrors
/// [`crate::library::state::LibraryDisplaySettings`] but lives in
/// the panels crate so view code doesn't pull
/// `crate::library::state` directly.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolDisplayOptions {
    pub sheet_color: SheetColor,
    pub grid_visible: bool,
    pub grid_size_mm: f32,
    pub unit: signex_types::coord::Unit,
    /// Display name of the containing `.snxlib` (or the file stem
    /// when the symbol lives outside any mounted library).
    pub library_name: String,
    /// Total number of symbols across every `.snxsym` in the
    /// containing library — surfaced as "Symbols in library: N".
    /// `None` for lone-file edits.
    pub library_symbol_count: Option<usize>,
}

impl Default for SymbolDisplayOptions {
    fn default() -> Self {
        Self {
            sheet_color: SheetColor::default(),
            grid_visible: true,
            grid_size_mm: 2.54,
            unit: signex_types::coord::Unit::Mm,
            library_name: "(lone file)".to_string(),
            library_symbol_count: None,
        }
    }
}

/// One symbol's row entry in the SCH Library panel's components list.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolFileEntry {
    pub idx: usize,
    pub name: String,
    pub uuid: uuid::Uuid,
    pub pin_count: usize,
    /// Free-text description — surfaced as the second tree column.
    pub description: String,
}

/// Extended pin fields surfaced on the Properties panel — flows
/// alongside [`SymbolPinSummary`] so the panel doesn't have to
/// reach back into the editor state to read them. Mirrors the
/// Altium SchLib Pin Properties layout.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPinDetails {
    pub description: String,
    pub function: Vec<String>,
    pub pin_package_length: Option<f64>,
    pub propagation_delay_ns: Option<f64>,
    pub designator_visible: bool,
    pub name_visible: bool,
    pub inside_symbol: signex_library::PinSymbolKind,
    pub inside_edge_symbol: signex_library::PinSymbolKind,
    pub outside_edge_symbol: signex_library::PinSymbolKind,
    pub outside_symbol: signex_library::PinSymbolKind,
    pub hidden: bool,
    pub locked: bool,
    /// Multi-part scoping: 1 = single-part default, 0 = "Part Zero"
    /// (pin appears on every part), 2..N = scoped to that part.
    pub part_number: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPinSummary {
    pub idx: usize,
    pub number: String,
    pub name: String,
    pub electrical: String,
    pub position: [f64; 2],
    pub orientation: String,
    pub length: f64,
    /// Extended fields — surfaced on the Properties panel only when
    /// the pin is selected. Kept in a sub-struct so the SCH Library
    /// Pins sub-list (which only needs number/name/electrical) can
    /// stay terse.
    pub details: SymbolPinDetails,
}

/// What the canvas currently has selected — drives Properties panel
/// content. `None` = nothing selected, panel shows the symbol's defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolEditorSelection {
    None,
    Pin(SymbolPinSummary),
    FieldReference,
    FieldValue,
    /// A placed graphic — drives the per-shape Properties branch
    /// (corners / endpoints / centre + radius / arc angles / text +
    /// stroke).
    Graphic(GraphicSummary),
}

/// Per-shape Properties-panel summary for a placed `SymbolGraphic`.
/// Cloned out of the live `Symbol::graphics` vector each refresh so
/// the panel doesn't hold a borrow into the editor state.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicSummary {
    pub idx: usize,
    pub kind: GraphicKindSummary,
    pub stroke_width: f64,
    /// Solid fill colour (RGBA) or `None` for an unfilled outline.
    /// Only surfaced in the panel for closed shapes (Rectangle / Circle).
    pub fill: Option<[u8; 4]>,
}

/// Per-variant geometry for [`GraphicSummary`] — mirrors
/// `signex_library::SymbolGraphicKind` so the panel can render each
/// shape's editable fields without depending on the library type.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphicKindSummary {
    Rectangle {
        from: [f64; 2],
        to: [f64; 2],
    },
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Text {
        position: [f64; 2],
        content: String,
        size: f64,
    },
    /// Read-only vertex count — no per-vertex numeric editor in this
    /// slice; the canvas's per-vertex drag handles cover editing.
    Polygon {
        vertex_count: usize,
    },
}

/// Identifier for one numeric field on a graphic — carried by
/// [`PanelMsg::SymEditorSetGraphicField`] so the dispatcher knows which
/// scalar to mutate. The dispatcher silently ignores (idx, field)
/// pairs whose field doesn't apply to the graphic's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicFieldId {
    /// `Rectangle`/`Line` `from.x`.
    FromX,
    /// `Rectangle`/`Line` `from.y`.
    FromY,
    /// `Rectangle`/`Line` `to.x`.
    ToX,
    /// `Rectangle`/`Line` `to.y`.
    ToY,
    /// `Circle`/`Arc` `center.x`.
    CenterX,
    /// `Circle`/`Arc` `center.y`.
    CenterY,
    /// `Circle`/`Arc` `radius`.
    Radius,
    /// `Arc` `start_deg`.
    StartDeg,
    /// `Arc` `end_deg`.
    EndDeg,
    /// `Text` `position.x`.
    PositionX,
    /// `Text` `position.y`.
    PositionY,
    /// `Text` `size`.
    TextSize,
    /// All variants — `SymbolGraphic.stroke_width`.
    StrokeWidth,
}
