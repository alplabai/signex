//! Panel implementations — uses signex-widgets for proper Altium-style content.

use iced::mouse;
use iced::widget::canvas;
use iced::widget::{
    Column, Row, Space, button, column, container, pick_list, row, scrollable, svg, text,
    text_input,
};
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Theme};
use iced_aw::{NumberInput, Wrap};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::tree_view::{TreeIcon, TreeMsg, TreeNode, TreeView};
use std::sync::OnceLock;

pub mod components_panel;
mod element_properties;
mod footprint_editor_properties;
pub mod history;
mod properties_parameters;
mod symbol_editor_properties;

use element_properties::{
    view_child_sheet_properties, view_drawing_properties, view_selected_element_properties,
};
use footprint_editor_properties::view_footprint_editor_properties;
// HI-22: re-export the form / layout helpers extracted into
// `properties_parameters.rs` so sibling modules (element_properties,
// symbol_editor_properties, footprint_editor_properties) and other
// view fns still in this file can reach them via `super::name(...)`.
pub(super) use properties_parameters::{
    canvas_font_popup, custom_filter_tab, empty_section_row, font_style_row, form_check_row,
    form_check_row_shortcut, form_edit_row, form_font_link_row, form_grid_row, form_grid_size_row,
    form_input_row, form_int_edit_row, form_label, form_label_row, form_mm_edit_row, form_pick_row,
    justification_grid, net_numeric_row, net_params_add_bar, net_params_header, net_params_tabs,
    param_table_row, preplacement_justification_grid, preset_chip, props_tab_btn, section_hdr,
    seg_btn, tag_btn, thin_sep, view_custom_selection_filters_section,
};
use properties_parameters::{view_properties_general, view_properties_parameters};
use symbol_editor_properties::view_symbol_editor_properties;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum PanelKind {
    Projects,
    Components,
    Navigator,
    Properties,
    Filter,
    Erc,
    Messages,
    Signal,
    Drc,
    LayerStack,
    NetClasses,
    Variants,
    OutputJobs,
    // Additional Altium schematic panels
    SchFilter,
    SchList,
    BomStudio,
    Favorites,
    Snippets,
    Todo,
    Wiki,
    /// v0.9 Library panel — open `*.snxlib/` libraries, drill into
    /// their components, drag onto the canvas (Phase 2).
    Library,
    /// v0.9 phase 2.5 — Altium SCH Library panel. Lists symbols
    /// inside the active `.snxsym` container; clicking switches the
    /// editor's `active_idx`. Visible whenever a Symbol editor tab
    /// is focused.
    SchLibrary,
    /// v0.14.2 — Altium PCB Library panel parity. Lists every
    /// `.snxfpt` sibling inside the containing `.snxlib`'s
    /// `footprints/` directory; clicking opens that footprint as a
    /// new tab (same `Open ▸ Footprint` flow). Visible whenever a
    /// Footprint editor tab is focused.
    FootprintLibrary,
    /// VSCode-style per-file Git history. Right-dock surface that
    /// follows the active tab and shows the file's last 50 commits
    /// via `signex_widgets::history_pane`.
    History,
}

/// All available panel kinds for the panel list button.
pub const ALL_PANELS: &[PanelKind] = &[
    PanelKind::Projects,
    PanelKind::Components,
    PanelKind::Library,
    PanelKind::Navigator,
    PanelKind::Properties,
    PanelKind::Filter,
    PanelKind::SchFilter,
    PanelKind::SchList,
    PanelKind::Erc,
    PanelKind::Messages,
    PanelKind::Signal,
    PanelKind::Drc,
    PanelKind::BomStudio,
    PanelKind::Favorites,
    PanelKind::Snippets,
    PanelKind::LayerStack,
    PanelKind::NetClasses,
    PanelKind::Variants,
    PanelKind::OutputJobs,
    PanelKind::Todo,
    PanelKind::Wiki,
    PanelKind::SchLibrary,
    PanelKind::FootprintLibrary,
    PanelKind::History,
];

impl PanelKind {
    /// Whether this panel requires a schematic document to be open.
    pub fn needs_schematic(self) -> bool {
        matches!(
            self,
            PanelKind::Navigator
                | PanelKind::Properties
                | PanelKind::Filter
                | PanelKind::Erc
                | PanelKind::SchFilter
                | PanelKind::SchList
                | PanelKind::Signal
                | PanelKind::Drc
                | PanelKind::BomStudio
                | PanelKind::Snippets
                | PanelKind::Variants
                | PanelKind::OutputJobs
        )
    }

    /// Whether this panel requires a PCB document to be open.
    pub fn needs_pcb(self) -> bool {
        matches!(self, PanelKind::LayerStack | PanelKind::NetClasses)
    }

    pub fn label(self) -> &'static str {
        match self {
            PanelKind::Projects => "Projects",
            PanelKind::Components => "Components",
            PanelKind::Navigator => "Navigator",
            PanelKind::Properties => "Properties",
            PanelKind::Filter => "Filter",
            PanelKind::Erc => "ERC",
            PanelKind::Messages => "Messages",
            PanelKind::Signal => "Signal",
            PanelKind::Drc => "DRC",
            PanelKind::LayerStack => "Layer Stack",
            PanelKind::NetClasses => "Net Classes",
            PanelKind::Variants => "Variants",
            PanelKind::SchFilter => "SCH Filter",
            PanelKind::SchList => "SCH List",
            PanelKind::BomStudio => "BOM Studio",
            PanelKind::Favorites => "Favorites",
            PanelKind::Snippets => "Snippets",
            PanelKind::Todo => "To-Do",
            PanelKind::Wiki => "Wiki",
            PanelKind::OutputJobs => "Output Jobs",
            PanelKind::Library => "Library",
            PanelKind::SchLibrary => "SCH Library",
            PanelKind::FootprintLibrary => "Footprint Library",
            PanelKind::History => "History",
        }
    }
}

/// Track which sections are collapsed (by section name).
pub type CollapsedSections = std::collections::HashSet<String>;

/// Per-sheet info for the project tree.
#[derive(Debug, Clone)]
pub struct SheetInfo {
    #[allow(dead_code)]
    pub name: String,
    pub filename: String,
    pub sym_count: usize,
    #[allow(dead_code)]
    pub wire_count: usize,
    #[allow(dead_code)]
    pub label_count: usize,
    /// True when this sheet is currently in `document_state.tabs`.
    /// Drives the small accent-coloured dot on the tree row (Altium parity).
    pub is_open: bool,
    /// True when the open tab for this sheet has unsaved edits.
    /// Drives the bright red dot on the tree row.
    pub is_dirty: bool,
    /// True when this sheet is the document the user is currently
    /// viewing (`document_state.tabs[active_tab].path == sheet path`).
    /// Drives the highlighted row background — Altium parity.
    pub is_active: bool,
    /// F24 (2026-05-03) — `true` when the file backing this entry
    /// is registered on the project but no longer exists on disk
    /// (orphan reference, e.g. user moved/deleted outside Signex).
    /// Drives the `(missing)` suffix in `build_project_tree` so the
    /// user sees the broken state at a glance instead of having to
    /// double-click and read an error.
    pub missing: bool,
}

/// Per-project bundle surfaced to the Projects panel. One entry per
/// `LoadedProject` in `DocumentState.projects`. `build_project_tree`
/// iterates this list to emit one tree root per project.
#[derive(Debug, Clone)]
pub struct ProjectPanelInfo {
    pub id: crate::app::ProjectId,
    /// Display name (project stem — "MyBoard" from "MyBoard.standard_pro").
    pub name: String,
    /// Root schematic filename shown as the "project file" under each
    /// root, when present.
    pub project_file: Option<String>,
    /// Open / dirty state for the root schematic, mirrors the same
    /// flags that `SheetInfo` carries for inner sheets.
    pub project_file_open: bool,
    pub project_file_dirty: bool,
    pub project_file_active: bool,
    /// F24 — same `(missing)` indicator as on `SheetInfo` but for the
    /// project's root schematic.
    pub project_file_missing: bool,
    /// Companion PCB filename, when present.
    pub pcb_file: Option<String>,
    pub pcb_file_open: bool,
    pub pcb_file_dirty: bool,
    pub pcb_file_active: bool,
    /// F24 — `(missing)` indicator for the companion PCB file.
    pub pcb_file_missing: bool,
    pub sheets: Vec<SheetInfo>,
    /// Component libraries attached to this project. One entry per
    /// `Project::libraries[]`. Drives the `Libraries` branch under
    /// the project root — each entry renders as a `*.snxlib` leaf
    /// and (when the library is mounted) a small list of cached
    /// components beneath it.
    pub libraries: Vec<LibraryNodeInfo>,
    /// Whether this is the currently-active project — drives accent
    /// styling on the root node.
    pub is_active: bool,
    /// `.snxprj` dirty state — flips when in-memory project metadata
    /// (sheet list / pcb / libraries) has changed and not yet been
    /// written via `write_project`. Drives the red dirty indicator
    /// on the project root row so the user knows Save is pending.
    pub is_dirty: bool,
}

/// Per-library bundle for the project tree's `Libraries` group.
/// Mirrors what [`signex_types::project::LibraryEntry`] records on
/// the project, plus a couple of cached fields the panel pulls from
/// `LibraryState` so the view doesn't have to re-borrow the library
/// crate at render time.
///
/// The library renders as a single leaf in the project tree under
/// the v0.9 `.snxlib`-as-file model — symbols / footprints / sims
/// are not surfaced here. Browsing the library's contents is the
/// Library Browser tab's job; double-clicking the leaf opens it.
#[derive(Debug, Clone)]
pub struct LibraryNodeInfo {
    /// Display name for the row — `<entry.path>.file_name()` or the
    /// manifest name when the library is mounted.
    pub display_name: String,
    /// Absolute on-disk path of the `.snxlib` file — feeds the
    /// right-click menu (Add New ▸ Component pre-selects this path)
    /// and the double-click → Library Browser open dispatch.
    pub root: std::path::PathBuf,
    /// True when the library is currently mounted in
    /// `LibraryState::open_libraries`. Drives the icon tint —
    /// unmounted entries render in the muted "missing" colour.
    pub mounted: bool,
    /// F24 — `true` when the `.snxlib` file is registered on the
    /// project but no longer exists on disk. Drives the `(missing)`
    /// suffix in the tree so the user spots orphan references
    /// without double-clicking through to a "Library not mounted"
    /// recovery message.
    pub missing: bool,
    /// F29 — names of `.snxsym` files inside this library. Populated
    /// from `OpenLibrary::cached_symbols` when the library is
    /// mounted. Empty when the library is unmounted or has no
    /// symbols yet. Used by `build_project_tree` to surface a
    /// `Symbols` subbranch under the library node so the user can
    /// navigate to a specific symbol file directly from the tree.
    pub symbols: Vec<String>,
    /// F29 — same as `symbols` but for `.snxfpt` footprints. Used to
    /// build the `Footprints` subbranch under the library node.
    pub footprints: Vec<String>,
    /// v0.13 — `true` when this `.snxlib` is currently open as a tab
    /// (Library Browser). Drives the white open-dot indicator in the
    /// project tree, matching schematic / pcb sheet open status.
    pub is_open: bool,
    /// v0.13 — `true` when the `.snxlib` has unsaved changes. Drives
    /// the red dirty-dot indicator in the project tree.
    pub is_dirty: bool,
}

/// Context passed to panels — owned data to avoid lifetime issues.
#[derive(Debug, Clone)]
pub struct LibrarySymbolEntry {
    pub lib_id: String,
    pub symbol_name: String,
    pub library_name: String,
    pub pin_count: usize,
}

