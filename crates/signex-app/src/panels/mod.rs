//! Panel implementations, built on signex-widgets.

use iced::mouse;
use iced::widget::canvas;
use iced::widget::{Column, Space, container, row, scrollable, svg};
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Theme, Widget};
use iced_aw::{NumberInput, Wrap};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::tree_view::{TreeIcon, TreeMsg, TreeNode, TreeView};
use signex_widgets::{button, raw, text};

pub mod components_panel;
mod element_properties;
mod footprint_editor_properties;
pub mod history;
pub mod layer_stack;
mod pcb_layers;
mod placed_edit;
mod scroll;
pub(crate) use scroll::panel_scrollable;
mod properties_parameters;
mod symbol_editor_properties;

// v0.14.x -- domain modules split out of the former single-file panels module.
mod board_sketch_attr;
pub use board_sketch_attr::BoardSketchAttrMsg;
mod board_net_classes;
mod color_field;
mod context;
mod drafts;
mod drc;
mod drc_results;
pub use drafts::PanelDrafts;
mod empty_state;
pub(crate) mod enable_section;
pub use board_net_classes::{
    BoardNetClassDraft, BoardNetClassField, BoardNetClassMsg, BoardNetClassView,
};
pub use drc::{DrcPanel, DrcRow, MAX_ROWS as DRC_MAX_ROWS};
mod drc_idle;
pub use drc_idle::BaseRule;
mod drc_rows;
pub use drc_results::{
    DrcCategory, DrcFilter, DrcFilterChoice, DrcPanelMsg, DrcView, DrcWaiveDraft,
};
pub(crate) use empty_state::empty_state_card;
mod erc_panel;
mod erc_rows;
pub(super) use erc_panel::view_erc;
pub use erc_panel::{
    ErcChecked, ErcDiagnosticEntry, ErcPanelMsg, ErcPanelView, ErcSeverityLite, ErcWaiverEntry,
};
mod erc_waivers;
mod messages_view;
pub use messages_row::MessageSource;
pub(super) use messages_view::view_messages;
pub use messages_view::{MessageLevel, MessagesPanelMsg, MessagesView};
pub(crate) use messages_view::{
    copy_text as messages_copy_text, is_verbose, shown as messages_shown,
};
mod validation_kit;
mod validation_state;
pub use validation_state::ValidationPanels;
mod waiver_form;
pub use waiver_form::{UserIdentity, WaiverReason};
mod footprint_context;
pub(crate) mod form_look;
pub(crate) mod graphite;
mod library_row;
mod messages;
pub(crate) mod messages_row;
mod multi_selection;
mod navigator;
mod net_class_panel;
mod net_class_table;
pub mod net_inspector;
mod numeric_draft;
pub mod paired;
mod palette;
mod paper;
pub mod pcb_board_properties;
pub mod pcb_component;
pub mod pcb_nets;
pub mod pcb_primitive_properties;
mod projects;
mod projects_graphite;
mod properties;
mod sch_filter;
pub use sch_filter::SchFilterMsg;
mod symbol_context;
mod widgets;
mod wire_style_rows;

#[cfg(test)]
mod contrast_tests;
#[cfg(test)]
mod planned_tests;

