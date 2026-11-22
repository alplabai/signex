//! Symbol-editor panel context and its summary view-models.

use super::*;

/// Context handed to the right-dock Properties panel and the SCH-Library
/// left-dock panel when the active tab is a `.snxsym` standalone editor.
/// Mirrors the live `SymbolDocument` but contains only the fields the
/// panels render — keeps `panel_ctx` cloneable and decouples panel code
/// from the canvas state struct.
#[derive(Debug, Clone)]
pub struct SymbolEditorPanelContext {
    /// File path of the open `.snxsym` (used as the tab key).
    pub path: std::path::PathBuf,
    /// The active symbol's `name` field (the Symbol name row).
    pub symbol_name: String,
    /// The designator template (e.g. `U?`), the Reference field.
    pub symbol_designator: String,
    /// The comment passthrough (e.g. `*` or a fixed value).
    pub symbol_comment: String,
    /// Free-text description.
    pub symbol_description: String,
    /// The component type — `Standard / Mechanical / Graphical
    /// / Net Tie / Standard (No BOM) / Jumper`.
    pub symbol_component_type: signex_library::ComponentType,
    /// The Graphical ▸ Mirrored toggle.
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
    /// A Delete of `symbols_in_file[idx]` waiting for the user's answer
    /// because rows or sheets still use the symbol (#1845).
    pub delete_warning: Option<SymbolDeleteWarning>,
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
    /// (the shared part, on every part). Drives the
    /// optional "Part 0 (shared)" tree-expander row.
    pub active_has_part_zero: bool,
    /// The body (normal or alternate) the editor shows.
    pub active_body: signex_types::schematic::BodyStyle,
    /// The Parts section's Applies to row (mockup row 123).
    pub body_scope: crate::library::editor::symbol::BodyScopeRow,
    /// The shapes and pins of the part and body the editor shows, for the
    /// Symbol tab's illustration.
    pub part_drawing: PartDrawing,
    /// The Properties panel's own state for this editor: tab, Pins filter,
    /// part chip and illustration hover (mockup rows 122 and 123).
    pub props: crate::library::editor::symbol::SymbolPropsUi,
    /// Per-`.snxlib` display settings (sheet color, grid, unit),
    /// resolved by `runtime.rs` from the
    /// containing library; falls back to defaults for lone-file
    /// edits.
    pub display: SymbolDisplayOptions,
}

/// What the Symbol tab's illustration draws: the shapes and pins of the
/// part and body the editor shows, in symbol mm (y up).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartDrawing {
    pub shapes: Vec<PartShape>,
    /// Indices into [`SymbolEditorPanelContext::pins`].
    pub pins: Vec<usize>,
}

impl SymbolEditorPanelContext {
    /// The shapes and pins of `sym` the editor shows: those `shows_graphic`
    /// and `shows_pin` keep (the editor's part and body view).
    pub fn part_drawing_of(
        sym: &signex_library::Symbol,
        shows_graphic: impl Fn(&signex_library::SymbolGraphic) -> bool,
        shows_pin: impl Fn(&signex_library::SymbolPin) -> bool,
    ) -> PartDrawing {
        use signex_library::SymbolGraphicKind as K;
        let shapes = sym
            .graphics
            .iter()
            .enumerate()
            .filter(|(_, g)| shows_graphic(g))
            .filter_map(|(idx, g)| {
                let filled = g.fill.is_some();
                Some(match &g.kind {
                    K::Rectangle { from, to } => PartShape::Rectangle {
                        from: *from,
                        to: *to,
                        filled,
                    },
                    K::Line { from, to } => PartShape::Line {
                        from: *from,
                        to: *to,
                    },
                    K::Circle { center, radius } => PartShape::Circle {
                        center: *center,
                        radius: *radius,
                        filled,
                    },
                    K::Arc {
                        center,
                        radius,
                        start_deg,
                        end_deg,
                    } => PartShape::Arc {
                        center: *center,
                        radius: *radius,
                        start_deg: *start_deg,
                        end_deg: *end_deg,
                    },
                    K::Polygon { vertices } => PartShape::Polygon {
                        idx,
                        vertices: vertices.clone(),
                        filled,
                    },
                    kind @ (K::Bezier { .. } | K::Pie { .. }) => PartShape::Outline {
                        points: kind.flattened_outline(),
                        is_closed: matches!(kind, K::Pie { .. }),
                        filled,
                    },
                    K::Text { .. } => return None,
                })
            })
            .collect();
        let pins = sym
            .pins
            .iter()
            .enumerate()
            .filter(|(_, p)| shows_pin(p))
            .map(|(idx, _)| idx)
            .collect();
        PartDrawing { shapes, pins }
    }
}