/// Flattened ERC diagnostic row for the ERC panel.
///
/// The app flattens per-sheet ERC caches into this list so the panel can
/// present one navigable table across the entire project.
#[derive(Debug, Clone)]
pub struct ErcDiagnosticEntry {
    pub global_index: usize,
    pub sheet_name: String,
    pub sheet_path: std::path::PathBuf,
    pub severity: ErcSeverityLite,
    pub rule_label: &'static str,
    /// Underlying rule kind — drives the Quick Fix chip's label and
    /// per-rule action (UnusedPin → place a NoConnect; others →
    /// zoom + select on the canvas). Carrying it here means the panel
    /// view can decide both the label and the dispatch with no
    /// extra lookup against `erc_violations_by_path`.
    pub rule_kind: signex_erc::RuleKind,
    pub message: String,
    pub world_x: f64,
    pub world_y: f64,
    pub select: Option<signex_types::schematic::SelectedItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcSeverityLite {
    Error,
    Warning,
    Info,
}

/// Detail for the row currently selected in the active Library
/// Browser tab. Surfaces in the right-edge Properties panel so
/// primitive binding (Pick Symbol / Pick Footprint) lives next to
/// every other "what am I selected?" affordance the user looks for
/// there. F15 (2026-05-03 library polish): "right pane can be opened
/// on properties instead."
///
/// Populated by `refresh_panel_ctx` when the active tab is
/// `TabKind::LibraryBrowser(path)` AND the matching browser state's
/// `selected_row` is `Some(_)`. Cleared otherwise.
#[derive(Debug, Clone)]
pub struct LibraryRowDetail {
    pub library_path: std::path::PathBuf,
    pub table: String,
    pub row_id: uuid::Uuid,
    pub internal_pn: String,
    /// Class string stored on the row — derived from the table name
    /// at create time per F20 (Tables-only model). Surfaced as
    /// read-only metadata only; users can't edit it directly.
    pub class: String,
    /// Pretty `LifecycleState` token ("Draft", "Released", …).
    pub lifecycle_label: String,
    /// "Symbol bound" / "Symbol unresolved (UUID not mounted)" /
    /// "Symbol unbound". Same shape as the legacy preview pane's
    /// `symbol_summary` first line.
    pub symbol_summary: String,
    /// Same shape for footprint — "Footprint bound", "unbound", …
    pub footprint_summary: String,
}

pub struct PanelContext {
    /// Multi-project workspace — one entry per `LoadedProject`. Every
    /// project-aware panel reads from this Vec; the active project is
    /// the one with `is_active == true`.
    pub projects: Vec<ProjectPanelInfo>,
    pub sym_count: usize,
    pub wire_count: usize,
    pub label_count: usize,
    pub junction_count: usize,
    pub child_sheets: Vec<String>,
    pub has_schematic: bool,
    pub has_pcb: bool,
    pub paper_size: String,
    pub lib_symbol_count: usize,
    /// Library symbol names for Components panel.
    pub lib_symbol_names: Vec<String>,
    /// Placed symbols: (reference, value, footprint, lib_id).
    pub placed_symbols: Vec<(String, String, String, String)>,
    pub tokens: ThemeTokens,
    /// Active theme id. Feeds the icon registry so dock/panel SVGs tint
    /// to the theme's accent (see `crate::icons`).
    pub theme_id: signex_types::theme::ThemeId,
    // Live settings (synced from Signex on every update)
    pub unit: Unit,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub grid_size_mm: f32,
    pub visible_grid_mm: f32,
    pub snap_hotspots: bool,
    /// UI font family name (shown in settings; applies on restart).
    pub ui_font_name: String,
    /// User-editable component-class registry mirrored from
    /// `UiState.component_classes`. Read by the New Component modal's
    /// class dropdown so the picker reflects whatever the user has
    /// added / renamed / removed in Preferences ▸ Component Classes.
    pub component_classes: Vec<crate::fonts::ComponentClassEntry>,
    /// Canvas font family name (Iosevka by default; applies immediately).
    pub canvas_font_name: String,
    /// Canvas font size in px (applied immediately as a global scale).
    pub canvas_font_size: f32,
    /// Canvas font bold style flag.
    pub canvas_font_bold: bool,
    /// Canvas font italic style flag.
    pub canvas_font_italic: bool,
    /// Whether the canvas font picker popup is open.
    pub canvas_font_popup_open: bool,
    pub properties_tab: usize, // 0=General, 1=Parameters
    // Components panel
    pub standard_libraries: Vec<String>,
    pub active_library: Option<String>,
    /// Browser entries from the selected library or aggregated catalog.
    pub library_symbols: Vec<LibrarySymbolEntry>,
    /// Selected component lib_id.
    pub selected_component: Option<String>,
    /// (pin_number, pin_name, pin_type) for the selected component.
    pub selected_pins: Vec<(String, String, String)>,
    /// Full LibSymbol data for canvas preview.
    pub selected_lib_symbol: Option<signex_types::schematic::LibSymbol>,
    /// Height in px for the Components list section (details gets the rest).
    pub components_split: f32,
    /// Persistent project tree — toggle state survives across renders.
    pub project_tree: Vec<TreeNode>,
    /// Path of the currently single-clicked tree row, used to drive
    /// the row highlight independently of which document is active.
    /// Set on `TreeMsg::Select`, cleared when the tree resets.
    pub project_tree_selected: Option<Vec<usize>>,
    /// F15 — detail for the row currently selected in the active
    /// Library Browser tab. `Some` when (a) the active tab is a
    /// `LibraryBrowser(path)` AND (b) `library_browsers[path]
    /// .selected_row` is `Some`. `view_properties` reads this and
    /// renders the row's metadata + Pick Symbol / Pick Footprint
    /// buttons; outside of a Library Browser tab the field stays
    /// `None` and the panel falls through to its existing branches.
    pub library_row_detail: Option<LibraryRowDetail>,
    // Selection info for Properties panel
    /// How many items are currently selected.
    pub selection_count: usize,
    /// UUID of the single selected item (for property editing).
    pub selected_uuid: Option<uuid::Uuid>,
    /// Kind of the single selected item.
    pub selected_kind: Option<signex_types::schematic::SelectedKind>,
    /// Description of the selected item (for single selection).
    pub selection_info: Vec<(String, String)>,
    /// Transient numeric-input buffers for the drawing properties
    /// panel — keyed on DrawingFieldId. Keeps half-typed strings
    /// ("", "-", "2.") alive between rerenders so fields can be
    /// fully erased and retyped. Reset when selection changes.
    pub drawing_edit_buf: std::collections::HashMap<DrawingFieldId, String>,
    /// UUID that owns the current drawing_edit_buf. When
    /// `selected_uuid` changes the handler clears the buffer.
    pub drawing_edit_buf_for: Option<uuid::Uuid>,
    /// The live SchDrawing matching `selected_uuid` when a single
    /// drawing is selected. Feeds the mini preview canvas in the
    /// Properties panel so the shape renders true-to-life.
    pub selected_drawing: Option<signex_types::schematic::SchDrawing>,
    /// The live ChildSheet matching `selected_uuid` when a single
    /// hierarchical sheet is selected. Powers the editable
    /// border/fill colour pickers and stroke-width input.
    pub selected_child_sheet: Option<signex_types::schematic::ChildSheet>,
    /// Whether the border-colour picker overlay is open for the
    /// currently-selected child sheet.
    pub child_sheet_border_picker_open: bool,
    /// Whether the fill-colour picker overlay is open for the
    /// currently-selected child sheet.
    pub child_sheet_fill_picker_open: bool,
    /// Whether the user expanded the border picker into the full
    /// HSV / RGB ColorPicker (vs the default preset palette).
    pub child_sheet_border_advanced_open: bool,
    /// Whether the user expanded the fill picker into the full
    /// HSV / RGB ColorPicker (vs the default preset palette).
    pub child_sheet_fill_advanced_open: bool,
    /// Transient text-input buffer for the child sheet's stroke-width
    /// numeric field. `None` means "show the live value formatted";
    /// `Some(s)` keeps half-typed strings between rerenders.
    pub child_sheet_stroke_width_buf: Option<String>,
    /// Component search filter text.
    pub component_filter: String,
    /// Which sections are collapsed (by section name key).
    pub collapsed_sections: CollapsedSections,
    /// Pre-placement configuration (shown when Tab pressed during placement tool).
    pub pre_placement: Option<PrePlacementData>,
    /// Current diagnostics level resolved from SIGNEX_LOG / RUST_LOG.
    /// Flattened ERC diagnostics from the most recent Run-ERC pass.
    pub erc_diagnostics: Vec<ErcDiagnosticEntry>,
    /// Focused ERC diagnostic index used by prev/next navigation arrows.
    pub erc_focus_index: Option<usize>,
    pub diagnostics_level: String,
    /// Recent application diagnostics shown in the Messages panel.
    pub diagnostics: Vec<crate::diagnostics::DiagnosticEntry>,
    /// Selection filter state, shared with the Active Bar.
    pub selection_filters: std::collections::HashSet<crate::active_bar::SelectionFilter>,
    /// User-defined custom filter presets (capped at
    /// `crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT`). Edited from
    /// the Properties panel; surfaced as shortcut buttons in the Active
    /// Bar's filter dropdown.
    pub custom_filter_presets: Vec<crate::active_bar::CustomFilterPreset>,
    /// Index of the active preset tab in the Properties-panel editor
    /// (mirrored from `InteractionState`). Clamped on every sync.
    pub active_custom_filter_tab: usize,
    /// Page formatting mode (Template / Standard / Custom).
    pub page_format_mode: PageFormatMode,
    /// Vertical page margin zones.
    pub margin_vertical: u32,
    /// Horizontal page margin zones.
    pub margin_horizontal: u32,
    /// Page coordinate origin.
    pub page_origin: PageOrigin,
    /// Custom paper width in mm (only used when page_format_mode == Custom).
    pub custom_paper_w_mm: f32,
    /// Custom paper height in mm (only used when page_format_mode == Custom).
    pub custom_paper_h_mm: f32,
    /// Sheet background colour.
    pub sheet_color: SheetColor,
    /// When the active tab is a `.snxsym` standalone editor, this carries
    /// the symbol's display data so the right-dock Properties panel and
    /// the left-dock SCH-Library panel can render context-aware content
    /// without the in-tab editor having to embed its own properties pane.
    /// `None` for any other tab kind.
    pub symbol_editor: Option<SymbolEditorPanelContext>,
    /// v0.14.2 — context for the right-dock Properties panel when the
    /// active tab is a `.snxfpt` standalone footprint editor.
    /// Surfaces editor mode (Pads / Sketch / 3D View) + the current
    /// selection so the panel can switch its body between pad
    /// properties, sketch entity properties, or footprint defaults.
    /// `None` for any other tab kind.
    pub footprint_editor: Option<FootprintEditorPanelContext>,
    /// Per-file Git history snapshot for the right-dock History
    /// panel. Mirrored from [`crate::app::DocumentState::history`]
    /// each refresh; the panel reads it directly without holding a
    /// borrow into the document state.
    pub history: history::HistoryPanelState,
}

/// Context handed to the Properties panel when a `.snxfpt` editor
/// tab is active. Mirrors a small read-only slice of the live
/// `FootprintEditorState` — the panel never mutates this, edits flow
/// back through `LibraryMessage::PrimitiveEditorEvent` like every
/// other primitive editor.
#[derive(Debug, Clone)]
pub struct FootprintEditorPanelContext {
    /// Open `.snxfpt` path (the tab key).
    pub path: std::path::PathBuf,
    /// `Footprint::name` — surfaced as the panel header.
    pub footprint_name: String,
    /// `Footprint::version` — semver-style revision string.
    pub version: String,
    /// Current editor mode — drives the Properties panel body branch.
    pub mode_kind: FootprintModeKind,
    /// Number of baked pads on the footprint primitive.
    pub pad_count: usize,
    /// Number of sketch entities (Points + Lines + Arcs + Circles)
    /// when a sketch is present.
    pub sketch_entity_count: usize,
    /// Number of sketch constraints when a sketch is present.
    pub sketch_constraint_count: usize,
    /// Number of free DoF the most recent solve reported, plus
    /// elapsed_ms. `None` if no solve has run yet.
    pub last_solve: Option<FootprintSolveSummary>,
    /// Read-only summary of the selected pad — populated when in
    /// Pads mode and a pad is selected.
    pub selected_pad: Option<FootprintPadSummary>,
    /// v0.27 — total number of selected pads (primary + extras).
    /// Drives the multi-select "(N selected)" indicator in the
    /// Properties panel header. `1` for a single-select; `> 1` when
    /// the rubber band picked multiple pads or All-on-Layer / All
    /// fired.
    pub selected_pad_count: usize,
    /// Read-only summary of the primary selected sketch entity —
    /// populated when in Sketch mode and an entity is selected.
    pub selected_sketch_entity: Option<FootprintSketchEntitySummary>,
    /// Mirrors `editor.state.auto_fit_courtyard`. v0.14.2 — drives
    /// the Properties panel's Auto-fit Courtyard toggle (the
    /// equivalent active-bar button stays for quick access).
    pub auto_fit_courtyard: bool,
    /// v0.14.2 — every `.snxfpt` sibling inside the containing
    /// `.snxlib`'s `footprints/` directory. Drives the new Footprint
    /// Library panel (mirror of SCH Library). Empty when the active
    /// footprint lives outside any mounted library.
    pub library_siblings: Vec<FootprintLibSibling>,
    /// Display name of the containing `.snxlib` (file stem) — used
    /// in the Footprint Library panel breadcrumb. `None` for
    /// lone-file edits.
    pub library_stem: Option<String>,
    /// v0.18.8 — every footprint inside the active multi-footprint
    /// `.snxfpt` envelope. Mirror of one row in Altium's PCB Library
    /// panel. Drives the Footprint Library panel's primary list.
    pub internal_footprints: Vec<FootprintLibInternalRow>,
    /// v0.18.8 — single-click selection within the panel's internal
    /// footprint list. `None` until the user clicks a row. Drives
    /// the Place / Delete / Edit button enable state.
    pub internal_selected_idx: Option<usize>,
    /// v0.16.2 — Sketch parameter table (name → expression). Drives
    /// the Properties panel's Parameters section in Sketch mode.
    /// Empty when the footprint has no `SketchData` yet.
    pub sketch_parameters: Vec<(String, String)>,
    /// v0.16.2 — solver warnings from the most recent solve + bake.
    /// Drives the Properties panel's "Solve warnings" section.
    pub solve_warnings: Vec<String>,
    /// v0.16.2 — sketch-entity ID of the primary selection. Wired
    /// through so the Properties panel's Role pick_list can emit
    /// `FootprintSketchSetRole` with the right id.
    pub selected_sketch_entity_id: Option<signex_sketch::id::SketchEntityId>,
    /// v0.16.2 — current role of the primary selected sketch entity
    /// (or `Unassigned` when no entity is selected). Inspected via
    /// `current_role_of` against the entity's `*Attr` slots.
    pub selected_sketch_role: crate::library::messages::RoleTag,
    /// v0.16.2 — `true` when the primary selected sketch entity is a
    /// Point. Drives the "Pad role applies to Points only" hint on
    /// the Role pick_list.
    pub selected_sketch_is_point: bool,
    /// v0.16.3 — `true` when the Pads-mode tool is `PlacePad`. Drives
    /// visibility of the "Pad placement defaults" form on the
    /// Properties panel (designator override + size_x / size_y / side).
    pub placement_active: bool,
    /// v0.16.3 — `true` when TAB has paused click-publish during pad
    /// placement. Adds a "PAUSED — TAB to resume" hint to the form.
    pub placement_paused: bool,
    /// v0.16.3 — designator override for the next placed pad. `None`
    /// = use the auto-incrementing numeric designator.
    pub next_pad_designator_override: Option<String>,
    /// v0.16.3 — size_x of the next placed pad in mm.
    pub next_pad_size_x_mm: f64,
    /// v0.16.3 — size_y of the next placed pad in mm.
    pub next_pad_size_y_mm: f64,
    /// v0.16.3 — copper side for the next placed pad.
    pub next_pad_side: crate::library::editor::footprint::state::PadSide,
    /// v0.16.6 — rotation in degrees (CCW positive) for the next
    /// placed pad. Persists through `EditorPad.rotation_deg` →
    /// `Pad::rotation` so the saved file carries the value.
    pub next_pad_rotation_deg: f64,
    /// v0.20 — Altium-parity pad-stack defaults for the next placed
    /// pad. Mirrors `editor.state.next_pad_defaults.stack` for the
    /// Properties panel "Pad Stack" section. UI mutates these via
    /// `FpEditorSetNextPad*` messages; the dispatcher writes the
    /// value back so `add_pad_at` picks it up on the next click.
    pub next_pad_stack: crate::library::editor::footprint::state::PadStackUi,
    /// v0.20 — pad shape for the next placed pad (read out of
    /// `EditorPad::shape` after mint). Defaults to `Rect`.
    pub next_pad_shape: signex_library::PadShape,
    /// v0.20 — drill diameter for the next placed pad in mm. `None`
    /// = SMD pad (no hole). Drives the HOLE → Hole size row.
    pub next_pad_drill_diameter_mm: Option<f64>,
    /// v0.20 — drill slot length for the next placed pad in mm.
    /// `None` = round drill; `Some(l)` = oval slot of length l.
    /// Drives the HOLE → Shape pick_list (Round vs Slot) and the
    /// Slot length input visibility.
    pub next_pad_drill_slot_length_mm: Option<f64>,
    /// v0.20 — pad-template name for the next placed pad. Empty =
    /// no template.
    pub next_pad_template: String,
    /// v0.20 — pad-template library reference for the next placed
    /// pad. Empty = local.
    pub next_pad_template_library: String,
    /// v0.20 — top-side surface feature for the next placed pad.
    pub next_pad_feature_top: signex_sketch::attr::PadFeature,
    /// v0.20 — bottom-side surface feature for the next placed pad.
    pub next_pad_feature_bottom: signex_sketch::attr::PadFeature,
    /// v0.20 — test-point participation flags for the next placed
    /// pad. All `false` = not a test point.
    pub next_pad_testpoint: signex_sketch::attr::TestpointFlags,
    /// v0.20 — currently-active Pad Stack tab (Simple / Top-Middle-
    /// Bottom / Full Stack). Drives which body the Pad Stack section
    /// renders. UI-only state; not persisted to disk.
    pub pad_stack_tab: crate::library::editor::footprint::state::PadStackTab,
    /// v0.21 — Altium-parity electrical-type for the next placed pad.
    pub next_pad_electrical_type: signex_sketch::attr::ElectricalType,
    /// v0.21 — net assignment for the next placed pad.
    pub next_pad_net: String,
    /// v0.21 — locked flag for the next placed pad.
    pub next_pad_locked: bool,
    /// v0.21 — Pad mounting kind for the next placed pad.
    pub next_pad_kind: signex_library::PadKind,
    /// v0.21 — Altium-parity component-level fields. Surface in the
    /// empty-canvas Footprint summary form.
    pub footprint_description: String,
    pub footprint_default_designator: String,
    pub footprint_component_type: signex_library::primitive::footprint::ComponentType,
    pub footprint_height_mm: Option<f64>,
    /// v0.21 — Pad Hole detail fields surfaced for the Multi-Layer
    /// pad placement form.
    pub next_pad_hole_tolerance_plus_mm: Option<f64>,
    pub next_pad_hole_tolerance_minus_mm: Option<f64>,
    pub next_pad_hole_rotation_deg: Option<f64>,
    pub next_pad_copper_offset_x_mm: Option<f64>,
    pub next_pad_copper_offset_y_mm: Option<f64>,
    /// v0.16.4 — Pour-role sub-form values for the selected entity.
    /// `None` when the entity isn't a pour. Carries the net string,
    /// fill type, and a snapshot of thermal-relief defaults so the
    /// panel can show + edit each field.
    pub selected_pour: Option<PourSummary>,
    /// v0.16.4 — Keepout-role sub-form values for the selected
    /// entity. `None` when the entity isn't a keepout.
    pub selected_keepout: Option<KeepoutSummary>,
    /// v0.16.4 — BoardCutout-role sub-form values for the selected
    /// entity. `None` when the entity isn't a board cutout.
    pub selected_cutout: Option<CutoutSummary>,
    /// v0.21 — pad-role sub-form values for the selected sketch
    /// entity. `Some` when the entity carries a `PadAttr`.
    pub selected_sketch_pad: Option<SketchPadAttrSummary>,
    /// v0.17.0 — empty-canvas Snap Options. Surfaced on the
    /// Properties panel default branch (no selection) so the user
    /// can toggle each priority chain step.
    pub snap_options: crate::library::editor::footprint::state::SnapOptions,
    /// v0.18.13 — Altium 5-section Properties layout state.
    pub selection_filter: crate::library::editor::footprint::state::SelectionFilter,
    pub snap_subtab: crate::library::editor::footprint::state::SnapSubTab,
    pub snapping_mode: crate::library::editor::footprint::state::SnappingMode,
    /// v0.18.20 — Altium-style guide lines visible to the Guide
    /// Manager UI table. Cloned out of `editor.state.guides` so the
    /// panel can iterate them without holding a borrow into the
    /// document state.
    pub guides: Vec<crate::library::editor::footprint::state::Guide>,
    /// v0.18.21 — Altium grid table rows. Cloned out of
    /// `editor.state.grids`; the active row is `active_grid_idx`.
    pub grids: Vec<crate::library::editor::footprint::state::GridDef>,
    /// v0.18.21 — index into `grids` of the active row (mirror of
    /// `editor.state.active_grid_idx`).
    pub active_grid_idx: usize,
    /// v0.18.24 — selected silk-front graphic summary. `None` when
    /// no silk graphic is selected; the Properties panel only renders
    /// the silk-selection branch when this is `Some`.
    pub selected_silk_summary: Option<FootprintSelectedSilkSummary>,
    /// v0.25 polish — verbatim per-input buffers for Properties-panel
    /// numeric fields. Renderer reads `numeric_buffers.get(key)` and
    /// uses the literal buffer if present; falls back to formatting
    /// the canonical f64 otherwise. See
    /// [`crate::library::editor::footprint::state::FootprintEditorState::numeric_buffers`]
    /// for the buffer contract.
    pub numeric_buffers: std::collections::HashMap<String, String>,
    /// v0.23 — Array (Pattern) summary surfaced when the selected
    /// sketch entity is the `source` of an array. Drives the
    /// Properties panel "Pattern" sub-section.
    pub selected_array: Option<ArraySummary>,
    /// v0.24 Phase 3 (Track A2) — parametric handle summary for the
    /// selected pad. Empty when the pad has no `shape_params` bindings
    /// (e.g. legacy pads minted before v0.24, or pads whose shape has
    /// no parametric handles like Rect / Oval). One entry per
    /// (feature_key → parameter) binding; the Properties panel
    /// renders one editable row per entry so the user can edit the
    /// shared sketch parameter without entering Sketch mode.
    pub selected_pad_shape_params: Vec<PadShapeParamSummary>,
}

/// v0.24 Phase 3 (Track A2) — surface entry for one parametric pad
/// handle. Carries the feature key (e.g. `"corner_r"`, `"diameter"`),
/// the resolved sketch parameter name, the current expression string
/// (read out of `sketch.parameters`), and a UI label so the
/// Properties panel can render a localised row label without each
/// editor having to repeat the mapping.
#[derive(Debug, Clone)]
pub struct PadShapeParamSummary {
    /// Feature key as stored on `EditorPad.shape_params`
    /// (e.g. `"corner_r"`, `"diameter"`).
    pub key: String,
    /// Display label for the Properties panel row.
    pub label: String,
    /// Resolved parameter name in `sketch.parameters` (the value
    /// stored under `key` in `pad.shape_params`).
    pub parameter_name: String,
    /// Current expression string (e.g. `"0.25mm"`). Empty when the
    /// parameter is missing from `sketch.parameters` (defensive — the
    /// row still renders so the user can re-bind via direct edit).
    pub current_expr: String,
}

/// v0.16.4 — Pour role properties surfaced on the Properties panel.
#[derive(Debug, Clone)]
pub struct PourSummary {
    pub net: Option<String>,
    pub fill_type: signex_sketch::attr::PourFillType,
    pub priority: u32,
}

/// v0.16.4 — Keepout role properties surfaced on the Properties panel.
#[derive(Debug, Clone)]
pub struct KeepoutSummary {
    pub no_routing: bool,
    pub no_components: bool,
    pub no_copper: bool,
    pub no_vias: bool,
    pub no_drilling: bool,
    pub no_pours: bool,
}

/// v0.16.4 — BoardCutout role properties surfaced on the Properties panel.
#[derive(Debug, Clone)]
pub struct CutoutSummary {
    pub edge_radius_expr: Option<String>,
    pub through: bool,
}

/// v0.23 — Array (Pattern) properties surfaced on the Properties panel
/// when the selected sketch entity is the source of an
/// [`signex_sketch::array::Array`]. The handler resolves the array by
/// `array_id`, mutates the matching field, then runs solve+bake.
#[derive(Debug, Clone)]
pub struct ArraySummary {
    pub array_id: signex_sketch::array::ArrayId,
    pub kind: ArrayKindSummary,
    pub numbering: NumberingSchemeKindUi,
    /// `true` when the polar centre re-pick is active — the next
    /// sketch click on a Point sets `array.center`.
    pub repicking_polar_center: bool,
    /// v0.25 polish — when `numbering == BgaRowCol`, this carries the
    /// BGA-specific config (skip_letters / start_row / start_col) so
    /// the Properties panel can surface editable rows for each. `None`
    /// for Linear / Explicit numbering schemes.
    pub bga_config: Option<BgaConfigSummary>,
}

/// v0.25 polish — surface for BGA numbering scheme parameters.
/// Mirror of [`signex_sketch::array::NumberingScheme::BgaRowCol`].
#[derive(Debug, Clone)]
pub struct BgaConfigSummary {
    /// IPC-7351 letter-skip convention (omits I/O/Q/S/X/Z to avoid
    /// confusion with numerals). Default `true` matches Altium.
    pub skip_letters: bool,
    /// First row letter (e.g. `'A'`). Drives the row labels in the
    /// baked replicas.
    pub start_row: char,
    /// First column number (e.g. `1`).
    pub start_col: u32,
}

#[derive(Debug, Clone)]
pub enum ArrayKindSummary {
    Linear {
        count_expr: String,
        dx_expr: String,
        dy_expr: String,
    },
    Grid {
        nx_expr: String,
        ny_expr: String,
        dx_expr: String,
        dy_expr: String,
        /// Empty string when the array has no `GridDepopulation`.
        mask_expr: String,
        /// Per-instance suppression carried alongside `mask_expr`. The
        /// bake honours both — a (i, j) pair in this list skips the
        /// instance regardless of the mask predicate. Stored as
        /// `(i, j)` 0-based row/column indices. Drives the v0.23 B5
        /// per-instance checkbox grid in the Properties panel.
        suppressed_instances: Vec<(u32, u32)>,
        /// Snapshot of `nx` after evaluation — drives the checkbox
        /// grid's column count. `None` when the expression isn't
        /// numeric (e.g. references a parameter that isn't bound).
        nx_value: Option<u32>,
        /// Snapshot of `ny` — checkbox grid's row count.
        ny_value: Option<u32>,
    },
    Polar {
        count_expr: String,
        sweep_angle_expr: String,
        center_position_mm: Option<[f64; 2]>,
        mask_expr: String,
        /// Per-instance suppression for Polar; the j coordinate is
        /// always 0 (Polar arrays are 1-D).
        suppressed_instances: Vec<u32>,
        /// Snapshot of the evaluated `count` — drives the checkbox row
        /// length. `None` when the expression isn't numeric.
        count_value: Option<u32>,
    },
}

/// v0.23 — Numbering scheme kind for the Properties panel pick_list.
/// Mirrors [`signex_sketch::array::NumberingScheme`]'s tag. The handler
/// preserves the inner expression fields when flipping kinds where
/// possible (e.g. switching to LinearIncrement keeps any prior
/// start/step expressions; switching to Explicit clears them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberingSchemeKindUi {
    LinearIncrement,
    BgaRowCol,
    Explicit,
}