pub(crate) use element_properties::{
    BODY_ALTERNATE, BODY_ALTERNATE_MISSING, BODY_NORMAL, BODY_NORMAL_CAN_SWITCH,
};
pub use element_properties::{
    BlanketEdit, FootprintCardFacts, FootprintCardKey, FootprintCardStatus, FootprintCardView,
    FootprintCards, NoErcRuleEdit,
};
use element_properties::{
    view_child_sheet_properties, view_drawing_properties, view_selected_element_properties,
};
use footprint_editor_properties::view_footprint_editor_properties;
// HI-22: re-export the form / layout helpers extracted into
// `properties_parameters.rs` so sibling modules (element_properties,
// symbol_editor_properties, footprint_editor_properties) and other
// view fns still in this file can reach them via `super::name(...)`.
pub use numeric_draft::{
    NumericDraft, NumericFieldKey, NumericOwner, field_state as numeric_field_state, numeric_input,
    shown_text as numeric_shown_text,
};
pub use properties_parameters::{
    BuiltinParameter, PaperEdge, PaperEdgeDraft, SheetVariableMsg, TitleBlockDraft,
    TitleBlockField, shown_title_block_value, typed_mm,
};
pub(super) use properties_parameters::{
    canvas_font_popup, empty_section_row, font_style_row, form_check_row, form_check_row_shortcut,
    form_choice_row, form_edit_row, form_font_link_row, form_grid_row, form_input_row,
    form_input_row_untipped, form_label, form_mm_edit_row, form_pick_row, form_rotation_row,
    justification_grid, net_numeric_row, net_params_add_bar, net_params_header, net_params_tabs,
    preplacement_justification_grid, props_tab_btn, section_hdr, seg_btn, thin_sep,
};
use properties_parameters::{view_properties_general, view_properties_parameters};
use symbol_editor_properties::view_symbol_editor_properties;