/// One shape of a [`PartDrawing`]; text is left out.
#[derive(Debug, Clone, PartialEq)]
pub enum PartShape {
    Rectangle {
        from: [f64; 2],
        to: [f64; 2],
        filled: bool,
    },
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
        filled: bool,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Polygon {
        /// Index of the graphic, so the selected polygon's illustration can
        /// find its vertices.
        idx: usize,
        vertices: Vec<[f64; 2]>,
        filled: bool,
    },
    /// A flattened Bezier (open) or pie (closed).
    Outline {
        points: Vec<[f64; 2]>,
        is_closed: bool,
        filled: bool,
    },
}

/// Sheet / grid / unit + library identity of the library holding the
/// symbol. Mirrors
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
/// reach back into the editor state to read them.
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
    /// Multi-part scoping: 1 = single-part default, 0 = the shared part
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
    /// `[start, c1, c2, end]`.
    Bezier {
        points: [[f64; 2]; 4],
    },
    Pie {
        center: [f64; 2],
        radius: f64,
        start_deg: f64,
        end_deg: f64,
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
    /// `Circle`/`Arc`/`Pie` `center.x`.
    CenterX,
    /// `Circle`/`Arc`/`Pie` `center.y`.
    CenterY,
    /// `Circle`/`Arc`/`Pie` `radius`.
    Radius,
    /// `Arc`/`Pie` `start_deg`.
    StartDeg,
    /// `Arc`/`Pie` `end_deg`.
    EndDeg,
    /// `Bezier` point `0..=3` (start, C1, C2, end), x.
    BezierX(u8),
    /// `Bezier` point `0..=3`, y.
    BezierY(u8),
    /// `Text` `position.x`.
    PositionX,
    /// `Text` `position.y`.
    PositionY,
    /// `Text` `size`.
    TextSize,
    /// All variants — `SymbolGraphic.stroke_width`.
    StrokeWidth,
    /// `Rectangle` width, `|to.x - from.x|`: written as `to.x`, keeping the
    /// side `to` is on (`state::rect_end_for_extent`).
    Width,
    /// `Rectangle` height, `|to.y - from.y|`: written as `to.y`.
    Height,
}

/// What the SCH Library panel shows before a Delete of a symbol that is
/// still in use (#1845).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDeleteWarning {
    pub idx: usize,
    pub name: String,
    /// Nothing is confirmed in use, but some table was not read, so the
    /// symbol may still be in use.
    pub only_unknown: bool,
    /// The closed sheets of the project are still being read from disk.
    pub checking: bool,
    /// One line per kind of user (component rows, open sheets).
    pub lines: Vec<String>,
}

impl SymbolDeleteWarning {
    pub fn heading(&self, i18n: &signex_i18n::I18n) -> signex_i18n::Localized {
        use signex_i18n::{ids, t};
        let id = if self.checking && self.only_unknown {
            ids::LIBRARY_SYMBOL_DELETE_CHECKING
        } else if self.only_unknown {
            ids::LIBRARY_SYMBOL_DELETE_MAY_BE_IN_USE
        } else {
            ids::LIBRARY_SYMBOL_DELETE_IN_USE
        };
        t!(i18n, id, "name" => self.name.as_str())
    }
}