impl NumberingSchemeKindUi {
    pub const ALL: [Self; 3] = [
        Self::LinearIncrement,
        Self::BgaRowCol,
        Self::Explicit,
    ];
}

impl std::fmt::Display for NumberingSchemeKindUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::LinearIncrement => "Linear (1, 2, 3, …)",
            Self::BgaRowCol => "BGA (A1, A2, …)",
            Self::Explicit => "Explicit list",
        })
    }
}

/// v0.23 — Field discriminator for [`PanelMsg::FpEditorEditArrayParam`].
/// Each variant maps to a single text-input on one [`ArrayKindSummary`]
/// branch; the handler uses the variant to disambiguate the target
/// field when mutating the array in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayParamField {
    LinearCountExpr,
    LinearDxExpr,
    LinearDyExpr,
    GridNxExpr,
    GridNyExpr,
    GridDxExpr,
    GridDyExpr,
    PolarCountExpr,
    PolarSweepAngleExpr,
    /// Maps to `GridDepopulation.mask_expr` for both Grid and Polar
    /// arrays. Empty string clears the depopulation entirely (set
    /// `Option::None` on the array).
    MaskExpr,
}

/// v0.16.4 — discrete bit identifier for [`KeepoutSummary`] flags.
/// PanelMsg-friendly so the Keepout sub-form's checklist can carry
/// "which flag" without smuggling a closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepoutKindFlag {
    NoRouting,
    NoComponents,
    NoCopper,
    NoVias,
    NoDrilling,
    NoPours,
}

/// v0.17.0 — discrete identifier for the four `SnapOptions` flags.
/// Used by the Properties-panel snap checklist to carry "which
/// flag" through `PanelMsg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapOptionFlag {
    PointHit,
    HorizontalVertical,
    Angle,
    Grid,
    // v0.13 — Altium "Objects for snapping" set.
    TrackVertices,
    TrackLines,
    ArcCenters,
    Intersections,
    PadCenters,
    PadVertices,
    PadEdges,
    ViaCenters,
    Texts,
    Regions,
    FootprintOrigins,
    Body3dPoints,
    /// Independent target-category toggles (Altium PCB Library
    /// editor): each can be on simultaneously.
    SnapToGrids,
    SnapToGuides,
    SnapToAxes,
}

/// One row in the Footprint Library panel — a sibling `.snxfpt`
/// living next to the active footprint inside the same `.snxlib`'s
/// `footprints/` directory.
#[derive(Debug, Clone)]
pub struct FootprintLibSibling {
    /// Absolute path to the `.snxfpt` on disk.
    pub path: std::path::PathBuf,
    /// File stem ("SOIC-8") — falls back to filename when the stem
    /// can't be resolved.
    pub display_name: String,
    /// `true` when this sibling is the currently-open footprint
    /// (the active tab). The panel renders this row with the
    /// selection highlight.
    pub is_active: bool,
}

/// v0.18.8 — one row in the Footprint Library panel representing a
/// footprint *inside* the active `.snxfpt` envelope. Mirror of one
/// component row in Altium's PCB Library panel.
#[derive(Debug, Clone)]
pub struct FootprintLibInternalRow {
    /// `Footprint::name` — primary display label.
    pub name: String,
    /// Pad count, surfaced in a secondary column.
    pub pad_count: usize,
    /// `true` when this index matches the editor's `active_idx` —
    /// the row that the canvas / Properties panel currently shows.
    pub is_active: bool,
}

/// Editor mode mirror — kept in this crate so the panel doesn't need
/// to import `library::editor::footprint::state::EditorMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintModeKind {
    Pads,
    Sketch,
    View3d,
}

#[derive(Debug, Clone)]
pub struct FootprintSolveSummary {
    pub iterations: usize,
    pub elapsed_ms: u64,
    pub final_residual_norm: f64,
    pub over_constraint_count: usize,
    /// v0.22 Phase E3+E4 — per-over-constraint summary so the
    /// Properties panel can list the conflicts and the user can
    /// click each to focus the canvas on the offending geometry.
    /// Sorted descending by `residual_magnitude` so the worst
    /// offender is first. Empty when `over_constraint_count == 0`.
    pub over_constraints: Vec<OverConstraintSummary>,
}

#[derive(Debug, Clone)]
pub struct OverConstraintSummary {
    /// v0.23 — Constraint ID used by the canvas to render the
    /// specific row at full red while everything else (including
    /// other over-constraints) dims. Drives per-row hover precision
    /// in the Properties panel "Conflicts" list.
    pub constraint_id: signex_sketch::id::ConstraintId,
    /// Kind label — "Coincident", "DistancePtPt", "Horizontal", etc.
    /// Static string avoids allocating per-row.
    pub kind_label: &'static str,
    /// Post-solve residual magnitude. The LM iteration drives this
    /// below `Solver::tolerance` for satisfiable constraints; values
    /// above `RANK_TOL` indicate the constraint couldn't be met
    /// alongside the others.
    pub residual_magnitude: f64,
    /// First Point entity the constraint touches. Click → select
    /// this Point so the canvas pans and the constraint icon
    /// rendered in red sits in view. `None` for constraints with
    /// no Point endpoints (rare — Fixed pseudo-rows).
    pub focus_entity_id: Option<signex_sketch::id::SketchEntityId>,
}

#[derive(Debug, Clone)]
pub struct FootprintPadSummary {
    pub idx: usize,
    pub number: String,
    pub kind_label: &'static str,
    pub shape_label: &'static str,
    pub size_mm: [f64; 2],
    pub position_mm: [f64; 2],
    pub rotation_deg: f64,
    pub layer_count: usize,
    pub has_drill: bool,
    /// v0.20 — full snapshot of editable pad state surfaced for the
    /// selected-pad Properties panel. Mirrors the same fields the
    /// next-pad form binds to so the selected-pad branch can render
    /// the same Properties / Pad Stack / Pad Features sections.
    pub side: crate::library::editor::footprint::state::PadSide,
    pub shape: signex_library::PadShape,
    pub kind: signex_library::PadKind,
    pub drill_diameter_mm: Option<f64>,
    pub stack: crate::library::editor::footprint::state::PadStackUi,
    pub feature_top: signex_sketch::attr::PadFeature,
    pub feature_bottom: signex_sketch::attr::PadFeature,
    pub testpoint: signex_sketch::attr::TestpointFlags,
    pub template: String,
    pub template_library: String,
    /// v0.21 — Altium-parity electrical-type.
    pub electrical_type: signex_sketch::attr::ElectricalType,
    /// v0.21 — net assignment.
    pub net: String,
    /// v0.21 — locked flag.
    pub locked: bool,
    /// v0.21 — Pad Hole detail fields for the selected pad.
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    pub hole_rotation_deg: Option<f64>,
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
}

/// v0.18.24 — Read-only summary of the currently-selected silk-front
/// v0.21 — sketch-mode pad attribute snapshot. Mirrors the new
/// fields we added to `PadAttr` so the sketch-entity Properties
/// branch can render an editable Pad Attributes section.
#[derive(Debug, Clone)]
pub struct SketchPadAttrSummary {
    pub id: signex_sketch::id::SketchEntityId,
    pub electrical_type: signex_sketch::attr::ElectricalType,
    pub net: String,
    pub locked: bool,
    pub template: String,
    pub template_library: String,
    pub feature_top: signex_sketch::attr::PadFeature,
    pub feature_bottom: signex_sketch::attr::PadFeature,
    pub testpoint: signex_sketch::attr::TestpointFlags,
    pub thermal_relief: bool,
    pub mask_top_tented: bool,
    pub mask_bottom_tented: bool,
    pub paste_top_enabled: bool,
    pub paste_bottom_enabled: bool,
    pub corner_radius_pct: Option<f64>,
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    pub hole_rotation_deg: Option<f64>,
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
    /// `true` when the pad has a drill spec (i.e. THT / NPT). Used
    /// to gate Hole-detail UI rows.
    pub has_drill: bool,
}

/// graphic (`FpGraphic` in `silk_f`). Drives the Properties panel's
/// silk-selection branch with full per-kind editable fields.
#[derive(Debug, Clone)]
pub struct FootprintSelectedSilkSummary {
    pub idx: usize,
    pub kind_label: &'static str,
    /// Stroke width in mm.
    pub stroke_width_mm: f64,
    /// `true` = solid fill; `false` = outline only.
    pub filled: bool,
    /// Per-kind geometry — only the field set matching `kind_label`
    /// is meaningful at any given time. `None` slots stay None.
    pub kind: SilkKindGeometry,
}

/// v0.21 — per-`FpGraphicKind` editable geometry. We surface
/// dedicated editable forms only for Line (Track) and Text (String);
/// Arc / Rectangle / Circle / Polygon collapse to `Other` and get a
/// minimal banner pointing the user at sketch mode for parametric
/// editing.
#[derive(Debug, Clone)]
pub enum SilkKindGeometry {
    Line { from_mm: [f64; 2], to_mm: [f64; 2] },
    Text {
        position_mm: [f64; 2],
        content: String,
        size_mm: f64,
    },
    /// Fallback for Arc / Rectangle / Circle / Polygon — the
    /// Properties panel surfaces stroke width / filled / locked /
    /// delete and a hint to switch to Sketch mode for full editing.
    Other,
}

#[derive(Debug, Clone)]
pub struct FootprintSketchEntitySummary {
    /// Display label for the entity kind ("Point", "Line", "Arc",
    /// "Circle"). Wrapper avoids importing the sketch enum.
    pub kind_label: &'static str,
    /// Coordinates in mm — for Points only; None for Lines/Arcs/Circles.
    pub position_mm: Option<[f64; 2]>,
    /// Number of attached constraints touching this entity.
    pub attached_constraint_count: usize,
    /// `true` if this is a construction entity (solver scaffolding
    /// only, no baked geometry).
    pub construction: bool,
    /// v0.22 Phase A3 — solver-state colour for the entity. `Some` for
    /// Points (looked up in `last_solve.colours`); `None` for other
    /// entity kinds whose DOF state is implicitly the min of their
    /// endpoints'. Drives the "DOF" row in the Properties panel.
    pub dof_state: Option<signex_sketch::solver::dof::DofColor>,
}

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

/// Sheet background colour presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetColor {
    #[default]
    Black,
    White,
    DarkGray,
    LightGray,
    Cream,
}

impl SheetColor {
    pub fn to_color(self) -> iced::Color {
        match self {
            SheetColor::Black => iced::Color::from_rgb8(0x14, 0x14, 0x14),
            SheetColor::White => iced::Color::WHITE,
            SheetColor::DarkGray => iced::Color::from_rgb8(0x2A, 0x2A, 0x2A),
            SheetColor::LightGray => iced::Color::from_rgb8(0xD0, 0xD0, 0xD0),
            SheetColor::Cream => iced::Color::from_rgb8(0xFB, 0xF4, 0xE0),
        }
    }
    pub const ALL: &'static [SheetColor] = &[
        SheetColor::Black,
        SheetColor::White,
        SheetColor::DarkGray,
        SheetColor::LightGray,
        SheetColor::Cream,
    ];
}

impl std::fmt::Display for SheetColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SheetColor::Black => "Black",
            SheetColor::White => "White",
            SheetColor::DarkGray => "Dark Gray",
            SheetColor::LightGray => "Light Gray",
            SheetColor::Cream => "Cream",
        })
    }
}

/// Altium-style page formatting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageFormatMode {
    Template,
    #[default]
    Standard,
    Custom,
}

/// Altium-style page coordinate origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageOrigin {
    UpperLeft,
    #[default]
    LowerLeft,
}

impl std::fmt::Display for PageOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PageOrigin::UpperLeft => "Upper Left",
            PageOrigin::LowerLeft => "Lower Left",
        })
    }
}

/// Supported paper sizes (Altium-compatible subset).
pub const PAPER_SIZES: &[&str] = &[
    "A0", "A1", "A2", "A3", "A4", "A5", "B5", "Letter", "Legal", "Tabloid",
];

/// (width_mm, height_mm) for a paper size string.
pub fn paper_dimensions(size: &str) -> (f32, f32) {
    match size {
        "A0" => (1189.0, 841.0),
        "A1" => (841.0, 594.0),
        "A2" => (594.0, 420.0),
        "A3" => (420.0, 297.0),
        "A4" => (297.0, 210.0),
        "A5" => (210.0, 148.0),
        "B5" => (257.0, 182.0),
        "Letter" => (279.4, 215.9),
        "Legal" => (355.6, 215.9),
        "Tabloid" => (431.8, 279.4),
        _ => (297.0, 210.0),
    }
}

/// Pre-placement configuration data — shown in Properties panel when Tab pressed.
#[derive(Debug, Clone)]
pub struct PrePlacementData {
    /// Which tool is being configured.
    pub tool_name: String,
    /// Semantic kind so the panel can render the right field set.
    pub kind: PrePlacementKind,
    /// Net label / text note text.
    pub label_text: String,
    /// Component designator override.
    pub designator: String,
    /// Rotation (degrees).
    pub rotation: f64,
    /// Font family (cosmetic until font switching ships).
    pub font: String,
    /// Font size in points (10 pt = Altium default).
    pub font_size_pt: u32,
    /// Horizontal justification.
    pub justify_h: signex_types::schematic::HAlign,
    /// Vertical justification (TextNote / Component fields).
    pub justify_v: signex_types::schematic::VAlign,
    /// Style toggles (currently cosmetic — engine wiring tracks v0.7+).
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Most-recent cursor world position (for the X/Y readout).
    pub cursor_x_mm: f64,
    pub cursor_y_mm: f64,
    /// Stroke width for the shape tools (Line / Rect / Circle / Arc /
    /// Polygon). 0 = Standard default ≈ 0.15 mm.
    pub shape_width_mm: f64,
    /// Fill style for shapes that support it (Rect / Circle / Polygon).
    pub shape_fill: signex_types::schematic::FillType,
}

/// Stable identifiers for every numeric drawing-field editor so the
/// panel keeps a transient string buffer per field across rerenders.
/// Erasing a text_input leaves an empty string in the buffer until
/// the user types a valid f64, at which point UpdateDrawingEdit fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawingFieldId {
    LineStartX,
    LineStartY,
    LineEndX,
    LineEndY,
    LineWidth,
    RectStartX,
    RectStartY,
    RectWidth,
    RectHeight,
    RectBorder,
    CircleCenterX,
    CircleCenterY,
    CircleRadius,
    CircleBorder,
    ArcCenterX,
    ArcCenterY,
    ArcRadius,
    ArcStartAngle,
    ArcEndAngle,
    ArcWidth,
    PolyBorder,
}

/// Distinguishes placement flavors so the pre-placement form only shows
/// fields relevant to what the user is about to drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePlacementKind {
    Wire,
    Bus,
    BusEntry,
    NoConnect,
    NetLabel,
    GlobalPort,
    HierPort,
    PowerPort,
    TextNote,
    Component,
    Line,
    Rectangle,
    Circle,
    Arc,
    Polygon,
    Other,
}

