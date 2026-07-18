//! Panel implementations — uses signex-widgets for proper Altium-style content.

use iced::mouse;
use iced::widget::canvas;
use iced::widget::{
    Column, Row, Space, button, container, pick_list, row, scrollable, svg, text,
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

// v0.14.x -- domain modules split out of the former single-file panels module.
mod messages;
mod color_field;
mod context;
mod footprint_context;
mod symbol_context;
mod paper;
mod widgets;
mod projects;
mod library;
mod components;
mod properties;
mod status;

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


// -- Re-exports: keep the `crate::panels::*` public surface stable and the
//    sibling `super::` / `crate::panels::` paths resolving after the split. --
pub use messages::PanelMsg;
pub use color_field::{ColorFieldProps, color_field};
pub use context::PanelContext;
pub use footprint_context::{
    ArrayKindSummary, ArrayParamField, ArraySummary, BgaConfigSummary, CutoutSummary,
    FootprintEditorPanelContext, FootprintLibInternalRow, FootprintLibSibling, FootprintModeKind,
    FootprintPadSummary, FootprintSelectedSilkSummary, FootprintSketchEntitySummary,
    FootprintSolveSummary, KeepoutKindFlag, KeepoutSummary, NumberingSchemeKindUi,
    OverConstraintSummary, PadShapeParamSummary, PourSummary, SilkKindGeometry,
    SketchPadAttrSummary, SnapOptionFlag,
};
pub use symbol_context::{
    GraphicFieldId, GraphicKindSummary, GraphicSummary, SymbolDisplayOptions,
    SymbolEditorPanelContext, SymbolEditorSelection, SymbolFileEntry, SymbolPinDetails,
    SymbolPinSummary,
};
pub use paper::{PAPER_SIZES, PageFormatMode, PageOrigin, SheetColor, paper_dimensions};
pub use projects::{LibraryNodeInfo, ProjectPanelInfo, SheetInfo, build_project_tree};
pub(super) use projects::{view_navigator, view_projects};
pub use library::{LibraryRowDetail, LibrarySymbolEntry};
pub(super) use library::{view_footprint_library, view_library_row_properties, view_sch_library};
pub(super) use components::view_components;
pub use properties::{DrawingFieldId, PrePlacementData, PrePlacementKind};
pub(super) use properties::{
    LABEL_W, PROPERTY_CONTROL_PORTION, PROPERTY_LABEL_PORTION, PROPERTY_ROW_PAD_X, view_properties,
};
pub use status::{ErcDiagnosticEntry, ErcSeverityLite};
pub(super) use status::{view_erc, view_messages};
pub(super) use widgets::{
    collapsible_section, collapsible_section_header, form_edit_row_f64, is_section_collapsed,
    part_tree_row, prop_kv_row, property_label, section_title, separator, shape_fill_row,
    shape_icon_handle, view_stub,
};

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