// -- Re-exports: keep the `crate::panels::*` public surface stable and the
//    sibling `super::` / `crate::panels::` paths resolving after the split. --
pub use color_field::{ColorFieldProps, color_field};
pub use context::{FootprintChoiceItem, PINS_SECTION_KEY, PanelContext, SymbolChoiceItem};
pub use erc_waivers::{ErcViolationWaiverEntry, ErcWaiveDraft, ErcWaiverMsg, ErcWaiverStatus};
pub use footprint_context::{
    ArrayKindSummary, ArrayParamField, ArraySummary, BgaConfigSummary, BoardSketchProps,
    CutoutSummary, FootprintEditorPanelContext, FootprintLibInternalRow, FootprintLibSibling,
    FootprintModeKind, FootprintPadSummary, FootprintSelectedSilkSummary,
    FootprintSketchEntitySummary, FootprintSolveSummary, KeepoutKindFlag, KeepoutSummary,
    MountingHoleProps, NumberingSchemeKindUi, OverConstraintSummary, PadShapeParamSummary,
    PourSummary, RejectedConflictSummary, SilkKindGeometry, SketchPadAttrSummary, SnapOptionFlag,
};
pub use layer_stack::{LayerStackFacts, LayerStackMsg, StackField};
pub use library_row::{ComponentSheet, LibraryRowDetail};
pub(super) use library_row::{view_component_sheet, view_library_row_properties};
pub use messages::{PanelMsg, SheetFrameEdit};
pub use multi_selection::{MultiEditMsg, MultiField, edit_from_text as multi_edit_from_text};
pub use navigator::NavigatorModel;
pub use net_class_panel::{
    LabelClassPick, NetClassChoice, NetClassDraft, NetClassField, NetClassMsg, NetClassPanel,
    net_class_choice, net_class_choices, view_net_classes_section,
};
pub use net_inspector::{NetInspectorMsg, NetInspectorState, NetInspectorView};
pub use palette::PanelPalette;
pub use paper::{
    PageFormatMode, PageOrigin, SchematicSheetColor, SheetColor, paper_dimensions,
    paper_size_options,
};
pub use pcb_board_properties::{
    BoardField, BoardTab, ComponentSide, GridCard, PcbBoardMsg, PcbBoardPanelView, PcbBoardSummary,
};
pub use pcb_component::{
    ComponentField, ComponentSettingFacts, FootprintEditability, FootprintLibrary, ParameterFacts,
    PartIdentity, PcbComponentFacts, PcbComponentMsg, PcbFootprintFacts, PcbFootprintTextMsg,
    Shared,
};
pub use pcb_layers::{LayersPanelMsg, LayersPanelUi, LayersTab, PcbLayersPanel};
pub use pcb_nets::PcbNetsSummary;
pub use pcb_primitive_properties::{PcbPrimitiveFacts, PcbPrimitiveMsg};
pub use placed_edit::{PlacedDraft, PlacedEditMsg, PlacedField, PlacedPanel};
pub use projects::{LibraryNodeInfo, ProjectPanelInfo, SheetInfo, build_project_tree};
pub(crate) use projects::{MISSING_MARK, UNREADABLE_MARK};
pub(super) use projects::{view_navigator, view_projects};
pub use projects_graphite::{ProjectsMsg, ProjectsView};
#[cfg(test)]
pub(crate) use properties::pre_placement_readout;
pub use properties::{DrawingFieldId, PrePlacementData, PrePlacementKind, PrePlacementStyle};
pub(super) use properties::{
    PROPERTY_CONTROL_PORTION, PROPERTY_LABEL_COLUMN, PROPERTY_LABEL_WIDTH, PROPERTY_ROW_GAP,
    PROPERTY_ROW_PAD_X, view_properties,
};
pub use symbol_context::{
    GraphicFieldId, GraphicKindSummary, GraphicSummary, SymbolDeleteWarning, SymbolDisplayOptions,
    SymbolEditorPanelContext, SymbolEditorSelection, SymbolFileEntry, SymbolPinDetails,
    SymbolPinSummary,
};
pub(super) use widgets::{
    collapsible_section_header, form_edit_row_f64, is_section_collapsed, part_tree_row,
    prop_kv_row, property_label, section_title, separator, shape_fill_row, shape_icon_handle,
    view_stub,
};
pub use wire_style_rows::{WireColorPicker, wire_color_row, wire_width_row};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Projects,
    Components,
    Navigator,
    Properties,
    Filter,
    Erc,
    Messages,
    Drc,
    LayerStack,
    /// The layers of the active tab (#2223, #2359): a board's layers with
    /// their eyes and the active layer, or a footprint's layers by side.
    Layers,
    NetClasses,
    Variants,
    OutputJobs,
    SchFilter,
    SchList,
    BomStudio,
    Favorites,
    Snippets,
    Todo,
    Wiki,
    /// The one Library panel (mockup row 119): the symbols of the open `.snxsym`,
    /// the footprints of the open `.snxfpt` or the parts of every open
    /// library, picked with its kind chips.
    Library,
    /// Panel-list alias of [`Self::Library`] in its Symbols kind (mockup row 120).
    /// Opening it opens the Library panel; it is never docked itself.
    SchLibrary,
    /// Panel-list alias of [`Self::Library`] in its Footprints kind (mockup row 121).
    /// Opening it opens the Library panel; it is never docked itself.
    FootprintLibrary,
    /// PCB editor net table: nets with pad counts and routed length.
    PcbNets,
    /// VSCode-style per-file Git history. Right-dock surface that
    /// follows the active tab and shows the file's last 50 commits
    /// via `signex_widgets::history_pane`.
    History,
    /// Every net of the active document's project from the stitched
    /// netlist, filterable, each expandable to its pins, labels, ports and
    /// sheets; a click goes to the member (#1269).
    NetInspector,
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
    PanelKind::Drc,
    PanelKind::BomStudio,
    PanelKind::Favorites,
    PanelKind::Snippets,
    PanelKind::LayerStack,
    PanelKind::Layers,
    PanelKind::NetClasses,
    PanelKind::Variants,
    PanelKind::OutputJobs,
    PanelKind::Todo,
    PanelKind::Wiki,
    PanelKind::SchLibrary,
    PanelKind::FootprintLibrary,
    PanelKind::PcbNets,
    PanelKind::History,
    PanelKind::NetInspector,
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
                | PanelKind::BomStudio
                | PanelKind::Snippets
                | PanelKind::Variants
                | PanelKind::OutputJobs
        )
    }

    /// Whether this panel requires a PCB document to be open. Net Classes
    /// edits the project's table, so it needs a project, not a board (#2416,
    /// #2417).
    pub fn needs_pcb(self) -> bool {
        matches!(self, PanelKind::Drc | PanelKind::LayerStack)
    }

    /// The Library panel kind an alias row opens it in (alias rows kept by
    /// Caner, 2026-11-20): SCH
    /// Library opens Symbols and Footprint Library opens Footprints.
    pub fn library_alias(self) -> Option<crate::library::state::LibraryPanelKind> {
        use crate::library::state::LibraryPanelKind;
        match self {
            PanelKind::SchLibrary => Some(LibraryPanelKind::Symbols),
            PanelKind::FootprintLibrary => Some(LibraryPanelKind::Footprints),
            _ => None,
        }
    }

    /// The catalog message of [`Self::label`] (dock tabs and titles).
    pub fn message(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            PanelKind::Projects => ids::PANELS_KIND_PROJECTS,
            PanelKind::Components => ids::PANELS_KIND_COMPONENTS,
            PanelKind::Navigator => ids::PANELS_KIND_NAVIGATOR,
            PanelKind::Properties => ids::PANELS_KIND_PROPERTIES,
            PanelKind::Filter => ids::PANELS_KIND_FILTER,
            PanelKind::Erc => ids::PANELS_KIND_ERC,
            PanelKind::Messages => ids::PANELS_KIND_MESSAGES,
            PanelKind::Drc => ids::PANELS_KIND_DRC,
            PanelKind::LayerStack => ids::PANELS_KIND_LAYER_STACK,
            PanelKind::Layers => ids::PANELS_KIND_LAYERS,
            PanelKind::NetClasses => ids::PANELS_KIND_NET_CLASSES,
            PanelKind::Variants => ids::PANELS_KIND_VARIANTS,
            PanelKind::SchFilter => ids::PANELS_KIND_SCH_FILTER,
            PanelKind::SchList => ids::PANELS_KIND_SCH_LIST,
            PanelKind::BomStudio => ids::PANELS_KIND_BOM_STUDIO,
            PanelKind::Favorites => ids::PANELS_KIND_FAVORITES,
            PanelKind::Snippets => ids::PANELS_KIND_SNIPPETS,
            PanelKind::Todo => ids::PANELS_KIND_TODO,
            PanelKind::Wiki => ids::PANELS_KIND_WIKI,
            PanelKind::OutputJobs => ids::PANELS_KIND_OUTPUT_JOBS,
            PanelKind::Library => ids::PANELS_KIND_LIBRARY,
            PanelKind::SchLibrary => ids::PANELS_KIND_SCH_LIBRARY,
            PanelKind::FootprintLibrary => ids::PANELS_KIND_FOOTPRINT_LIBRARY,
            PanelKind::PcbNets => ids::PANELS_KIND_PCB_NETS,
            PanelKind::History => ids::PANELS_KIND_HISTORY,
            PanelKind::NetInspector => ids::PANELS_KIND_NET_INSPECTOR,
        }
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
            PanelKind::Drc => "DRC",
            PanelKind::LayerStack => "Layer Stack",
            PanelKind::Layers => "Layers",
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
            PanelKind::PcbNets => "PCB",
            PanelKind::History => "History",
            PanelKind::NetInspector => "Net Inspector",
        }
    }
}