/// Panel-level message wrapping widget messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PanelMsg {
    Tree(TreeMsg),
    SetUnit(Unit),
    RunErc,
    /// v0.14.2 — Properties panel toggle for the active footprint
    /// editor's Auto-fit Courtyard setting. Routes through
    /// `handle_dock_sch_library_message` (same dispatcher every other
    /// `Sym*` / `FpEditor*` panel msg uses) which resolves the active
    /// footprint editor by tab and flips its `auto_fit_courtyard`
    /// flag via the existing `FootprintToggleAutoFit` path.
    FpEditorToggleAutoFitCourtyard,
    /// v0.16.2 — Properties-panel Role pick_list emit. Routed
    /// through the dock handler which forwards to
    /// `LibraryMessage::PrimitiveEditorEvent { ... FootprintSketchSetRole }`
    /// keyed on the active footprint editor tab.
    FpEditorSetRole {
        id: signex_sketch::id::SketchEntityId,
        role: crate::library::messages::RoleTag,
    },
    /// v0.16.2 — Properties-panel Parameter row text input. Routed
    /// through the dock handler which forwards to
    /// `LibraryMessage::PrimitiveEditorEvent { ... FootprintSketchEditParameter }`.
    FpEditorEditParameter {
        name: String,
        expr: String,
    },
    /// v0.16.3 — Properties-panel "Pad placement defaults" form
    /// updates. The handler mutates `editor.state.next_pad_defaults`
    /// directly so the next `add_pad_at` picks up the new values.
    FpEditorSetNextPadDesignator(String),
    FpEditorSetNextPadSizeX(String),
    FpEditorSetNextPadSizeY(String),
    FpEditorSetNextPadSide(crate::library::editor::footprint::state::PadSide),
    /// v0.16.6 — Properties-panel rotation input for the next placed
    /// pad. String-typed so the user can erase / type freely.
    FpEditorSetNextPadRotation(String),
    /// v0.16.6 — Properties-panel rotation input for the SELECTED
    /// pad in Pads mode. Mutates `state.pads[idx].rotation_deg`
    /// directly + dirty-marks the tab.
    FpEditorSetSelectedPadRotation {
        idx: usize,
        value: String,
    },
    /// v0.20 — Altium-parity Pad Properties / Pad Stack / Pad Features
    /// form for the next placed pad. Each variant maps to one row in
    /// the right-dock Properties panel; the dispatcher writes the
    /// parsed value into `editor.state.next_pad_defaults` (or the
    /// matching sub-struct) so the next `add_pad_at` picks it up.
    /// String-typed inputs preserve the per-field typing buffer
    /// behaviour we use for size_x / size_y / rotation.
    FpEditorSetNextPadShape(signex_library::PadShape),
    FpEditorSetNextPadKind(signex_library::PadKind),
    FpEditorSetNextPadDrillDiameter(String),
    FpEditorSetNextPadDrillSlotLength(String),
    FpEditorSetNextPadCornerRadiusPct(String),
    FpEditorSetNextPadTemplate(String),
    FpEditorSetNextPadTemplateLibrary(String),
    FpEditorSetNextPadPasteMarginTop(String),
    FpEditorSetNextPadPasteMarginBottom(String),
    FpEditorToggleNextPadPasteEnabledTop(bool),
    FpEditorToggleNextPadPasteEnabledBottom(bool),
    FpEditorSetNextPadMaskMarginTop(String),
    FpEditorSetNextPadMaskMarginBottom(String),
    FpEditorToggleNextPadMaskTentedTop(bool),
    FpEditorToggleNextPadMaskTentedBottom(bool),
    FpEditorToggleNextPadThermalRelief(bool),
    FpEditorSetNextPadFeatureTop(signex_sketch::attr::PadFeature),
    FpEditorSetNextPadFeatureBottom(signex_sketch::attr::PadFeature),
    FpEditorToggleNextPadTestpointTopAssembly(bool),
    FpEditorToggleNextPadTestpointTopFab(bool),
    FpEditorToggleNextPadTestpointBottomAssembly(bool),
    FpEditorToggleNextPadTestpointBottomFab(bool),
    /// v0.20 — Altium-parity Pad Properties / Pad Stack / Pad Features
    /// editing for the SELECTED pad. Each handler mutates
    /// `state.pads[idx]` (and dirty-marks the editor + syncs the
    /// primitive). String-typed numeric inputs preserve the
    /// per-field typing buffer behaviour.
    FpEditorSetSelectedPadDesignator { idx: usize, value: String },
    FpEditorSetSelectedPadSide { idx: usize, side: crate::library::editor::footprint::state::PadSide },
    FpEditorSetSelectedPadShape { idx: usize, shape: signex_library::PadShape },
    FpEditorSetSelectedPadKind { idx: usize, kind: signex_library::PadKind },
    FpEditorSetSelectedPadSizeX { idx: usize, value: String },
    FpEditorSetSelectedPadSizeY { idx: usize, value: String },
    FpEditorSetSelectedPadDrillDiameter { idx: usize, value: String },
    FpEditorSetSelectedPadDrillSlotLength { idx: usize, value: String },
    FpEditorSetSelectedPadCornerRadiusPct { idx: usize, value: String },
    FpEditorSetSelectedPadTemplate { idx: usize, value: String },
    FpEditorSetSelectedPadTemplateLibrary { idx: usize, value: String },
    FpEditorSetSelectedPadPasteMarginTop { idx: usize, value: String },
    FpEditorSetSelectedPadPasteMarginBottom { idx: usize, value: String },
    FpEditorToggleSelectedPadPasteEnabledTop { idx: usize, value: bool },
    FpEditorToggleSelectedPadPasteEnabledBottom { idx: usize, value: bool },
    FpEditorSetSelectedPadMaskMarginTop { idx: usize, value: String },
    FpEditorSetSelectedPadMaskMarginBottom { idx: usize, value: String },
    FpEditorToggleSelectedPadMaskTentedTop { idx: usize, value: bool },
    FpEditorToggleSelectedPadMaskTentedBottom { idx: usize, value: bool },
    FpEditorToggleSelectedPadThermalRelief { idx: usize, value: bool },
    FpEditorSetSelectedPadFeatureTop { idx: usize, value: signex_sketch::attr::PadFeature },
    FpEditorSetSelectedPadFeatureBottom { idx: usize, value: signex_sketch::attr::PadFeature },
    FpEditorToggleSelectedPadTestpointTopAssembly { idx: usize, value: bool },
    FpEditorToggleSelectedPadTestpointTopFab { idx: usize, value: bool },
    FpEditorToggleSelectedPadTestpointBottomAssembly { idx: usize, value: bool },
    FpEditorToggleSelectedPadTestpointBottomFab { idx: usize, value: bool },
    /// v0.20 — switch the Pad Stack section's tab (Simple /
    /// Top-Middle-Bottom / Full Stack). UI-only; mutates
    /// `editor.state.pad_stack_tab`.
    FpEditorSetPadStackTab(crate::library::editor::footprint::state::PadStackTab),
    /// v0.21 — Altium-parity Net / Locked / Electrical Type fields
    /// for both placement-defaults and selected-pad targets.
    FpEditorSetNextPadElectricalType(signex_sketch::attr::ElectricalType),
    FpEditorSetNextPadNet(String),
    FpEditorToggleNextPadLocked(bool),
    FpEditorSetSelectedPadElectricalType {
        idx: usize,
        value: signex_sketch::attr::ElectricalType,
    },
    FpEditorSetSelectedPadNet {
        idx: usize,
        value: String,
    },
    FpEditorToggleSelectedPadLocked {
        idx: usize,
        value: bool,
    },
    /// v0.21 — Footprint (component-level) edits.
    FpEditorSetFootprintDescription(String),
    FpEditorSetFootprintDefaultDesignator(String),
    FpEditorSetFootprintComponentType(signex_library::primitive::footprint::ComponentType),
    FpEditorSetFootprintHeight(String),
    /// v0.21 — Selected silk graphic edits (Line + Text only;
    /// Arc/Region/Fill/etc are sketch-mode-authored).
    FpEditorSetSilkLineFromX(String),
    FpEditorSetSilkLineFromY(String),
    FpEditorSetSilkLineToX(String),
    FpEditorSetSilkLineToY(String),
    FpEditorSetSilkTextPositionX(String),
    FpEditorSetSilkTextPositionY(String),
    FpEditorSetSilkTextSize(String),
    FpEditorSetSilkStrokeWidth(String),
    FpEditorToggleSilkFilled(bool),
    /// v0.21 — Pad Hole detail fields (Multi-Layer only).
    FpEditorSetNextPadHoleTolerancePlus(String),
    FpEditorSetNextPadHoleToleranceMinus(String),
    FpEditorSetNextPadHoleRotation(String),
    FpEditorSetNextPadCopperOffsetX(String),
    FpEditorSetNextPadCopperOffsetY(String),
    /// v0.21 — Plated toggle on the Pad Hole row. `true` = THT
    /// (plated), `false` = NPT (non-plated).
    FpEditorToggleNextPadPlated(bool),
    /// v0.21 — Selected-pad hole-detail mirror.
    FpEditorSetSelectedPadHoleTolerancePlus { idx: usize, value: String },
    FpEditorSetSelectedPadHoleToleranceMinus { idx: usize, value: String },
    FpEditorSetSelectedPadHoleRotation { idx: usize, value: String },
    FpEditorSetSelectedPadCopperOffsetX { idx: usize, value: String },
    FpEditorSetSelectedPadCopperOffsetY { idx: usize, value: String },
    FpEditorToggleSelectedPadPlated { idx: usize, value: bool },
    /// v0.21 — Sketch-mode pad attribute edits. Mutate the `PadAttr`
    /// on the selected sketch entity (identified by SketchEntityId)
    /// and re-run solve+bake. Mirrors the new pad fields surfaced in
    /// Pads-mode but addressed by the sketch entity rather than the
    /// flat-pad index.
    FpEditorSetSketchPadElectricalType {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::ElectricalType,
    },
    FpEditorSetSketchPadNet {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorToggleSketchPadLocked {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorSetSketchPadTemplate {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadTemplateLibrary {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadFeatureTop {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PadFeature,
    },
    FpEditorSetSketchPadFeatureBottom {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PadFeature,
    },
    FpEditorToggleSketchPadTestpointTopAssembly {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadTestpointTopFab {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadTestpointBottomAssembly {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadTestpointBottomFab {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadThermalRelief {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadMaskTentedTop {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadMaskTentedBottom {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadPasteEnabledTop {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadPasteEnabledBottom {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorSetSketchPadHoleTolerancePlus {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadHoleToleranceMinus {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadHoleRotation {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadCopperOffsetX {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadCopperOffsetY {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadCornerRadiusPct {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    /// v0.21 — "Edit in Sketch" jump from a selected pad to its
    /// backing sketch entity. Switches editor mode to Sketch and
    /// selects the entity. No-op when the pad has no
    /// `sketch_entity_id` (placed before sketch-mode auto-mint).
    FpEditorEditPadInSketch { pad_idx: usize },
    /// v0.24 Phase 3 (Track A2) — Properties-panel parametric handle
    /// row edit. The handler looks up `pad.shape_params[key]` to
    /// resolve the bound parameter name, writes `value` into
    /// `sketch.parameters[parameter_name]`, then dispatches a sketch
    /// `ForceRebuild` so the solver re-runs and every entity bound to
    /// that parameter (e.g. all 4 corner Arcs of a RoundRect) updates
    /// in lockstep.
    FpEditorEditPadShapeParam {
        pad_idx: usize,
        key: String,
        value: String,
    },
    /// v0.24 Phase 3 (Track A3) — sketch-canvas right-click action.
    /// "Unlink corner radius" mints a fresh per-corner parameter and
    /// rebinds the clicked Arc to it so the user can override that
    /// one corner independently. The other three corners stay on the
    /// shared `corner_r` parameter. No-op when the Arc isn't part of
    /// any pad's `shape_params` graph.
    FpEditorUnlinkCornerRadius {
        arc_entity_id: signex_sketch::id::SketchEntityId,
    },
    /// v0.22 Phase D6 — Mirror of `FpEditorEditPadInSketch` going the
    /// other direction. From a sketch entity carrying a `PadAttr`,
    /// switch to Pads mode and select the EditorPad whose
    /// `sketch_entity_id` matches this id. No-op when no pad has
    /// this entity as its backing point.
    FpEditorEditSketchPadInPads {
        id: signex_sketch::id::SketchEntityId,
    },
    /// v0.22 Phase E3+E4 — Properties-panel "Conflicts (worst first)"
    /// over-constrained constraint row. Click → select the row's
    /// focus entity in the sketch so the canvas re-renders with the
    /// constraint icon highlighted. The handler dispatches the
    /// equivalent `FootprintSketchSelect` library message.
    FpEditorSelectSketchEntity {
        id: signex_sketch::id::SketchEntityId,
    },
    /// v0.22 Phase 8.5 — Right-dock History panel "Restore this
    /// version" button. The handler resolves the active tab's
    /// owning project, opens `LocalGitProjectAdapter`, and runs
    /// `restore_at(rel_path, oid)` to overwrite the working-tree
    /// file with the historical blob. Marks the file dirty so the
    /// next save commits the restored content.
    HistoryRestoreClicked {
        sha: String,
    },
    /// v0.22 Phase E3+E4 polish — Hover state for the Properties
    /// panel's "Conflicts (worst first)" list. `true` on row
    /// `on_enter`, `false` on `on_exit`. The handler flips
    /// `editor.state.hovered_over_constraint` between
    /// v0.22 Phase E3+E4 → v0.23 — Per-row hover on a Properties
    /// panel "Conflicts" list row. `Some(constraint_id)` highlights
    /// the specific constraint at full red and dims everything else
    /// (including other over-constraints) so the user can isolate a
    /// single offender. `None` clears the isolation back to the
    /// default rendering.
    FpEditorHoverOverConstraint {
        constraint: Option<signex_sketch::id::ConstraintId>,
    },
    /// v0.16.4 — Pour-role sub-form. The handler mutates the
    /// selected entity's `pour` attr and runs solve+bake.
    FpEditorSetPourNet {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetPourFillType {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PourFillType,
    },
    FpEditorSetPourPriority {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    /// v0.16.4 — Keepout-role kinds checklist. The handler mutates
    /// the matching `kinds.<flag>` and runs solve+bake.
    FpEditorSetKeepoutKind {
        id: signex_sketch::id::SketchEntityId,
        kind: KeepoutKindFlag,
        value: bool,
    },
    /// v0.16.4 — BoardCutout-role edge-radius expression input.
    FpEditorSetCutoutEdgeRadius {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    /// v0.16.4 — BoardCutout-role through-vs-partial-depth toggle.
    FpEditorSetCutoutThrough {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    /// v0.23 — Pattern Properties sub-form text-input edit. The
    /// handler walks `sketch.arrays`, finds the array with `array_id`,
    /// mutates the field identified by `field`, then runs
    /// `SketchEdit::ForceRebuild` so the bake re-expands.
    FpEditorEditArrayParam {
        array_id: signex_sketch::array::ArrayId,
        field: ArrayParamField,
        value: String,
    },
    /// v0.23 — Switch the numbering scheme on an array. The handler
    /// preserves the existing inner fields when possible (LinearIncrement
    /// keeps prior start/step exprs; flipping to Explicit clears the
    /// names list).
    FpEditorSetArrayNumberingScheme {
        array_id: signex_sketch::array::ArrayId,
        scheme: NumberingSchemeKindUi,
    },
    /// v0.25 polish — toggle BGA `skip_letters`. Active only when the
    /// array's numbering is BgaRowCol; ignored for Linear / Explicit.
    FpEditorSetBgaSkipLetters {
        array_id: signex_sketch::array::ArrayId,
        value: bool,
    },
    /// v0.25 polish — set BGA `start_row` letter. Empty input no-ops;
    /// non-letter input no-ops; multi-char input takes the first
    /// letter. Uppercased before storage.
    FpEditorSetBgaStartRow {
        array_id: signex_sketch::array::ArrayId,
        value: String,
    },
    /// v0.25 polish — set BGA `start_col` integer. Empty input no-ops;
    /// non-numeric input no-ops; bounds are otherwise unconstrained.
    FpEditorSetBgaStartCol {
        array_id: signex_sketch::array::ArrayId,
        value: String,
    },
    /// v0.23 — Delete the array entirely. The source entity stays put.
    FpEditorDeleteArray {
        array_id: signex_sketch::array::ArrayId,
    },
    /// v0.23 — Begin re-picking the polar centre. Sets
    /// `ToolPending::RepickPolarCenter { array_id }` so the next sketch
    /// click on a Point overwrites `array.center`. Cancels with Esc.
    FpEditorBeginRepickPolarCenter {
        array_id: signex_sketch::array::ArrayId,
    },
    /// v0.23 — Toggle a single (i, j) instance in a Grid array's
    /// `GridDepopulation.suppressed_instances`. `value=true` re-enables
    /// the instance; `value=false` suppresses it.
    FpEditorToggleArrayInstance {
        array_id: signex_sketch::array::ArrayId,
        i: u32,
        j: u32,
        value: bool,
    },
    /// v0.17.0 — empty-canvas Snap Options toggles. The handler
    /// flips the matching `SnapOptions` flag.
    FpEditorToggleSnapOption(SnapOptionFlag),
    /// v0.18.9 — author-controlled snap grid step (mm). The handler
    /// parses the string and writes
    /// `state.snap_options.grid_step_mm`. Invalid / empty strings
    /// no-op so the input doesn't fight intermediate keystrokes.
    FpEditorSetSnapGridStep(String),
    /// v0.13 — Altium "Snap Distance" numeric input.
    FpEditorSetSnapDistance(String),
    /// v0.13 — Altium "Axis Snap Range" numeric input.
    FpEditorSetAxisSnapRange(String),
    /// v0.13 — Properties panel rename for the active internal
    /// footprint. Routes to `editor.primitive_mut().name`.
    FpEditorSetFootprintName(String),
    /// v0.18.13 — Altium Selection Filter pill toggle. The pills
    /// live on the v0.18.14 unified active bar; the Properties
    /// panel surfaces the toggle through this same message.
    FpEditorToggleSelectionFilter(crate::library::editor::footprint::state::SelectionFilterKind),
    /// v0.18.13 — open the Custom Selection Filter modal. Stubbed
    /// until v0.18.14 ships the modal body alongside the active
    /// bar pills.
    FpEditorOpenSelectionFilterCustom,
    /// v0.18.13 — Snap Options sub-tab switch (Grids / Guides / Axes).
    FpEditorSetSnapSubTab(crate::library::editor::footprint::state::SnapSubTab),
    /// v0.18.13 — Snapping mode 3-state toggle.
    FpEditorSetSnappingMode(crate::library::editor::footprint::state::SnappingMode),
    /// v0.18.13 — `Add` button on the Grid Manager table (placeholder
    /// for the multi-grid CRUD that lands with the v0.18.14 grid
    /// system). Dispatch logs a warn for now.
    FpEditorGridManagerAdd,
    /// v0.18.13 — `Properties` button on the Grid Manager row;
    /// reuses the Ctrl+G Cartesian Grid Editor modal.
    FpEditorGridManagerProperties,
    /// v0.18.13 — `Delete` button on the Grid Manager row
    /// (placeholder until multi-grid CRUD lands).
    FpEditorGridManagerDelete,
    /// v0.18.21 — Activate the row at the given index. Mirrors the
    /// row's step / display style / multiplier onto `snap_options` so
    /// the canvas + snap logic switch to the new grid.
    FpEditorGridSetActive(usize),
    /// v0.18.24 — Edit the `content` field of a selected silk-front
    /// `FpGraphicKind::Text { content, .. }` entry. The dispatcher
    /// finds `editor.state.selected_silk_f` and mutates the matching
    /// `silk_f[idx]` if it's a Text. No-op for non-Text selections.
    FpEditorSetSilkText(String),
    /// v0.18.24 — Delete the selected silk-front graphic. Mirrors the
    /// existing `FootprintDeleteSilkF` PrimitiveEditorMsg surface but
    /// is emitted from the Properties panel's silk-selection branch.
    FpEditorDeleteSelectedSilk,
    /// v0.18.13 — `Add` button on the Guide Manager table
    /// (placeholder until guide system lands).
    FpEditorGuideManagerAdd,
    /// v0.18.20 — `Add Vertical` button on the Guide Manager footer.
    /// Appends a new vertical guide at world X = 0 mm.
    FpEditorGuideAddVertical,
    /// v0.18.20 — `Add Horizontal` button on the Guide Manager footer.
    /// Appends a new horizontal guide at world Y = 0 mm.
    FpEditorGuideAddHorizontal,
    /// v0.18.20 — Per-row delete button on the Guide Manager. Removes
    /// the guide at the given index.
    FpEditorGuideDelete(usize),
    /// v0.18.20 — Per-row enabled toggle on the Guide Manager. Flips
    /// `guides[idx].enabled` so the user can hide individual guides
    /// without deleting them.
    FpEditorGuideToggle(usize),
    /// v0.18.20 — Per-row position edit on the Guide Manager. The text
    /// input emits the raw string; the dispatcher parses to f64 and
    /// no-ops on invalid input so intermediate keystrokes don't fight
    /// the user.
    FpEditorGuideSetPosition(usize, String),
    /// v0.14.2 — open a sibling `.snxfpt` from the Footprint Library
    /// panel. The handler routes through the existing
    /// `handle_open_primitive` flow so the file gets a fresh tab + a
    /// `FootprintEditorState` (or activates an existing tab).
    FpLibraryOpenSibling(std::path::PathBuf),
    /// v0.18.8 — Footprint Library panel single-click on an internal
    /// footprint row. Sets `panel_selected_idx` (independent of
    /// `active_idx`) so the row highlights and the bottom button
    /// row gates Place / Delete / Edit on it.
    FpLibrarySelectInternal(usize),
    /// v0.18.8 — `+ Add` button: append an empty `Footprint` to the
    /// active envelope and switch onto it. Routes through the
    /// existing `FootprintAddNewSibling` handler.
    FpLibraryAddInternal,
    /// v0.18.8 — `Delete` button: remove the selected internal
    /// footprint from the envelope. The active footprint clamps to
    /// the new last index when the deleted row was the active one.
    FpLibraryDeleteInternal(usize),
    /// v0.18.8 — `Edit` button (also fires on row double-click):
    /// promote `panel_selected_idx` to `active_idx` so the canvas
    /// switches to the selected sibling.
    FpLibraryEditInternal(usize),
    /// v0.18.8 — `Place` button: place the selected internal
    /// footprint as a Component on the active PCB. Stubbed until
    /// the PCB integration lands; for now no-op + tracing-warn.
    FpLibraryPlaceInternal(usize),
    /// Clear the current ERC violations list and canvas markers.
    ClearErc,
    /// Focus a specific ERC diagnostic row from the global flattened list.
    FocusErcViolation(usize),
    /// Focus previous ERC diagnostic row in the global list.
    FocusPrevErcDiagnostic,
    /// Focus next ERC diagnostic row in the global list.
    FocusNextErcDiagnostic,
    /// User clicked the Quick Fix chip on an ERC violation row. Routes
    /// to a per-rule handler — UnusedPin places a NoConnect at the
    /// pin, every other rule falls back to "zoom + select" (same as
    /// clicking the row body).
    ErcQuickFix(usize),
    ToggleGrid,
    ToggleSnap,
    PropertiesTab(usize),
    SelectLibrary(String),
    SelectComponent(String),
    DragComponentsSplit,
    ComponentFilter(String),
    /// F15 — Properties panel: open the primitive picker (symbol /
    /// footprint) for the row described by the active
    /// `PanelContext.library_row_detail`. Routes to
    /// `LibraryMessage::OpenPrimitivePicker` with a
    /// `PrimitivePickerTarget::BrowserRow` so the pick applies +
    /// persists through the existing adapter path.
    LibraryRowPickSymbol,
    LibraryRowPickFootprint,
    /// Toggle a collapsible section (by section key).
    ToggleSection(String),
    /// Edit a symbol's designator (committed on submit).
    EditSymbolDesignator(uuid::Uuid, String),
    /// Edit a symbol's value (committed on submit).
    EditSymbolValue(uuid::Uuid, String),
    /// Edit a symbol's footprint (committed on submit).
    EditSymbolFootprint(uuid::Uuid, String),
    /// Toggle a symbol's mirror_x.
    ToggleSymbolMirrorX(uuid::Uuid),
    /// Toggle a symbol's mirror_y.
    ToggleSymbolMirrorY(uuid::Uuid),
    /// Toggle a symbol's locked state.
    ToggleSymbolLocked(uuid::Uuid),
    /// Toggle a symbol's DNP state.
    ToggleSymbolDnp(uuid::Uuid),
    /// Set absolute rotation on a symbol (degrees).
    EditSymbolRotation(uuid::Uuid, f64),
    /// Set font size (Altium pt) on a symbol's value text property.
    EditSymbolValueFontSizePt(uuid::Uuid, u32),
    /// Change the lib_id of a symbol (used by power-port Style dropdown).
    EditSymbolLibId(uuid::Uuid, String),
    /// Swap a power-port's style: change lib_id and preserve visual direction
    /// by setting rotation accordingly.
    EditPowerPortStyle {
        symbol_id: uuid::Uuid,
        new_lib_id: String,
        rotation_degrees: f64,
    },
    /// Edit a label's text (committed on submit).
    EditLabelText(uuid::Uuid, String),
    /// Edit a label's horizontal justification.
    EditLabelJustifyH(uuid::Uuid, signex_types::schematic::HAlign),
    /// Edit a label direction preset (rotation + horizontal justify).
    EditLabelDirection(uuid::Uuid, f64, signex_types::schematic::HAlign),
    /// Edit a label's rotation (degrees).
    EditLabelRotation(uuid::Uuid, f64),
    /// Edit a label's font size in Altium pt (10 = 2.54 mm).
    EditLabelFontSizePt(uuid::Uuid, u32),
    /// Edit a text note's text (committed on submit).
    EditTextNoteText(uuid::Uuid, String),
    /// Pre-placement: update label/text field.
    SetPrePlacementText(String),
    /// Pre-placement: update designator field.
    SetPrePlacementDesignator(String),
    /// Pre-placement: update rotation.
    SetPrePlacementRotation(f64),
    /// Pre-placement: update font family.
    SetPrePlacementFont(String),
    /// Pre-placement: update font size (pt).
    SetPrePlacementFontSize(u32),
    /// Pre-placement: set horizontal justification.
    SetPrePlacementJustifyH(signex_types::schematic::HAlign),
    /// Pre-placement: set vertical justification.
    SetPrePlacementJustifyV(signex_types::schematic::VAlign),
    /// Pre-placement: toggle bold / italic / underline.
    TogglePrePlacementBold,
    TogglePrePlacementItalic,
    TogglePrePlacementUnderline,
    SetPrePlacementShapeWidth(f64),
    SetPrePlacementShapeFill(signex_types::schematic::FillType),
    /// Properties panel — edit a pin's designator (number) on the
    /// active Symbol editor tab. Routed through `handle_dock_sch_library_message`
    /// because the symbol editor's lifecycle owns the pin edits.
    SymEditorSetPinNumber {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's display name.
    SymEditorSetPinName {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's stub length in mm.
    SymEditorSetPinLength {
        pin_idx: usize,
        value: f64,
    },
    /// Properties panel — set a pin's electrical type from the
    /// Altium-spec dropdown (Input / Output / Bidirectional / Power
    /// / Passive / Open Collector / Open Emitter / Tri-state /
    /// Not Connected / Unspecified).
    SymEditorSetPinElectrical {
        pin_idx: usize,
        value: signex_library::PinDirection,
    },
    /// Properties panel — set a pin's orientation (Right / Up /
    /// Left / Down). Also updates the canvas cache so the pin
    /// re-renders.
    SymEditorSetPinOrientation {
        pin_idx: usize,
        value: signex_library::PinOrientation,
    },
    /// Properties panel — set a pin's X coordinate in mm.
    SymEditorSetPinX {
        pin_idx: usize,
        value: f64,
    },
    /// Properties panel — set a pin's Y coordinate in mm.
    SymEditorSetPinY {
        pin_idx: usize,
        value: f64,
    },
    /// SCH Library panel — click on a row in the Pins sub-list.
    /// Selects the pin on the canvas (Properties panel switches
    /// to pin-mode automatically via the next refresh_panel_ctx).
    SymEditorSelectPin(usize),
    /// Properties panel — edit a pin's free-text Description.
    SymEditorSetPinDescription {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's Function list (alt-names) as
    /// a single comma-separated string. Persisted as `Vec<String>`
    /// after splitting + trimming on commit.
    SymEditorSetPinFunctionCsv {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — toggle a pin's designator visibility.
    SymEditorTogglePinDesignatorVisible(usize),
    /// Properties panel — toggle a pin's name visibility.
    SymEditorTogglePinNameVisible(usize),
    /// Properties panel — toggle Pin Hide.
    SymEditorTogglePinHidden(usize),
    /// Properties panel — toggle Pin Locked.
    SymEditorTogglePinLocked(usize),
    /// Properties panel — set one of the four IEEE-symbol slots on
    /// a pin. `slot` is 0=Inside, 1=InsideEdge, 2=OutsideEdge, 3=Outside.
    SymEditorSetPinSymbol {
        pin_idx: usize,
        slot: u8,
        value: signex_library::PinSymbolKind,
    },
    /// Properties panel — set a pin's multi-part scope (Altium
    /// "Part Number" spinner). `0` is the special Part Zero (pin
    /// appears on every part); `1..=N` scopes to a specific part.
    SymEditorSetPinPartNumber {
        pin_idx: usize,
        value: u8,
    },
    /// SCH Library panel: select a placed graphic from the active
    /// symbol's `graphics` vector. Fires the same selection state as
    /// a canvas click on a graphic body.
    SymEditorSelectGraphic(usize),
    /// SCH Library panel: switch the editor's `active_part` to the
    /// given value. `0` selects Part Zero (shared pins). The
    /// dispatcher clamps to `[0, active_max_part]` so a stale tree
    /// click can't park `active_part` outside the symbol's range.
    SymEditorSelectPart(u8),
    /// Properties panel — set one numeric field of the placed graphic
    /// at `idx`. The dispatcher routes `field` to the matching
    /// `SymbolGraphicKind` variant; mismatched (idx, field) pairs
    /// silently no-op so an out-of-date Properties panel can't
    /// corrupt geometry.
    SymEditorSetGraphicField {
        idx: usize,
        field: GraphicFieldId,
        value: f64,
    },
    /// Properties panel — set the text content of a placed
    /// `SymbolGraphicKind::Text` at `idx`. No-op for other kinds.
    SymEditorSetGraphicText {
        idx: usize,
        value: String,
    },
    /// Properties panel — edit the active symbol's name (Altium
    /// "Design Item ID"). Affects the SCH Library panel row label
    /// + the on-disk container's `display_name` when the active
    /// symbol is the only one in the file.
    SymEditorSetSymbolName(String),
    /// Properties panel — edit the active symbol's designator
    /// template (Altium "Designator", e.g. `U?`).
    SymEditorSetSymbolDesignator(String),
    /// Properties panel — edit the active symbol's comment
    /// passthrough.
    SymEditorSetSymbolComment(String),
    /// Properties panel — edit the active symbol's free-text
    /// description.
    SymEditorSetSymbolDescription(String),
    /// Properties panel — pick the active symbol's Component Type.
    SymEditorSetSymbolType(signex_library::ComponentType),
    /// Properties panel — toggle the active symbol's mirrored flag.
    SymEditorToggleSymbolMirrored,
    /// Properties panel — cycle the active symbol's local fill
    /// colour through preset palette → None → preset palette.
    SymEditorCycleLocalFillColor,
    /// Properties panel — cycle the active symbol's local line
    /// colour.
    SymEditorCycleLocalLineColor,
    /// Properties panel — cycle the active symbol's local pin
    /// colour.
    SymEditorCycleLocalPinColor,
    /// Document Options (Properties pane when nothing is selected)
    /// — set the sheet background color preset on the containing
    /// `.snxlib`. All `.snxsym` tabs from the same library share.
    SymEditorSetDisplaySheetColor(SheetColor),
    /// Document Options — toggle the visible dot grid on the
    /// containing `.snxlib`.
    SymEditorToggleDisplayGrid,
    /// Document Options — cycle the visible grid spacing through
    /// `crate::canvas::grid::GRID_SIZES_MM`.
    SymEditorCycleDisplayGridSize,
    /// Document Options — cycle the coordinate display unit on
    /// the containing `.snxlib` (mm → mil → inch → um → mm).
    SymEditorCycleDisplayUnit,
    /// SCH Library panel: switch the active symbol within the open
    /// `.snxsym` container to the given index.
    SchLibrarySelectSymbol(usize),
    /// SCH Library panel: append a new empty symbol to the open
    /// container and make it active. Caller emits a default name —
    /// the user renames via the Properties panel.
    SchLibraryAddSymbol,
    /// SCH Library panel: delete the symbol at the given index from
    /// the open container. Refuses to delete the last remaining
    /// symbol — the file would be empty otherwise.
    SchLibraryDeleteSymbol(usize),
    UpdateDrawingEdit(crate::app::contracts::DrawingFieldEdit),
    /// Numeric text_input keystroke for a drawing field. The string
    /// is stored verbatim in panel_ctx.drawing_edit_buf so empty /
    /// partial input survives between frames; the handler parses
    /// best-effort and fires UpdateDrawingEdit when the value is a
    /// valid f64.
    DrawingFieldTyping(DrawingFieldId, String),
    /// Open / close the border-colour picker overlay for a child sheet.
    ToggleChildSheetBorderPicker(uuid::Uuid),
    /// Open / close the fill-colour picker overlay for a child sheet.
    ToggleChildSheetFillPicker(uuid::Uuid),
    /// Expand the currently-open child-sheet picker dropdown into the
    /// full HSV / RGB ColorPicker overlay. `is_border` selects which
    /// channel (border vs fill).
    OpenChildSheetAdvancedPicker(uuid::Uuid, bool),
    /// Cancel the currently-open child-sheet colour picker without
    /// committing a new value.
    CancelChildSheetColorPicker,
    /// Commit a new border colour for a child sheet (closes the picker).
    EditChildSheetBorderColor(uuid::Uuid, iced::Color),
    /// Commit a new fill colour for a child sheet (closes the picker).
    EditChildSheetFillColor(uuid::Uuid, iced::Color),
    /// Buffered keystroke for the child sheet stroke-width input.
    ChildSheetStrokeWidthTyping(uuid::Uuid, String),
    /// Commit the currently-buffered child sheet stroke width.
    CommitChildSheetStrokeWidth(uuid::Uuid),
    /// Reset child sheet styling (border / fill colour, line width)
    /// back to theme defaults.
    ResetChildSheetStyle(uuid::Uuid),
    /// Pre-placement: confirm and close.
    ConfirmPrePlacement,
    /// Set snap grid size (mm).
    SetGridSize(f32),
    /// Set visible grid size (mm) — independent of snap grid.
    SetVisibleGridSize(f32),
    /// Toggle snap to electrical object hotspots.
    ToggleSnapHotspots,
    /// Change the UI font (saved to prefs; applies on next restart).
    #[allow(dead_code)]
    SetUiFont(String),
    /// Change the canvas font (applied immediately to schematic/PCB text).
    SetCanvasFont(String),
    /// Change canvas font size (px) applied to canvas text rendering.
    SetCanvasFontSize(f32),
    /// Toggle canvas font bold style.
    SetCanvasFontBold(bool),
    /// Toggle canvas font italic style.
    SetCanvasFontItalic(bool),
    /// Open canvas font popup.
    OpenCanvasFontPopup,
    /// Close canvas font popup.
    CloseCanvasFontPopup,
    /// Set page margin vertical zones.
    SetMarginVertical(u32),
    /// Set page margin horizontal zones.
    SetMarginHorizontal(u32),
    /// Toggle a single selection filter — shared with the Active Bar.
    ToggleSelectionFilter(crate::active_bar::SelectionFilter),
    /// Toggle all selection filters on/off — shared with the Active Bar.
    ToggleAllSelectionFilters,
    /// Append a new empty custom filter preset (no-op when at the cap).
    AddCustomFilterPreset,
    /// Remove the preset at this index.
    RemoveCustomFilterPreset(usize),
    /// Rename the preset at this index.
    RenameCustomFilterPreset(usize, String),
    /// Toggle whether the preset at `idx` includes `filter`.
    ToggleCustomFilterPresetMember(usize, crate::active_bar::SelectionFilter),
    /// Snapshot the active selection filter set into the preset at `idx`.
    CaptureCustomFilterPreset(usize),
    /// Switch the Properties-panel preset editor to the given tab.
    SelectCustomFilterTab(usize),
    /// Page Options: choose formatting mode.
    SetPageFormatMode(PageFormatMode),
    /// Page Options: choose paper size.
    SetPaperSize(String),
    /// Page Options: choose origin corner.
    SetPageOrigin(PageOrigin),
    /// Page Options: set custom paper width (mm).
    SetCustomPaperWidth(f32),
    /// Page Options: set custom paper height (mm).
    SetCustomPaperHeight(f32),
    /// Page Options: choose sheet background colour.
    SetSheetColor(SheetColor),
    /// No-op placeholder for unimplemented UI controls.
    Noop,
}

/// Render a panel's content.
pub fn view_panel<'a>(kind: PanelKind, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    // Components has its own split scrollables — don't wrap again
    if kind == PanelKind::Components {
        return view_components(ctx);
    }

    let content = match kind {
        PanelKind::Components => return view_components(ctx),
        PanelKind::Projects => view_projects(ctx),
        PanelKind::Navigator => view_navigator(ctx),
        PanelKind::Properties => view_properties(ctx),
        PanelKind::Filter => view_stub("Selection Filter", "All object types enabled", ctx),
        PanelKind::Erc => view_erc(ctx),
        PanelKind::Messages => view_messages(ctx),
        PanelKind::Signal => view_stub("Signal AI", "Pro feature — AI design review", ctx),
        PanelKind::Drc => view_stub("DRC", "Run DRC to check PCB design rules", ctx),
        PanelKind::LayerStack => view_stub("Layer Stack", "PCB mode only", ctx),
        PanelKind::NetClasses => view_stub("Net Classes", "Define net classes and rules", ctx),
        PanelKind::Variants => view_stub("Variants", "Design variant management", ctx),
        PanelKind::OutputJobs => view_stub("Output Jobs", "Manufacturing output config", ctx),
        PanelKind::SchFilter => view_stub("SCH Filter", "Schematic object filter", ctx),
        PanelKind::SchList => view_stub("SCH List", "Schematic object list inspector", ctx),
        PanelKind::BomStudio => view_stub("BOM Studio", "Bill of Materials management", ctx),
        PanelKind::Favorites => view_stub("Favorites", "Favorite components and snippets", ctx),
        PanelKind::Snippets => view_stub("Snippets", "Reusable schematic snippets", ctx),
        PanelKind::Todo => view_stub("To-Do", "Task and issue tracking", ctx),
        PanelKind::Wiki => view_stub("Wiki", "Project documentation wiki", ctx),
        PanelKind::Library => view_stub(
            "Library",
            "Library panel — see Signex.library state. Use the dock host's Library panel \
             rendering path; this stub fires only if Library is mounted via PanelMsg \
             instead of LibraryMessage routing.",
            ctx,
        ),
        PanelKind::SchLibrary => view_sch_library(ctx),
        PanelKind::FootprintLibrary => view_footprint_library(ctx),
        PanelKind::History => return history::view_history(ctx),
    };

    scrollable(content).width(Length::Fill).into()
}

// ─── Helpers ──────────────────────────────────────────────────

// SVG chevrons (same as tree_view for consistency)
const SVG_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M3 1l5 4-5 4z" fill="currentColor"/></svg>"#;
const SVG_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M1 3l4 5 4-5z" fill="currentColor"/></svg>"#;

fn chevron_right() -> svg::Handle {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    H.get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_RIGHT))
        .clone()
}
fn chevron_down() -> svg::Handle {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    H.get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_DOWN))
        .clone()
}

pub(super) fn shape_icon_handle(
    elem_type: &str,
    theme: signex_types::theme::ThemeId,
) -> Option<svg::Handle> {
    match elem_type {
        "Line" => Some(crate::icons::icon_shape_line(theme)),
        "Rectangle" => Some(crate::icons::icon_shape_rect(theme)),
        "Circle" => Some(crate::icons::icon_shape_circle(theme)),
        "Arc" => Some(crate::icons::icon_shape_arc(theme)),
        "Polygon" => Some(crate::icons::icon_shape_polygon(theme)),
        _ => None,
    }
}

/// Just the header part of a collapsible section — clickable button
/// with SVG chevron + 1px rule. Returns whether the section is
/// collapsed via `is_collapsed_section(...)` so callers can guard
/// their body push without using a closure.
pub(super) fn collapsible_section_header<'a>(
    key: &str,
    title: &str,
    collapsed: &CollapsedSections,
    header_color: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let is_collapsed = collapsed.contains(key);
    let chevron_handle = if is_collapsed {
        chevron_right()
    } else {
        chevron_down()
    };
    let key_owned = key.to_string();

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        iced::widget::button(
            container(
                row![
                    svg(chevron_handle)
                        .width(10)
                        .height(10)
                        .style(move |_: &Theme, _| iced::widget::svg::Style {
                            color: Some(header_color),
                        }),
                    text(title.to_string()).size(12).color(header_color),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([6, 8])
            .width(Length::Fill),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key_owned))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(border_c)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            }
        }),
    );
    col = col.push(thin_sep(border_c));
    col
}

pub(super) fn is_section_collapsed(key: &str, collapsed: &CollapsedSections) -> bool {
    collapsed.contains(key)
}

/// Collapsible section: clickable header with SVG chevron, hides content when collapsed.
pub(super) fn collapsible_section<'a>(
    key: &str,
    title: &str,
    collapsed: &CollapsedSections,
    header_color: Color,
    border_c: Color,
    content: impl FnOnce() -> Column<'a, PanelMsg>,
) -> Column<'a, PanelMsg> {
    let is_collapsed = collapsed.contains(key);
    let mut col = collapsible_section_header(key, title, collapsed, header_color, border_c);
    if !is_collapsed {
        col = col.push(content());
    }
    col
}

/// Property key-value row (owned strings to avoid lifetime issues in closures).
pub(super) fn prop_kv_row<'a>(
    key: &str,
    value: &str,
    key_c: Color,
    val_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(key.to_string()).size(10).color(key_c).width(84),
            text(value.to_string()).size(10).color(val_c),
        ]
        .spacing(4),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

fn section_title<'a>(title: &str, tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    text(title.to_uppercase())
        .size(9)
        .color(theme_ext::text_secondary(tokens))
}

fn separator<'a>(tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    // Use a cached static string instead of allocating on every render
    const SEP: &str = "──────────────────────────────";
    text(SEP).size(4).color(theme_ext::border_color(tokens))
}

fn view_stub<'a>(title: &str, desc: &str, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    container(
        column![
            section_title(title, &ctx.tokens),
            separator(&ctx.tokens),
            text(desc.to_string())
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .spacing(4)
        .padding(6),
    )
    .width(Length::Fill)
    .into()
}

// ─── SCH Library Panel (Altium parity) ───────────────────────────────
//
// When a `.snxsym` standalone editor tab is active, this panel lists
// the symbols in the open `SymbolFile` container. Click switches the
// active symbol; "Add Symbol" appends a fresh empty Symbol to the
// container and makes it active. Read-only on Symbol metadata for now —
// rename / designator-prefix edits go through the right-dock Properties
// panel.
//
// When no Symbol editor is open the panel renders a hint pointing the
// user at the project tree's `Add New ▸ Symbol` flow.

/// Render one indented part row inside the SCH Library tree-expander.
/// Active part gets the selection background; otherwise the row hovers
/// like the symbol rows above it.
fn part_tree_row<'a>(
    label: &str,
    part: u8,
    is_active: bool,
    primary: Color,
    muted: Color,
    bg_active: Color,
) -> Element<'a, PanelMsg> {
    let label_color = if is_active { primary } else { muted };
    iced::widget::button(
        row![
            // Tree-expander indent: 18 px gutter + a faint glyph so
            // the part rows visually nest under the symbol.
            text("\u{2514}")
                .size(10)
                .color(muted)
                .width(Length::Fixed(18.0)),
            text(label.to_string())
                .size(10)
                .color(label_color)
                .width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .on_press(PanelMsg::SymEditorSelectPart(part))
    .style(
        move |_: &iced::Theme, status: iced::widget::button::Status| iced::widget::button::Style {
            background: if is_active {
                Some(iced::Background::Color(bg_active))
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                )))
            } else {
                None
            },
            border: iced::Border::default(),
            text_color: label_color,
            ..iced::widget::button::Style::default()
        },
    )
    .into()
}

fn view_sch_library<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(text("SCH Library").size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    let Some(sym) = ctx.symbol_editor.as_ref() else {
        col = col.push(
            container(
                text(
                    "Open a `.snxsym` to see its symbols here. Right-click a library node \
                     in the project tree and pick `Add New ▸ Symbol` to create one.",
                )
                .size(10)
                .color(muted),
            )
            .padding([6, 8])
            .width(Length::Fill),
        );
        return scrollable(col).width(Length::Fill).into();
    };

    // ── Breadcrumb header — F28 (2026-05-03) ──
    // Shows `<library_name>  >  <symbol_file>  (N symbols)` so the
    // user always knows which `.snxlib` the active `.snxsym` belongs
    // to. Walk `sym.path` ancestors looking for the first directory
    // ending in `.snxlib`; fall back to just the filename when the
    // file lives outside any library.
    let symbol_file = sym
        .path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<untitled>".to_string());
    let library_stem: Option<String> = sym
        .path
        .ancestors()
        .find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("snxlib"))
                .unwrap_or(false)
        })
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    let breadcrumb = match library_stem {
        Some(lib) => format!(
            "{}  ›  {}  ({} symbols)",
            lib,
            symbol_file,
            sym.symbols_in_file.len(),
        ),
        None => format!("{}  ({} symbols)", symbol_file, sym.symbols_in_file.len()),
    };
    col = col.push(container(text(breadcrumb).size(10).color(muted)).padding([4, 8]));
    col = col.push(thin_sep(border_c));

    // ── Column header — Altium SCH Library parity. Two columns:
    //     Design Item ID | Description.
    col = col.push(
        container(
            row![
                text("Design Item ID")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Description")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(4)),
            ]
            .spacing(6),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // ── Symbols tree ──
    // Each symbol is a row showing Design Item ID + Description.
    // Multi-part symbols (or symbols with Part Zero pins) expand
    // under the active symbol with one row per part — click to
    // switch active_part.
    for entry in &sym.symbols_in_file {
        let is_active = entry.idx == sym.active_idx;
        let label_color = if is_active { primary } else { muted };
        let bg_active = crate::styles::ti(ctx.tokens.selection);
        let row_msg = PanelMsg::SchLibrarySelectSymbol(entry.idx);
        let row_btn = iced::widget::button(
            row![
                text(entry.name.clone())
                    .size(11)
                    .color(label_color)
                    .width(Length::FillPortion(3)),
                text(entry.description.clone())
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(4)),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .on_press(row_msg)
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: if is_active {
                        Some(iced::Background::Color(bg_active))
                    } else if matches!(status, iced::widget::button::Status::Hovered) {
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04,
                        )))
                    } else {
                        None
                    },
                    border: iced::Border::default(),
                    text_color: label_color,
                    ..iced::widget::button::Style::default()
                }
            },
        );
        col = col.push(row_btn);

        // Part tree-expander under the active multi-part symbol.
        if is_active && (sym.active_max_part > 1 || sym.active_has_part_zero) {
            if sym.active_has_part_zero {
                col = col.push(part_tree_row(
                    "Part 0 (shared)",
                    0,
                    sym.active_part == 0,
                    primary,
                    muted,
                    bg_active,
                ));
            }
            for part in 1..=sym.active_max_part {
                col = col.push(part_tree_row(
                    &format!("Part {part}"),
                    part,
                    sym.active_part == part,
                    primary,
                    muted,
                    bg_active,
                ));
            }
        }
    }

    col = col.push(thin_sep(border_c));

    // Pins / Graphics sub-lists used to live here; per Altium SCH
    // Library panel parity they belong on dedicated panels (Pins
    // already shows in the Properties panel's Pin selection branch;
    // a dedicated SCHLIB Filter / SCHLIB List pair lives on the
    // bottom panel-tabs strip when that ships).
    col = col.push(thin_sep(border_c));

    // ── Action row: Place / Add / Delete / Edit (Altium parity) ──
    let action_btn_style = move |is_primary: bool, enabled: bool| {
        let text_color = if enabled { primary } else { muted };
        move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg_alpha = if !enabled {
                0.02
            } else if is_primary {
                if matches!(status, iced::widget::button::Status::Hovered) {
                    0.10
                } else {
                    0.04
                }
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                0.08
            } else {
                0.03
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, bg_alpha,
                ))),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                text_color,
                ..iced::widget::button::Style::default()
            }
        }
    };

    // Place — fires on the active symbol (would dispatch to schematic
    // place flow when wired). Stub for now: greyed-out.
    let place_btn = iced::widget::button(text("Place").size(11).color(muted))
        .padding([4, 10])
        .style(action_btn_style(true, false));

    let add_btn = iced::widget::button(text("Add").size(11).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::SchLibraryAddSymbol)
        .style(action_btn_style(false, true));

    let can_delete = sym.symbols_in_file.len() > 1;
    let delete_color = if can_delete { primary } else { muted };
    let mut delete_btn = iced::widget::button(text("Delete").size(11).color(delete_color))
        .padding([4, 10])
        .style(action_btn_style(false, can_delete));
    if can_delete {
        delete_btn = delete_btn.on_press(PanelMsg::SchLibraryDeleteSymbol(sym.active_idx));
    }

    // Edit — opens the active symbol in the standalone editor (for
    // when the SCH Library panel is the only visible surface).
    // Stub for now since the symbol is already open in its tab.
    let edit_btn = iced::widget::button(text("Edit").size(11).color(muted))
        .padding([4, 10])
        .style(action_btn_style(false, false));

    col = col.push(
        container(
            row![
                place_btn,
                Space::new().width(4),
                add_btn,
                Space::new().width(4),
                delete_btn,
                Space::new().width(4),
                edit_btn,
                Space::new().width(Length::Fill),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 8]),
    );

    scrollable(col).width(Length::Fill).into()
}

// ─── Projects Panel (TreeView) ────────────────────────────────

/// Build the project tree from panel context data. Produces one root
/// per loaded project so multi-project workspaces show all their
/// projects side by side. Single-project users see the same shape as
/// before (one root). `PanelContext::projects` is the source of truth;
/// the legacy `project_name` / `sheets` singletons are ignored here so
/// we never emit a duplicate root for the active project.
pub fn build_project_tree(ctx: &PanelContext) -> Vec<TreeNode> {
    if ctx.projects.is_empty() {
        return vec![];
    }

    ctx.projects.iter().map(project_root_node).collect()
}

/// One project root — "Source Documents" / "Libraries" / "Settings".
/// The Libraries branch lists this project's mounted `*.snxlib`
/// entries (right-click → Add New ▸ Component Library to add one);
/// it renders empty when the project has no libraries rather than
/// inheriting a workspace-wide symbol count.
fn project_root_node(project: &ProjectPanelInfo) -> TreeNode {
    let mut source_docs: Vec<TreeNode> = Vec::new();

    // F24 — surface a `(missing)` suffix on every leaf whose backing
    // file is registered on the project but absent from disk. Catches
    // orphan references (e.g. user moved/deleted a file outside Signex,
    // or a previous library-create attempt left an entry behind without
    // the file). User sees the broken state at a glance instead of
    // having to double-click and read an error.
    fn missing_label(filename: &str, is_missing: bool) -> String {
        if is_missing {
            format!("{filename}  (missing)")
        } else {
            filename.to_string()
        }
    }

    if !project.sheets.is_empty() {
        for sheet in &project.sheets {
            let icon = TreeIcon::for_path(&sheet.filename);
            source_docs.push(
                TreeNode::leaf(missing_label(&sheet.filename, sheet.missing), icon)
                    .with_open(sheet.is_open)
                    .with_dirty(sheet.is_dirty)
                    .with_active(sheet.is_active),
            );
        }
    } else if let Some(file) = &project.project_file {
        let icon = TreeIcon::for_path(file);
        source_docs.push(
            TreeNode::leaf(missing_label(file, project.project_file_missing), icon)
                .with_open(project.project_file_open)
                .with_dirty(project.project_file_dirty)
                .with_active(project.project_file_active),
        );
    }

    if let Some(pcb) = &project.pcb_file {
        let icon = TreeIcon::for_path(pcb);
        source_docs.push(
            TreeNode::leaf(missing_label(pcb, project.pcb_file_missing), icon)
                .with_open(project.pcb_file_open)
                .with_dirty(project.pcb_file_dirty)
                .with_active(project.pcb_file_active),
        );
    }

    // Render each `Project::libraries[i]` entry as a single `.snxlib`
    // leaf — no children, no chevron. Under the v0.9 `.snxlib`-as-file
    // model a library is one thing the user opens, not a folder they
    // browse. Symbols / footprints / sims are siblings on disk, but
    // surfacing them in the project tree confuses the mental model:
    // the user's contract is the `.snxlib` file, not its private
    // working directory. Double-click opens the Library Browser tab,
    // which is the proper surface for browsing the library's contents.
    //
    // When the project carries no library entries the Libraries branch
    // renders empty (matching Settings) — the user can right-click →
    // Add New ▸ Component Library to create one. We deliberately do
    // NOT mint a synthetic "N symbols loaded" child from the
    // workspace-wide library count: that pre-DBLib placeholder
    // advertised symbols the project hadn't actually mounted, which
    // caused real confusion (a project with nothing saved would show
    // "222 symbols loaded" sourced from globally-mounted libraries).
    // F29 — when a library is mounted and exposes symbols /
    // footprints, render a `Symbols` and `Footprints` subbranch
    // underneath the `.snxlib` node so the user can navigate to a
    // specific primitive directly from the tree. Unmounted /
    // missing / empty libraries collapse to a plain leaf (matches
    // the previous behaviour).
    // Libraries branch: every entry renders as a single leaf with the
    // file's full filename (incl. extension). `.snxlib` (Component
    // Libraries), `.snxsym` (Symbol Libraries), and `.snxfpt` (PCB
    // Libraries) all live as siblings under this branch — Altium
    // parity. No nested Symbols / Footprints subbranches: a `.snxlib`
    // can hold thousands of primitives and surfacing them in the tree
    // would explode the panel; opening the `.snxlib` shows the
    // browser instead.
    let lib_children: Vec<TreeNode> = project
        .libraries
        .iter()
        .map(|lib| {
            let filename = lib
                .root
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}.snxlib", lib.display_name));
            let display = if lib.missing {
                format!("{filename}  (missing)")
            } else {
                filename.clone()
            };
            let icon = TreeIcon::for_path(&filename);
            // v0.13 — surface the white open-dot / red dirty-dot
            // indicators on `.snxlib` leaves so library files match
            // the visual rhythm of `.snxsch` / `.snxpcb` sheets.
            TreeNode::leaf(display, icon)
                .with_open(lib.is_open)
                .with_dirty(lib.is_dirty)
        })
        .collect();

    // Settings holds nothing today — gated until a project actually
    // carries per-project preferences. Showing an empty branch reads
    // as "this project has settings hidden behind a toggle"; the
    // honest UI is to omit the heading.
    let settings_children: Vec<TreeNode> = Vec::new();

    // Build the project's child list, skipping any heading whose
    // children list is empty so the tree never renders a bare
    // "(empty)" placeholder under Libraries / Settings.
    let mut children: Vec<TreeNode> = Vec::new();
    if !source_docs.is_empty() {
        children.push(TreeNode::branch(
            "Source Documents".to_string(),
            TreeIcon::Folder,
            source_docs,
        ));
    }
    if !lib_children.is_empty() {
        children.push(TreeNode::branch(
            "Libraries".to_string(),
            TreeIcon::Library,
            lib_children,
        ));
    }
    if !settings_children.is_empty() {
        let mut settings =
            TreeNode::branch("Settings".to_string(), TreeIcon::File, settings_children);
        settings.expanded = false;
        children.push(settings);
    }

    TreeNode::branch(project.name.clone(), TreeIcon::Folder, children)
        .with_accent(project.is_active)
        .with_dirty(project.is_dirty)
}