/// An icon action a panel puts in its dock title bar.
pub struct TitleAction {
    pub icon: svg::Handle,
    pub tip: signex_i18n::Localized,
    pub on_press: PanelMsg,
}

/// The title-bar actions of `kind`: Clear of the ERC panel (#85) while it
/// holds results, Copy and Clear of the Messages panel (#86).
pub fn title_actions(kind: PanelKind, ctx: &PanelContext) -> Vec<TitleAction> {
    use signex_i18n::{ids, t};
    let tid = ctx.theme_id;
    match kind {
        PanelKind::Erc if ctx.erc_ran || !ctx.erc_diagnostics.is_empty() => vec![TitleAction {
            icon: crate::icons::icon_dd_delete(tid),
            tip: t!(ctx.i18n, ids::PANELS_ERC_CLEAR_TOOLTIP),
            on_press: PanelMsg::ClearErc,
        }],
        PanelKind::Messages => vec![
            TitleAction {
                icon: crate::icons::icon_dd_copy(tid),
                tip: t!(ctx.i18n, ids::PANELS_MESSAGES_COPY_TOOLTIP),
                on_press: PanelMsg::MessagesView(MessagesPanelMsg::Copied),
            },
            TitleAction {
                icon: crate::icons::icon_dd_delete(tid),
                tip: t!(ctx.i18n, ids::PANELS_MESSAGES_CLEAR_TOOLTIP),
                on_press: PanelMsg::MessagesView(MessagesPanelMsg::Cleared),
            },
        ],
        _ => Vec::new(),
    }
}