/// v0.18.8 — Footprint Library panel. Mirror of Altium's PCB Library
/// panel: rows are the footprints *inside* the active `.snxfpt`
/// envelope (one per `file.footprints[i]`), with a Place / Add /
/// Delete / Edit button row at the bottom. Single-click highlights;
/// double-click (or Edit) promotes the selection to `active_idx`.
///
/// Cross-file navigation (sibling `.snxfpt` files inside the same
/// `.snxlib`) is reachable through the project tree — keeping the
/// panel single-purpose so the button row's targets are unambiguous.
fn view_footprint_library<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);
    let bg_active = crate::styles::ti(ctx.tokens.selection);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(text("Footprint Library").size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    let Some(fp) = ctx.footprint_editor.as_ref() else {
        col = col.push(
            container(
                text(
                    "Open a `.snxfpt` to see its footprints here. Right-click a \
                     `.snxlib` (or a project) in the project tree and pick \
                     `Add New ▸ Footprint Library` to create one.",
                )
                .size(10)
                .color(muted),
            )
            .padding([6, 8])
            .width(Length::Fill),
        );
        return scrollable(col).width(Length::Fill).into();
    };

    // Breadcrumb — file name + footprint count.
    let file_name = fp
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("<unknown>")
        .to_string();
    let breadcrumb = format!(
        "{}  ({} footprint{})",
        file_name,
        fp.internal_footprints.len(),
        if fp.internal_footprints.len() == 1 {
            ""
        } else {
            "s"
        },
    );
    col = col.push(container(text(breadcrumb).size(10).color(muted)).padding([4, 8]));
    col = col.push(thin_sep(border_c));

    // Two-column header: Name + Pads (right-aligned count).
    col = col.push(
        container(
            row![
                text("Name").size(10).color(muted).width(Length::Fill),
                text("Pads").size(10).color(muted),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // Internal-footprint rows.
    for (idx, footprint_row) in fp.internal_footprints.iter().enumerate() {
        let is_selected = fp.internal_selected_idx == Some(idx);
        let is_active = footprint_row.is_active;
        // Active row paints with the selection tint; selected-only
        // (not yet active) paints with a slightly lighter tint so the
        // user can tell selection apart from "currently editing".
        let bg = if is_active {
            iced::Background::Color(bg_active)
        } else if is_selected {
            iced::Background::Color(iced::Color {
                a: 0.4,
                ..bg_active
            })
        } else {
            iced::Background::Color(iced::Color::TRANSPARENT)
        };
        let label_color = if is_active || is_selected {
            primary
        } else {
            muted
        };
        let row_btn = iced::widget::button(
            row![
                text(footprint_row.name.clone())
                    .size(10)
                    .color(label_color)
                    .width(Length::Fill),
                text(footprint_row.pad_count.to_string())
                    .size(10)
                    .color(label_color),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 8])
        .on_press(PanelMsg::FpLibrarySelectInternal(idx))
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(bg),
            border: iced::Border {
                width: 0.0,
                radius: 0.0.into(),
                color: iced::Color::TRANSPARENT,
            },
            ..iced::widget::button::Style::default()
        })
        .width(Length::Fill);
        col = col.push(row_btn);
    }

    // Place / Add / Delete / Edit button row pinned at the bottom of
    // the panel (Altium PCB Library parity). Place / Delete / Edit
    // require a selection; greyed when `internal_selected_idx` is
    // None. Add is always live.
    let selected = fp.internal_selected_idx;
    let footer = view_footprint_library_button_row(ctx, selected);

    iced::widget::column![
        scrollable(col).width(Length::Fill).height(Length::Fill),
        thin_sep(border_c),
        footer,
    ]
    .into()
}

/// Bottom button row for the Footprint Library panel — Altium's
/// `Place / Add / Delete / Edit` quartet.
fn view_footprint_library_button_row<'a>(
    ctx: &'a PanelContext,
    selected: Option<usize>,
) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);

    let mk_btn = |label: &'static str, on_press: Option<PanelMsg>| -> Element<'a, PanelMsg> {
        let enabled = on_press.is_some();
        let label_color = if enabled { primary } else { muted };
        let mut btn = iced::widget::button(
            text(label)
                .size(11)
                .color(label_color)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([4, 12])
        .width(Length::Fixed(64.0))
        .style(move |_: &Theme, status| {
            let bg = match status {
                iced::widget::button::Status::Hovered if enabled => {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                }
                _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.02),
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..iced::widget::button::Style::default()
            }
        });
        if let Some(msg) = on_press {
            btn = btn.on_press(msg);
        }
        btn.into()
    };

    let place = mk_btn(
        "Place",
        // PCB integration not wired yet — keep the button visible
        // but disabled to advertise the intended affordance.
        selected
            .map(PanelMsg::FpLibraryPlaceInternal)
            .filter(|_| false),
    );
    let add = mk_btn("Add", Some(PanelMsg::FpLibraryAddInternal));
    let delete = mk_btn("Delete", selected.map(PanelMsg::FpLibraryDeleteInternal));
    let edit = mk_btn("Edit", selected.map(PanelMsg::FpLibraryEditInternal));

    container(
        row![place, add, delete, edit]
            .spacing(4)
            .align_y(iced::Alignment::Center),
    )
    .padding([6, 8])
    .width(Length::Fill)
    .into()
}

fn view_projects<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    if ctx.project_tree.is_empty() {
        let muted = theme_ext::text_secondary(&ctx.tokens);
        column![
            text("No project open").size(11).color(muted),
            text("").size(4),
            text("File > Open to begin").size(10).color(muted),
        ]
        .spacing(2)
        .padding(6)
        .width(Length::Fill)
        .into()
    } else {
        // Render the persistent tree — toggle state is preserved.
        // Wrap in a container with a small top inset so the tree's
        // first row doesn't sit flush against the panel's tab-strip
        // border (matches the breathing room Altium leaves below its
        // panel tabs).
        container({
            let mut tree = TreeView::new(&ctx.project_tree, &ctx.tokens);
            if let Some(sel) = ctx.project_tree_selected.as_deref() {
                tree = tree.selected(sel);
            }
            tree.view().map(PanelMsg::Tree)
        })
        .padding(iced::Padding {
            top: 6.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .into()
    }
}

// ─── Components Panel (matched to Altium Designer) ───────────

fn view_components<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);
    let hover_c = crate::styles::ti(ctx.tokens.hover);
    let panel_bg_c = crate::styles::ti(ctx.tokens.panel_bg);
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);

    // ── TOP: Library selector + component list (scrollable) ──
    let mut list_col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    list_col = list_col.push(
        container(
            iced::widget::pick_list(
                ctx.standard_libraries.clone(),
                ctx.active_library.clone(),
                PanelMsg::SelectLibrary,
            )
            .placeholder("Select a library...")
            .text_size(11)
            .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    // Search filter input
    list_col = list_col.push(
        container(
            iced::widget::text_input("Search components...", &ctx.component_filter)
                .on_input(PanelMsg::ComponentFilter)
                .size(11)
                .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    list_col = list_col.push(thin_sep(border_c));
    list_col = list_col.push(
        container(
            row![
                text("Name")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Library")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
                text("Pins")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(1)),
            ]
            .spacing(4.0),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    list_col = list_col.push(thin_sep(border_c));

    // Filter the symbol list
    let filter = ctx.component_filter.to_ascii_lowercase();
    let filtered_symbols: Vec<&LibrarySymbolEntry> = if filter.is_empty() {
        ctx.library_symbols.iter().collect()
    } else {
        ctx.library_symbols
            .iter()
            .filter(|entry| {
                entry.symbol_name.to_ascii_lowercase().contains(&filter)
                    || entry.library_name.to_ascii_lowercase().contains(&filter)
                    || entry.lib_id.to_ascii_lowercase().contains(&filter)
            })
            .collect()
    };

    if filtered_symbols.is_empty() {
        let msg = if ctx.active_library.is_some() {
            if filter.is_empty() {
                "Loading..."
            } else {
                "No matches"
            }
        } else {
            "Select a library above"
        };
        list_col = list_col.push(container(text(msg).size(10).color(muted)).padding([8, 8]));
    } else {
        let sel = &ctx.selected_component;
        for entry in &filtered_symbols {
            let is_sel = sel.as_deref() == Some(entry.lib_id.as_str());
            let row_bg = if is_sel {
                theme_ext::selection_color(&ctx.tokens)
            } else {
                Color::TRANSPARENT
            };
            let name_c = if is_sel { Color::WHITE } else { primary };
            let lib_id = entry.lib_id.clone();
            list_col = list_col.push(
                column![
                    iced::widget::button(
                        row![
                            text(entry.symbol_name.clone())
                                .size(10)
                                .color(name_c)
                                .width(Length::FillPortion(3))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(entry.library_name.clone())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(entry.pin_count.to_string())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(1)),
                        ]
                        .spacing(4.0),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .on_press(PanelMsg::SelectComponent(lib_id))
                    .style(
                        move |_: &Theme, status: iced::widget::button::Status| {
                            let bg = match (is_sel, status) {
                                (true, _) => Some(Background::Color(row_bg)),
                                (false, iced::widget::button::Status::Hovered) => {
                                    Some(Background::Color(hover_c))
                                }
                                _ => None,
                            };
                            iced::widget::button::Style {
                                background: bg,
                                border: Border::default(),
                                ..iced::widget::button::Style::default()
                            }
                        }
                    ),
                    thin_sep(border_c),
                ]
                .spacing(0),
            );
        }
    }

    list_col = list_col.push(
        container(
            text(format!("Results: {}", filtered_symbols.len()))
                .size(10)
                .color(muted),
        )
        .padding([4, 8]),
    );

    // ── BOTTOM: Details panel (scrollable) ──
    let mut detail_col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    if let Some(comp_id) = &ctx.selected_component {
        let selected_entry = ctx
            .library_symbols
            .iter()
            .find(|entry| &entry.lib_id == comp_id);
        let comp_name = selected_entry
            .map(|entry| entry.symbol_name.as_str())
            .unwrap_or(comp_id.as_str());
        detail_col = detail_col.push(section_hdr(
            &format!("\u{25BC} Details  {comp_name}"),
            primary,
            border_c,
        ));
        let pin_count = ctx
            .library_symbols
            .iter()
            .find(|entry| entry.lib_id == *comp_id)
            .map(|entry| entry.pin_count)
            .unwrap_or(0);
        detail_col = detail_col.push(form_input_row(
            "Symbol", comp_name, muted, input_bg, input_bdr,
        ));
        detail_col = detail_col.push(form_input_row(
            "Pins",
            &pin_count.to_string(),
            muted,
            input_bg,
            input_bdr,
        ));
        detail_col = detail_col.push(form_input_row(
            "Library",
            selected_entry
                .map(|entry| entry.library_name.as_str())
                .or(ctx.active_library.as_deref())
                .unwrap_or(""),
            muted,
            input_bg,
            input_bdr,
        ));

        // Symbol preview canvas
        detail_col = detail_col.push(Space::new().height(4.0));
        detail_col = detail_col.push(section_hdr("\u{25BC} Models", primary, border_c));
        if let Some(lib_sym) = &ctx.selected_lib_symbol {
            // Symbol preview
            detail_col = detail_col.push(
                container(
                    container(
                        signex_widgets::symbol_preview::symbol_preview(lib_sym.clone(), 120.0)
                            .map(|_: ()| PanelMsg::ToggleGrid),
                    )
                    .width(Length::Fill)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(panel_bg_c)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: border_c,
                        },
                        ..container::Style::default()
                    }),
                )
                .padding([4, 8]),
            );

            // Footprint preview placeholder
            detail_col = detail_col.push(
                container(
                    container(
                        text("Footprint preview")
                            .size(10)
                            .color(muted)
                            .align_x(iced::alignment::Horizontal::Center),
                    )
                    .width(Length::Fill)
                    .padding([30, 8])
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(panel_bg_c)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: border_c,
                        },
                        ..container::Style::default()
                    }),
                )
                .padding([4, 8]),
            );
        }

        for section in &["References", "Part Choices", "Where Used"] {
            detail_col = detail_col.push(Space::new().height(2.0));
            detail_col = detail_col.push(section_hdr(
                &format!("\u{25BC} {section}"),
                primary,
                border_c,
            ));
        }
    } else {
        detail_col = detail_col.push(
            container(text("Select a component").size(10).color(muted))
                .padding([12, 8])
                .width(Length::Fill),
        );
    }

    // Split view: list (fixed height) | handle | details (fill)
    column![
        container(scrollable(list_col).width(Length::Fill))
            .height(ctx.components_split)
            .width(Length::Fill),
        // Drag handle
        iced::widget::mouse_area(
            container(Space::new())
                .height(5.0)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }),
        )
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(PanelMsg::DragComponentsSplit),
        container(scrollable(detail_col).width(Length::Fill))
            .height(Length::Fill)
            .width(Length::Fill),
    ]
    .spacing(0)
    .into()
}

// ─── Navigator Panel ──────────────────────────────────────────

fn view_navigator<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(section_title("Sheets", &ctx.tokens));
    col = col.push(separator(&ctx.tokens));

    // Resolve the active project from the multi-project Vec — replaces
    // the legacy `ctx.project_name` singleton (#54 phase 2.5).
    let active_project = ctx.projects.iter().find(|p| p.is_active);
    if let Some(project) = active_project {
        let mut sheets = vec![];
        for cs in &ctx.child_sheets {
            sheets.push(TreeNode::leaf(cs.clone(), TreeIcon::Sheet));
        }
        let roots = vec![TreeNode::branch(
            project.name.clone(),
            TreeIcon::Schematic,
            sheets,
        )];
        col = col.push(
            TreeView::new(&roots, &ctx.tokens)
                .view()
                .map(PanelMsg::Tree),
        );
    } else {
        col = col.push(
            text("No project")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    }
    container(col).width(Length::Fill).into()
}

// ─── Properties Panel (matched to Altium Designer) ───────────

pub(super) const LABEL_W: f32 = 76.0;
pub(super) const PROPERTY_LABEL_PORTION: u16 = 2;
pub(super) const PROPERTY_CONTROL_PORTION: u16 = 5;
pub(super) const PROPERTY_ROW_PAD_X: u16 = 6;

/// F15 — Library Browser row detail in the Properties panel. Shows
/// the row's identifier line + Symbol / Footprint binding status with
/// Pick buttons. Mirrors what the Library Browser's inline preview
/// pane used to render; surfacing here means the user gets the row's
/// detail in the canonical "selected thing" panel, freeing horizontal
/// space inside the browser tab for the grid.
fn view_library_row_properties<'a>(
    d: &'a LibraryRowDetail,
    muted: iced::Color,
    primary: iced::Color,
    border_c: iced::Color,
    tokens: &'a ThemeTokens,
) -> Element<'a, PanelMsg> {
    let _ = tokens;
    // Truncate the row UUID to its first 8 hex chars for a
    // human-scannable identity line.
    let row_id_short = {
        let s = d.row_id.simple().to_string();
        if s.len() >= 8 { s[..8].to_string() } else { s }
    };
    let pn_text = if d.internal_pn.is_empty() {
        "(unnamed row)".to_string()
    } else {
        d.internal_pn.clone()
    };

    let pick_symbol_btn = button(text("Pick Symbol…").size(11).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::LibraryRowPickSymbol)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: primary,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
            },
            ..iced::widget::button::Style::default()
        });

    let pick_footprint_btn = button(text("Pick Footprint…").size(11).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::LibraryRowPickFootprint)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: primary,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
            },
            ..iced::widget::button::Style::default()
        });

    let body = column![
        text("Library Row").size(11).color(muted),
        Space::new().height(4),
        text(pn_text).size(13).color(primary),
        Space::new().height(2),
        text(format!(
            "table: {}  ·  class: {}  ·  {}  ·  row {}",
            d.table, d.class, d.lifecycle_label, row_id_short,
        ))
        .size(10)
        .color(muted),
        Space::new().height(12),
        text("Symbol").size(11).color(muted),
        Space::new().height(4),
        text(d.symbol_summary.clone()).size(11).color(primary),
        Space::new().height(6),
        pick_symbol_btn,
        Space::new().height(14),
        text("Footprint").size(11).color(muted),
        Space::new().height(4),
        text(d.footprint_summary.clone()).size(11).color(primary),
        Space::new().height(6),
        pick_footprint_btn,
    ]
    .spacing(0)
    .padding(10);

    container(body).width(Length::Fill).into()
}

fn view_properties<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);

    // Library Browser tab — Properties panel surfaces the selected
    // row's metadata + Pick Symbol / Pick Footprint. F15 (2026-05-03
    // library polish): "right pane can be opened on properties
    // instead." Takes precedence over the schematic / pre-placement /
    // symbol-editor branches so when a Library Browser tab is active
    // the panel stays focused on it.
    if let Some(detail) = ctx.library_row_detail.as_ref() {
        return view_library_row_properties(detail, muted, primary, border_c, &ctx.tokens);
    }

    // Symbol-editor tab takes precedence — when the user is editing a
    // `.snxsym` the right-dock Properties panel shows symbol/pin
    // properties driven by `panel_ctx.symbol_editor`. Matches Altium's
    // SchLib editor flow where the same Properties panel switches mode
    // based on selection. (#62 / v0.9 phase 1)
    if let Some(sym) = ctx.symbol_editor.as_ref() {
        return view_symbol_editor_properties(sym, muted, primary, border_c);
    }

    // v0.14.2 — Footprint-editor tab. Properties panel switches body
    // based on (mode × selection): Pads-mode pad selected → pad
    // properties; Sketch-mode entity selected → sketch entity
    // properties; nothing selected → footprint summary + solve stats.
    if let Some(fp) = ctx.footprint_editor.as_ref() {
        let accent_c = crate::styles::ti(ctx.tokens.accent);
        let tag_hover = {
            let c = crate::styles::ti(ctx.tokens.accent);
            Color {
                r: (c.r * 1.3).min(1.0),
                g: (c.g * 1.3).min(1.0),
                b: (c.b * 1.3).min(1.0),
                ..c
            }
        };
        let seg_hover = crate::styles::ti(ctx.tokens.hover);
        return view_footprint_editor_properties(
            fp,
            muted,
            primary,
            border_c,
            input_bg,
            input_bdr,
            ctx.custom_filter_presets.clone(),
            ctx.active_custom_filter_tab,
            &ctx.collapsed_sections,
            accent_c,
            tag_hover,
            ctx.unit,
            seg_hover,
        );
    }

    if !ctx.has_schematic {
        // Don't mislead the user into thinking nothing is loaded when
        // they've just switched to a PCB tab — distinguish "no project
        // yet" from "project open, but the active tab isn't a
        // schematic". GitHub issue #51.
        let hint = if ctx.has_pcb {
            "Properties are available when a schematic is active"
        } else {
            "Open a project"
        };
        return container(
            column![
                text("Properties").size(12).color(primary),
                Space::new().height(12.0),
                text(hint).size(11).color(muted),
            ]
            .spacing(4)
            .padding(8),
        )
        .width(Length::Fill)
        .into();
    }

    // ── Pre-placement properties (TAB pressed during tool) ──
    // TAB pauses placement and edits the properties the NEXT click will
    // commit with. We render a full Altium-style Location + Properties
    // form bound to the pre_placement data — not the live engine.
    if let Some(ref pp) = ctx.pre_placement {
        return view_pre_placement(pp, ctx, muted, primary, border_c, input_bg, input_bdr);
    }

    // ── Context-aware: if something is selected, show element properties (Altium style) ──
    if ctx.selection_count == 1 && !ctx.selection_info.is_empty() {
        return view_selected_element_properties(ctx, muted, primary, border_c);
    }
    if ctx.selection_count > 1 {
        let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
        col = col.push(
            container(text("Multi-Selection").size(11).color(primary))
                .padding([6, 8])
                .width(Length::Fill),
        );
        col = col.push(thin_sep(border_c));
        col = col.push(
            container(
                text(format!("{} objects selected", ctx.selection_count))
                    .size(10)
                    .color(muted),
            )
            .padding([4, 8]),
        );
        for (key, value) in &ctx.selection_info {
            col = col.push(
                container(
                    row![
                        text(key)
                            .size(10)
                            .color(muted)
                            .width(Length::FillPortion(2)),
                        text(value)
                            .size(10)
                            .color(primary)
                            .width(Length::FillPortion(3)),
                    ]
                    .spacing(4),
                )
                .padding([3, 8])
                .width(Length::Fill),
            );
        }
        return scrollable(col).width(Length::Fill).into();
    }

    // ── Nothing selected: show Document Options (Altium default) ──
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        container(text("Document Options").size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );

    // ── General | Parameters tab bar ──
    let tab = ctx.properties_tab;
    let tab_hover = crate::styles::ti(ctx.tokens.hover);
    let text_inactive = crate::styles::ti(ctx.tokens.text_secondary);
    col = col.push(
        container(
            row![
                props_tab_btn(
                    "General",
                    tab == 0,
                    PanelMsg::PropertiesTab(0),
                    primary,
                    text_inactive,
                    tab_hover,
                    border_c
                ),
                props_tab_btn(
                    "Parameters",
                    tab == 1,
                    PanelMsg::PropertiesTab(1),
                    primary,
                    text_inactive,
                    tab_hover,
                    border_c
                ),
            ]
            .spacing(2.0),
        )
        .padding([4, 8]),
    );
    col = col.push(thin_sep(border_c));

    // ── Tab content ──
    if tab == 0 {
        col = col.push(view_properties_general(ctx, muted, primary, border_c));
    } else {
        col = col.push(view_properties_parameters(
            muted,
            primary,
            border_c,
            crate::styles::ti(ctx.tokens.selection),
            crate::styles::ti(ctx.tokens.accent),
            crate::styles::ti(ctx.tokens.hover),
        ));
    }

    // ── Status: Nothing selected ──
    col = col.push(Space::new().height(8.0));
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(text("Nothing selected").size(10).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );

    scrollable(col).width(Length::Fill).into()
}