/// Track which sections are collapsed (by section name).
pub type CollapsedSections = std::collections::HashSet<String>;

/// Render a panel's content.
pub fn view_panel<'a>(kind: PanelKind, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    use signex_i18n::ids;
    let content = match kind {
        // The Components panel is drawn by `components_panel::view` (the dock and the
        // detached window both route it there); nothing to show here.
        PanelKind::Components => iced::widget::Space::new().boxed(),
        PanelKind::Projects => view_projects(ctx),
        PanelKind::Navigator => view_navigator(ctx),
        PanelKind::Properties => view_properties(ctx),
        PanelKind::Filter => empty_state_card(
            &ctx.tokens,
            signex_i18n::t!(ctx.i18n, signex_i18n::ids::PANELS_SELECTION_FILTER_TITLE),
            signex_i18n::t!(ctx.i18n, signex_i18n::ids::PANELS_SELECTION_FILTER_BODY),
            None,
        ),
        PanelKind::Erc => view_erc(ctx),
        PanelKind::Messages => view_messages(ctx),
        PanelKind::Drc => drc_results::view(ctx),
        PanelKind::LayerStack => layer_stack::view(ctx),
        // One Layers panel follows the active tab (#2359): a footprint's
        // layers on a footprint tab, else the board's (or its empty state).
        PanelKind::Layers if ctx.footprint_editor.is_some() => {
            crate::library::editor::footprint::layers_panel::view(ctx)
        }
        PanelKind::Layers => pcb_layers::view(ctx),
        // On a board, its per-class rules (#2400); elsewhere the project's
        // net-class table, the editor Properties shows for the sheet without
        // its section frame (#2416, #2417).
        PanelKind::NetClasses if ctx.net_classes.board.is_some() => board_net_classes::view(ctx),
        PanelKind::NetClasses => net_class_panel::view_panel(ctx),
        PanelKind::Variants => view_stub(ids::PANELS_KIND_VARIANTS, ids::PANELS_STUB_VARIANTS, ctx),
        PanelKind::OutputJobs => view_stub(
            ids::PANELS_KIND_OUTPUT_JOBS,
            ids::PANELS_STUB_OUTPUT_JOBS,
            ctx,
        ),
        PanelKind::SchFilter => sch_filter::view(ctx),
        PanelKind::SchList => view_stub(ids::PANELS_KIND_SCH_LIST, ids::PANELS_STUB_SCH_LIST, ctx),
        PanelKind::BomStudio => view_stub(
            ids::PANELS_KIND_BOM_STUDIO,
            ids::PANELS_STUB_BOM_STUDIO,
            ctx,
        ),
        PanelKind::Favorites => {
            view_stub(ids::PANELS_KIND_FAVORITES, ids::PANELS_STUB_FAVORITES, ctx)
        }
        PanelKind::Snippets => view_stub(ids::PANELS_KIND_SNIPPETS, ids::PANELS_STUB_SNIPPETS, ctx),
        PanelKind::Todo => view_stub(ids::PANELS_KIND_TODO, ids::PANELS_STUB_TODO, ctx),
        PanelKind::Wiki => view_stub(ids::PANELS_KIND_WIKI, ids::PANELS_STUB_WIKI, ctx),
        // The dock draws the Library panel and its aliases with
        // `library::panel::view`; this arm is only reached by a surface
        // that does not route them there.
        PanelKind::SchLibrary | PanelKind::FootprintLibrary | PanelKind::Library => {
            view_stub(ids::PANELS_KIND_LIBRARY, ids::PANELS_STUB_LIBRARY, ctx)
        }
        PanelKind::PcbNets => pcb_nets::view(ctx),
        PanelKind::History => return history::view_history(ctx),
        PanelKind::NetInspector => return net_inspector::view(ctx),
    };

    panel_scrollable(content).boxed()
}