/// Altium-style context-aware properties for a single selected element.
/// Shows EDITABLE fields for symbols, labels, and text notes.

/// Pre-placement properties — shown when TAB pressed during a placement tool.
fn view_pre_placement<'a>(
    pp: &PrePlacementData,
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let label_text = pp.label_text.clone();
    let designator = pp.designator.clone();
    let rotation = pp.rotation;
    let tool_name = pp.tool_name.clone();
    let kind = pp.kind;
    let pos_str = format!("{:.2}, {:.2}", pp.cursor_x_mm, pp.cursor_y_mm);
    let rot_label = format!("{:.0} Degrees", rotation);
    let font = pp.font.clone();
    let font_size_pt = pp.font_size_pt;
    let justify_h = pp.justify_h;

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Header — kind-labelled, Altium style.
    col = col.push(
        container(
            row![
                text(tool_name.clone()).size(12).color(primary),
                Space::new().width(Length::Fill),
                iced::widget::button(text("OK").size(10).color(Color::WHITE))
                    .padding([2, 10])
                    .on_press(PanelMsg::ConfirmPrePlacement)
                    .style(iced::widget::button::primary),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 8])
        .width(Length::Fill),
    );

    col = col.push(container(Space::new()).height(1).width(Length::Fill).style(
        move |_: &Theme| container::Style {
            background: Some(Background::Color(border_c)),
            ..container::Style::default()
        },
    ));

    // ── Location ──
    col = col.push(collapsible_section(
        "preplace_location",
        "Location",
        &ctx.collapsed_sections,
        primary,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(form_input_row(
                "(X/Y)", &pos_str, muted, input_bg, input_bdr,
            ));
            let rotation_opts: Vec<String> = vec![
                "0 Degrees".into(),
                "90 Degrees".into(),
                "180 Degrees".into(),
                "270 Degrees".into(),
            ];
            c = c.push(form_pick_row(
                "Rotation",
                rotation_opts,
                rot_label.clone(),
                |s| {
                    let deg = s
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    PanelMsg::SetPrePlacementRotation(deg)
                },
                muted,
            ));
            c
        },
    ));

    // ── Properties (kind-specific) ──
    let text_label_for_kind = match kind {
        PrePlacementKind::NetLabel => "Net Name",
        PrePlacementKind::GlobalPort => "Port Name",
        PrePlacementKind::HierPort => "Sheet Name",
        PrePlacementKind::PowerPort => "Net Name",
        PrePlacementKind::TextNote => "Text",
        PrePlacementKind::Component => "Value",
        _ => "",
    };

    let show_text_field = !text_label_for_kind.is_empty();
    let show_designator = matches!(kind, PrePlacementKind::Component);
    let show_text_styling = matches!(
        kind,
        PrePlacementKind::NetLabel
            | PrePlacementKind::GlobalPort
            | PrePlacementKind::HierPort
            | PrePlacementKind::PowerPort
            | PrePlacementKind::TextNote
            | PrePlacementKind::Component
    );

    if show_text_field || show_text_styling {
        col = col.push(collapsible_section(
            "preplace_props",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                if show_text_field {
                    c = c.push(form_edit_row(
                        text_label_for_kind,
                        &label_text,
                        muted,
                        PanelMsg::SetPrePlacementText,
                    ));
                }
                if show_designator {
                    c = c.push(form_edit_row(
                        "Designator",
                        &designator,
                        muted,
                        PanelMsg::SetPrePlacementDesignator,
                    ));
                }
                if show_text_styling {
                    let font_opts: Vec<String> = crate::fonts::system_font_families().clone();
                    c = c.push(form_pick_row(
                        "Font",
                        font_opts,
                        font.clone(),
                        PanelMsg::SetPrePlacementFont,
                        muted,
                    ));
                    let size_opts: Vec<String> = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                        .iter()
                        .map(|n| n.to_string())
                        .collect();
                    c = c.push(form_pick_row(
                        "Font Size",
                        size_opts,
                        font_size_pt.to_string(),
                        |s| PanelMsg::SetPrePlacementFontSize(s.parse().unwrap_or(10)),
                        muted,
                    ));
                    c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                    c = c.push(form_label("Justification", muted));
                    c = c.push(
                        container(preplacement_justification_grid(
                            justify_h,
                            input_bg,
                            input_bdr,
                            primary,
                            muted,
                            ctx.theme_id,
                        ))
                        .padding([4, 8]),
                    );
                }
                c
            },
        ));
    } else if matches!(
        kind,
        PrePlacementKind::Line
            | PrePlacementKind::Rectangle
            | PrePlacementKind::Circle
            | PrePlacementKind::Arc
            | PrePlacementKind::Polygon
    ) {
        // Shape tools — Altium-style Width + Fill so users can
        // preconfigure the next placement via TAB.
        let width = pp.shape_width_mm;
        let fill = pp.shape_fill;
        let show_fill = !matches!(kind, PrePlacementKind::Line | PrePlacementKind::Arc);
        col = col.push(collapsible_section(
            "preplace_shape",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_edit_row_f64(
                    "Width (mm)",
                    width,
                    muted,
                    PanelMsg::SetPrePlacementShapeWidth,
                ));
                if show_fill {
                    c = c.push(shape_fill_row(fill, muted, border_c));
                }
                c
            },
        ));
    } else {
        col = col.push(
            container(
                text("Click to place. No per-instance options.")
                    .size(10)
                    .color(muted),
            )
            .padding([8, PROPERTY_ROW_PAD_X]),
        );
    }

    container(scrollable(col).width(Length::Fill))
        .width(Length::Fill)
        .into()
}

/// Wrap a property-row label `text` in a clipped fill-portion container.
/// Plain `text(...).width(FillPortion(...)).wrapping(None)` lays out at
/// the text's intrinsic width and bleeds past the allotted column when
/// the panel is narrow — covering the value column or the panel edge.
/// This helper enforces the FillPortion bound and clips visual overflow
/// inside it. Used by every `form_*_row` and inline property row.
pub(super) fn property_label<'a, M: 'a>(label: impl Into<String>, color: Color) -> Element<'a, M> {
    container(
        text(label.into())
            .size(11)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
    .clip(true)
    .into()
}

/// Like `property_label`, but for size-10 labels (used by `form_edit_row_f64`).
fn property_label_small<'a, M: 'a>(label: impl Into<String>, color: Color) -> Element<'a, M> {
    container(
        text(label.into())
            .size(10)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(Length::FillPortion(2))
    .clip(true)
    .into()
}

/// Numeric edit row used by the shape pre-placement form. Writes on
/// submit — partial text mid-type doesn't panic via parse failure.
pub(super) fn form_edit_row_f64<'a>(
    label: &'a str,
    value: f64,
    muted: Color,
    on_submit: impl Fn(f64) -> PanelMsg + 'a + Clone,
) -> Element<'a, PanelMsg> {
    use iced::widget::{row, text, text_input};
    let buf = format!("{value:.3}");
    let on_submit_cb = on_submit.clone();
    row![
        property_label_small(label, muted),
        text_input("", &buf)
            .size(11)
            .on_input(move |s| {
                if let Ok(v) = s.parse::<f64>() {
                    on_submit_cb(v)
                } else {
                    PanelMsg::Noop
                }
            })
            .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn shape_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::{button, row, text};
    use signex_types::schematic::FillType;
    let tile = |label: &'static str, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        button(text(label).size(10))
            .padding([3, 8])
            .on_press(PanelMsg::SetPrePlacementShapeFill(ft))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(if active {
                    Color::from_rgb(0.20, 0.36, 0.58)
                } else {
                    Color::from_rgba(0.25, 0.25, 0.28, 0.4)
                })),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: Color::from_rgb(0.28, 0.28, 0.32),
                },
                text_color: if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    muted
                },
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    row![
        text("Fill")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile("None", FillType::None, current == FillType::None),
            tile("Outline", FillType::Outline, current == FillType::Outline),
            tile(
                "Background",
                FillType::Background,
                current == FillType::Background
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn view_erc<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("ERC", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(
                text("Run ERC (F8)")
                    .size(11)
                    .color(theme_ext::text_primary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::RunErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(6).height(Length::Shrink),
            iced::widget::button(
                text("Clear")
                    .size(11)
                    .color(theme_ext::text_secondary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::ClearErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.erc_diagnostics.is_empty() {
        col = col.push(
            text("No ERC diagnostics yet")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        col = col.push(
            text("Run ERC to populate project-wide diagnostics")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        return col.into();
    }

    let errors = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Error)
        .count();
    let warnings = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Warning)
        .count();
    let infos = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Info)
        .count();
    let focus_label = if let Some(i) = ctx.erc_focus_index {
        format!("{}/{}", i + 1, ctx.erc_diagnostics.len())
    } else {
        format!("0/{}", ctx.erc_diagnostics.len())
    };

    col = col.push(
        row![
            text("ERC Diagnostic Results")
                .size(10)
                .color(theme_ext::text_primary(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{errors} errors"))
                .size(10)
                .color(theme_ext::error_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{warnings} warnings"))
                .size(10)
                .color(theme_ext::warning_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{infos} info"))
                .size(10)
                .color(theme_ext::accent(&ctx.tokens)),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(text("<").size(11))
                .padding([1, 6])
                .on_press(PanelMsg::FocusPrevErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            text(focus_label)
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            iced::widget::button(text(">").size(11))
                .padding([1, 6])
                .on_press(PanelMsg::FocusNextErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );

    for v in &ctx.erc_diagnostics {
        let sev_color = match v.severity {
            ErcSeverityLite::Error => theme_ext::error_color(&ctx.tokens),
            ErcSeverityLite::Warning => theme_ext::warning_color(&ctx.tokens),
            ErcSeverityLite::Info => theme_ext::accent(&ctx.tokens),
        };
        let sev_label = match v.severity {
            ErcSeverityLite::Error => "E",
            ErcSeverityLite::Warning => "W",
            ErcSeverityLite::Info => "I",
        };
        let is_focused = ctx.erc_focus_index == Some(v.global_index);
        let row_bg = if is_focused {
            Some(Background::Color(theme_ext::selection_color(&ctx.tokens)))
        } else {
            None
        };
        // Quick Fix chip label per rule kind (UX_IMPROVEMENTS_OVER_ALTIUM
        // §4.4). Only `UnusedPin` has a true mutating fix today —
        // place a NoConnect at the dangling pin. Every other rule's
        // chip is a fast "zoom + select" alias for the row click,
        // so the user has a one-click path to the offending item
        // even when the row's text is long enough that the click
        // target's centre lands far from the cursor.
        let quick_fix_label = match v.rule_kind {
            signex_erc::RuleKind::UnusedPin => "Add No-Connect",
            _ => "Show on Canvas",
        };
        col = col.push(
            row![
                iced::widget::button(
                    row![
                        text(sev_label).size(9).color(sev_color),
                        Space::new().width(4).height(Length::Shrink),
                        text(v.rule_label)
                            .size(9)
                            .color(theme_ext::text_primary(&ctx.tokens)),
                        Space::new().width(6).height(Length::Shrink),
                        text(v.message.clone())
                            .size(9)
                            .color(theme_ext::text_secondary(&ctx.tokens)),
                        Space::new().width(6).height(Length::Shrink),
                        text(format!(
                            "{}  ({:.2}, {:.2})  {}",
                            v.sheet_name,
                            v.world_x,
                            v.world_y,
                            v.sheet_path.display()
                        ))
                        .size(9)
                        .color(theme_ext::text_secondary(&ctx.tokens)),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::Fill),
                )
                .width(Length::Fill)
                .padding([2, 6])
                .on_press(PanelMsg::FocusErcViolation(v.global_index))
                .style(
                    move |_theme: &Theme, status: iced::widget::button::Status| {
                        let base = crate::styles::menu_item(&ctx.tokens)(_theme, status);
                        iced::widget::button::Style {
                            background: row_bg.clone().or(base.background),
                            ..base
                        }
                    },
                ),
                Space::new().width(6).height(Length::Shrink),
                iced::widget::button(
                    text(quick_fix_label)
                        .size(9)
                        .color(theme_ext::text_primary(&ctx.tokens)),
                )
                .padding([2, 8])
                .on_press(PanelMsg::ErcQuickFix(v.global_index))
                .style(crate::styles::menu_item(&ctx.tokens)),
            ]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        );
    }

    col.into()
}

// ─── Messages Panel ───────────────────────────────────────────

fn view_messages<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("Messages", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            text(format!("level {}", ctx.diagnostics_level))
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.diagnostics.is_empty() {
        col = col.push(
            text("No runtime messages yet")
                .size(10)
                .color(theme_ext::success_color(&ctx.tokens)),
        );
        col = col.push(
            text("Set RUST_LOG=debug or SIGNEX_LOG=debug for verbose output")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    } else {
        // Table header — mirrors Altium's Messages panel with fixed
        // columns for id, level, source tag, and the message body.
        let muted = theme_ext::text_secondary(&ctx.tokens);
        let primary = theme_ext::text_primary(&ctx.tokens);
        let border = theme_ext::border_color(&ctx.tokens);
        let header_bg = theme_ext::to_color(&ctx.tokens.hover);
        let row_bg = theme_ext::to_color(&ctx.tokens.panel_bg);
        // Alt-row tint: pull a darker variant from the panel background
        // by shifting alpha. iced::Color lets us blend cheaply.
        let alt_bg = Color {
            a: row_bg.a,
            r: (row_bg.r - 0.02).max(0.0),
            g: (row_bg.g - 0.02).max(0.0),
            b: (row_bg.b - 0.02).max(0.0),
        };

        let th = |label: &str| -> Element<'a, PanelMsg> {
            container(text(label.to_string()).size(11).color(muted))
                .padding([4, 8])
                .into()
        };
        // Header row — background fill, no border (separator line
        // below sits in its own element so the table reads as a grid
        // of horizontal rules instead of individually framed boxes).
        col = col.push(
            container(
                row![
                    container(th("#")).width(Length::Fixed(48.0)),
                    container(th("Level")).width(Length::Fixed(64.0)),
                    container(th("Source")).width(Length::Fixed(180.0)),
                    container(th("Message")).width(Length::Fill),
                ]
                .align_y(iced::Alignment::Center),
            )
            .style(move |_theme: &Theme| iced::widget::container::Style {
                background: Some(Background::Color(header_bg)),
                ..iced::widget::container::Style::default()
            }),
        );
        let separator = |bg: Color| -> Element<'a, PanelMsg> {
            container(Space::new())
                .height(Length::Fixed(1.0))
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(bg)),
                    ..iced::widget::container::Style::default()
                })
                .into()
        };
        col = col.push(separator(border));
        for (i, entry) in ctx.diagnostics.iter().rev().enumerate() {
            let level_color = match entry.level {
                crate::diagnostics::DiagnosticLevel::Error => theme_ext::error_color(&ctx.tokens),
                crate::diagnostics::DiagnosticLevel::Warning => {
                    theme_ext::warning_color(&ctx.tokens)
                }
                crate::diagnostics::DiagnosticLevel::Info => theme_ext::accent(&ctx.tokens),
                crate::diagnostics::DiagnosticLevel::Debug
                | crate::diagnostics::DiagnosticLevel::Trace => {
                    theme_ext::text_secondary(&ctx.tokens)
                }
            };
            let bg = if i % 2 == 0 { row_bg } else { alt_bg };
            let cell = |label: String, color: Color, size: f32| -> Element<'a, PanelMsg> {
                container(text(label).size(size).color(color))
                    .padding([4, 8])
                    .into()
            };
            col = col.push(
                container(
                    row![
                        container(cell(format!("#{}", entry.id), muted, 11.0))
                            .width(Length::Fixed(48.0)),
                        container(cell(entry.level.label().to_string(), level_color, 11.0,))
                            .width(Length::Fixed(64.0)),
                        container(cell(entry.code.as_str().to_string(), muted, 11.0))
                            .width(Length::Fixed(180.0)),
                        container(cell(entry.message.as_str().to_string(), primary, 12.0))
                            .width(Length::Fill),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                // No border on the row — the explicit separator line
                // drawn below is the grid. Boxes around each entry
                // (the previous look) were too heavy for a log table.
                .style(move |_theme: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(bg)),
                    ..iced::widget::container::Style::default()
                }),
            );
            // One-pixel horizontal rule between rows — no right / left
            // borders so the table reads as a stack of records with
            // shared separators, matching Altium's Messages panel.
            col = col.push(separator(border));
        }
    }

    container(col).width(Length::Fill).into()
}

// ─── Drawing properties editor (post-placement) ──────────────────
