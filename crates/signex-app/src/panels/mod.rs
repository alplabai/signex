//! Panel implementations, built on signex-widgets.

use iced::widget::{Column, Space, container, row, scrollable, svg};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use iced_aw::NumberInput;
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::tree_view::{TreeIcon, TreeMsg, TreeNode, TreeView};
use signex_widgets::{button, raw, text};

pub mod annotations;
mod click_away;
pub mod compiled_design;
pub mod components_panel;
mod element_properties;
mod footprint_editor_properties;
pub mod history;
pub mod layer_stack;
mod pcb_layers;
mod placed_edit;
pub mod pre_placement;
mod scroll;
pub(crate) use scroll::{PROPERTIES_SCROLL_ID, panel_scrollable, snap_properties_to_end};
mod properties_parameters;
mod symbol_editor_properties;

// v0.14.x -- domain modules split out of the former single-file panels module.
mod board_sketch_attr;
pub use board_sketch_attr::{BoardSketchAttrMsg, BoardTargetProps, ComponentChoice, LayerChoice};
mod board_bake_preview;
pub use board_bake_preview::{
    BakedHole, BakedKeepout, BakedOutline, BoardBakePreview, BoardBakeRefusal, BoardBakeRows,
};
mod board_sketch_props;
pub use board_sketch_props::{BoardSketchProps, MountingHoleProps};
mod free_motion_summary;
pub use free_motion_summary::FreeMotionSummary;
mod sketch_dof;
pub use sketch_dof::{ConflictSuggestion, SketchDof};
mod sketch_fix;
pub use sketch_fix::SketchFixMsg;
mod board_net_classes;
mod color_field;
mod color_hex;
pub use color_hex::{ColorHexDraft, ColorHexMsg, ColorRowKey, HexEntry, parse_typed_hex};
pub mod command_log;
pub use command_log::{CommandLogMsg, CommandLogState};
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
// The DRC popover of the status bar words its rows as the panel does (#2460).
pub(crate) use drc::row_text as drc_row_text;
mod drc_idle;
pub use drc_idle::BaseRule;
mod drc_rows;
pub use drc_results::{
    DrcCategory, DrcFilter, DrcFilterChoice, DrcPanelMsg, DrcView, DrcWaiveDraft,
};
pub(crate) use empty_state::empty_state_card;
pub(crate) use empty_state::{EmptyStateAction, empty_state_card_with_actions};
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
mod footprint_outline;
mod footprint_place_block;
mod footprint_properties_msg;
pub use footprint_properties_msg::FpPropertiesMsg;
mod footprint_subform_msg;
pub use footprint_subform_msg::{FpSubformMsg, expansion_buffer_key};
mod field_dropdown;
mod field_edits;
pub(crate) mod form_look;
pub(crate) mod graphite;
pub(crate) mod illustration;
pub mod length_matching;
pub use length_matching::{LengthMatchingFacts, LengthMatchingMsg, LengthMatchingUi};
mod library_row;
mod messages;
pub(crate) mod messages_row;
mod multi_selection;
mod navigator;
mod net_class_panel;
mod net_class_table;
pub mod net_inspector;
mod numeric_draft;
pub mod object_list;
pub mod paired;
mod palette;
pub mod panel_groups;
mod paper;
pub mod pcb_board_properties;
pub mod pcb_component;
pub mod pcb_nets;
pub mod pcb_primitive_properties;
pub mod pcb_room_properties;
pub mod polygons;
pub use polygons::{PolygonFacts, PolygonMsg, PolygonsUi};
mod projects;
mod projects_graphite;
mod properties;
mod selection_filter;
pub use selection_filter::{
    FilterEditor, KindCounts, SchFilterMsg, SelectionFilterFacts, board_counts,
};
pub(crate) mod rotation_steps;
mod symbol_context;
pub(crate) mod unions;
mod widgets;
mod window_scope;
mod wire_style_rows;

#[cfg(test)]
mod contrast_tests;
#[cfg(test)]
mod planned_tests;

pub use element_properties::{
    AnnotationField, AnnotationPanelMsg, AnnotationPanelState, DIMENSION_OVERRIDE_INPUT,
    DIMENSION_TEXT_SECTION, DimensionToolForm,
};
pub(crate) use element_properties::{
    BODY_ALTERNATE, BODY_ALTERNATE_MISSING, BODY_NORMAL, BODY_NORMAL_CAN_SWITCH, sheet_file_missing,
};
pub use element_properties::{
    BlanketEdit, FootprintCardFacts, FootprintCardKey, FootprintCardStatus, FootprintCardView,
    FootprintCards, FootprintThumb, NoErcRuleEdit, PartPanelEffect, PartPanelMsg, PartPanelState,
};
pub use element_properties::{
    BlanketNetRow, DirectivePanel, DirectivePanelMsg, DraftField, NetParametersEdit, NetRule,
    OutlineField,
};
pub use element_properties::{HarnessField, HarnessMismatchRow, HarnessPanel, HarnessPanelMsg};
use element_properties::{
    view_child_sheet_properties, view_drawing_properties, view_harness_definitions,
    view_selected_element_properties,
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
    BuiltinParameter, DocumentOptionsMsg, DocumentOptionsUi, PaperEdge, PaperEdgeDraft,
    SheetVariableMsg, TitleBlockDraft, TitleBlockField, TitleBlockSource, shown_title_block_value,
    title_block_source, typed_mm,
};
pub(super) use properties_parameters::{
    canvas_font_popup, empty_section_row, font_style_row, form_check_row, form_check_row_shortcut,
    form_choice_row, form_edit_row, form_edit_row_refusable, form_font_link_row, form_input_row,
    form_label, form_pick_row, justification_grid, label_anchor, net_params_add_bar,
    net_params_tabs, seg_btn, thin_sep,
};
use properties_parameters::{view_properties_general, view_properties_parameters};
use symbol_editor_properties::view_symbol_editor_properties;
/// The Pin table document tab's body (mockup row 122b) and the rows its
/// bulk actions act on.
pub(crate) use symbol_editor_properties::{pin_table_acted_rows, view_pin_table};

// -- Re-exports: keep the `crate::panels::*` public surface stable and the
//    sibling `super::` / `crate::panels::` paths resolving after the split. --
pub use annotations::{AnnotationsMsg, AnnotationsState, AnnotationsView};
pub use color_field::{ColorFieldProps, color_field};
pub use context::{FootprintChoiceItem, PINS_SECTION_KEY, PanelContext, SymbolChoiceItem};
pub use erc_waivers::{ErcViolationWaiverEntry, ErcWaiveDraft, ErcWaiverMsg, ErcWaiverStatus};
pub use footprint_context::{
    ArrayKindSummary, ArrayParamField, ArraySummary, BgaConfigSummary, CutoutSummary,
    FootprintEditorPanelContext, FootprintLibInternalRow, FootprintLibSibling, FootprintModeKind,
    FootprintPadSummary, FootprintSelectedSilkSummary, FootprintSketchEntitySummary,
    FootprintSolveSummary, KeepoutKindFlag, KeepoutSummary, NumberingSchemeKindUi,
    OverConstraintSummary, PadShapeParamSummary, PourSummary, RejectedConflictSummary,
    SilkKindGeometry, SketchPadAttrSummary, SnapOptionFlag,
};
pub use footprint_context::{
    PATTERN_PREVIEW_MAX_INSTANCES, SketchStateSummary, number_mask_buffer_key,
};
pub use footprint_outline::{OutlineSegment, PadOutlineSummary, segments as outline_segments};
pub use footprint_place_block::FootprintPlaceBlock;
pub use illustration::{HoverSubject, PanelIllustrationHover};
pub use layer_stack::{LayerStackFacts, LayerStackMsg, StackField};
pub use library_row::{ComponentSheet, LibraryRowDetail};
pub(super) use library_row::{view_component_sheet, view_library_row_properties};
mod doc_grid;
pub(crate) use doc_grid::typed_pitch_mm;
pub use doc_grid::{DocGridField, DocGridMsg};
mod typed_pick;
pub use messages::{FootprintFeatureEdit, PanelMsg, SheetFrameEdit};
pub use multi_selection::{
    MultiEditMsg, MultiField, MultiKind, SelectionForm, SelectionFormMsg, SelectionKinds,
    edit_from_text_in as multi_edit_from_text,
};
pub use navigator::{NavigatorModel, NavigatorMsg, NavigatorRow, SheetNumbers};
pub use net_class_panel::{
    LabelClassPick, NetClassChoice, NetClassDraft, NetClassField, NetClassMsg, NetClassPanel,
    net_class_choice, net_class_choices, view_net_classes_section,
};
pub use net_inspector::{NetInspectorMsg, NetInspectorState, NetInspectorView};
pub use object_list::{
    KindChip, ObjectKind, ObjectListMsg, ObjectListState, ObjectListStatus, ObjectListView,
    ObjectRef, ObjectRow, ObjectScope,
};
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
pub use pcb_nets::{PcbNetsSummary, PcbPanelMode, PcbPanelMsg, PcbPanelUi};
pub use pcb_primitive_properties::{PcbPrimitiveFacts, PcbPrimitiveMsg};
pub use placed_edit::{
    NoteDraft, NoteLayout, PlacedDraft, PlacedEditMsg, PlacedField, PlacedPanel, PortNameDraft,
    RunNet,
};
pub use pre_placement::{
    PlacementPosition, PositionField, PrePlacementData, PrePlacementKind, PrePlacementPositionMsg,
    PrePlacementStyle,
};
#[cfg(test)]
pub(crate) use projects::NOTEBOOK_ROW_KEY;
pub(super) use projects::view_projects;
pub use projects::{DiagramRow, LibraryNodeInfo, ProjectPanelInfo, SheetInfo, build_project_tree};
pub(crate) use projects::{MISSING_MARK, UNREADABLE_MARK};
pub use projects_graphite::{ProjectsMsg, ProjectsView};
pub use properties::DrawingFieldId;
pub(super) use properties::{
    PROPERTY_CONTROL_PORTION, PROPERTY_LABEL_COLUMN, PROPERTY_LABEL_WIDTH, PROPERTY_ROW_GAP,
    PROPERTY_ROW_PAD_X, view_properties,
};
pub use symbol_context::{
    GraphicFieldId, GraphicKindSummary, GraphicSummary, SymbolDeleteRebind, SymbolDeleteWarning,
    SymbolDisplayOptions, SymbolEditorPanelContext, SymbolEditorSelection, SymbolFileEntry,
    SymbolPinDetails, SymbolPinSummary, SymbolUsageEntry, SymbolUsageKey,
};
pub(super) use widgets::{
    collapsible_section_header, form_edit_row_f64, is_section_collapsed, part_tree_row,
    prop_kv_row, property_label, shape_fill_row, view_stub,
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
    /// Every catalog command this session ran (#2458).
    CommandLog,
    Drc,
    LayerStack,
    /// The layers of the active tab (#2223, #2359): a board's layers with
    /// their eyes and the active layer, or a footprint's layers by side.
    Layers,
    NetClasses,
    Variants,
    OutputJobs,
    /// Every object of the sheet or the project, listed and selectable
    /// (#1259).
    ObjectList,
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
    /// The compact Length Matching panel beside the board (mockup rows 7
    /// and 107): the board's length-matching groups while routing.
    LengthMatching,
    /// VSCode-style per-file Git history. Right-dock surface that
    /// follows the active tab and shows the file's last 50 commits
    /// via `signex_widgets::history_pane`.
    History,
    /// Every net of the active document's project from the stitched
    /// netlist, filterable, each expandable to its pins, labels, ports and
    /// sheets; a click goes to the member (#1269).
    NetInspector,
    /// The compiled hierarchy: repeated sheets expanded into channels, with
    /// part and net counts per sheet (#1308).
    CompiledDesign,
    /// Named unions of the sheet in view or the project (mockup row 158).
    Unions,
    /// Every annotation block of the project's Design Notebook files,
    /// filterable; a reference chip goes to its part or net (mockup row 165).
    Annotations,
    /// A panel an installed extension registered (#2495). Not in
    /// [`ALL_PANELS`]; the dock draws it through the extension's view hook.
    Extension(crate::extension::points::ExtensionPanelKind),
}

/// Every panel kind, in the panel list's order. The list leaves out the
/// tool documents, which open as document tabs (DECISIONS 2026-11-18,
/// mockup row 7), and To-Do, which folds into the Design Notebook
/// ([`panel_groups::panel_group`]).
pub const ALL_PANELS: &[PanelKind] = &[
    PanelKind::Projects,
    PanelKind::Components,
    PanelKind::Library,
    PanelKind::Navigator,
    PanelKind::Annotations,
    PanelKind::Properties,
    PanelKind::Filter,
    PanelKind::ObjectList,
    PanelKind::Erc,
    PanelKind::Messages,
    PanelKind::CommandLog,
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
    PanelKind::LengthMatching,
    PanelKind::History,
    PanelKind::NetInspector,
    PanelKind::CompiledDesign,
    PanelKind::Unions,
];

impl PanelKind {
    /// Whether this panel requires a schematic document to be open. Filter
    /// follows the active editor, a schematic's or a board's (Graphite mockup #55).
    pub fn needs_schematic(self) -> bool {
        matches!(
            self,
            PanelKind::Navigator
                | PanelKind::Properties
                | PanelKind::Erc
                | PanelKind::ObjectList
                | PanelKind::BomStudio
                | PanelKind::Snippets
                | PanelKind::Variants
                | PanelKind::OutputJobs
        )
    }

    /// Whether this panel requires a PCB document to be open. Net Classes
    /// edits the project's table, so it needs a project, not a board (#2416,
    /// #2417). The panel list also offers Layers on a footprint tab.
    pub fn needs_pcb(self) -> bool {
        matches!(
            self,
            PanelKind::Drc
                | PanelKind::LayerStack
                | PanelKind::Layers
                | PanelKind::PcbNets
                | PanelKind::LengthMatching
        )
    }

    /// The Library panel kind an alias row opens it in (alias rows kept by
    /// Caner, 2026-11-20): SCH Library opens Symbols and Footprint Library
    /// opens Footprints.
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
            PanelKind::CommandLog => ids::PANELS_KIND_COMMAND_LOG,
            PanelKind::Drc => ids::PANELS_KIND_DRC,
            PanelKind::LayerStack => ids::PANELS_KIND_LAYER_STACK,
            PanelKind::Layers => ids::PANELS_KIND_LAYERS,
            PanelKind::NetClasses => ids::PANELS_KIND_NET_CLASSES,
            PanelKind::Variants => ids::PANELS_KIND_VARIANTS,
            PanelKind::ObjectList => ids::PANELS_KIND_OBJECT_LIST,
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
            PanelKind::LengthMatching => ids::PANELS_PAIRED_LENGTH_MATCHING,
            PanelKind::History => ids::PANELS_KIND_HISTORY,
            PanelKind::NetInspector => ids::PANELS_KIND_NET_INSPECTOR,
            PanelKind::CompiledDesign => ids::PANELS_KIND_COMPILED_DESIGN,
            PanelKind::Unions => ids::PANELS_KIND_UNIONS,
            PanelKind::Annotations => ids::PANELS_KIND_ANNOTATIONS,
            PanelKind::Extension(panel) => panel.title(),
        }
    }

    /// The row's name in the panel list: the PCB panel is listed as Nets
    /// (mockup row 7), its dock title stays PCB.
    pub fn list_message(self) -> signex_i18n::MessageId {
        match self {
            PanelKind::PcbNets => signex_i18n::ids::PANELS_LIST_ROW_NETS,
            other => other.message(),
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
            PanelKind::CommandLog => "Command Log",
            PanelKind::Drc => "DRC",
            PanelKind::LayerStack => "Layer Stack",
            PanelKind::Layers => "Layers",
            PanelKind::NetClasses => "Net Classes",
            PanelKind::Variants => "Variants",
            PanelKind::ObjectList => "Object List",
            PanelKind::BomStudio => "BOM Studio",
            PanelKind::Favorites => "Favorites",
            PanelKind::Snippets => "Snippets",
            PanelKind::Todo => "To-Do",
            PanelKind::Wiki => "Wiki",
            PanelKind::OutputJobs => "Export Jobs",
            PanelKind::Library => "Library",
            PanelKind::SchLibrary => "SCH Library",
            PanelKind::FootprintLibrary => "Footprint Library",
            PanelKind::PcbNets => "PCB",
            PanelKind::LengthMatching => "Length Matching",
            PanelKind::History => "History",
            PanelKind::NetInspector => "Net Inspector",
            PanelKind::CompiledDesign => "Compiled Design",
            PanelKind::Unions => "Unions",
            PanelKind::Annotations => "Annotations",
            // No English name of its own: the layout key names it.
            PanelKind::Extension(panel) => panel.key(),
        }
    }
}

/// An icon action a panel puts in its dock title bar.
pub struct TitleAction {
    pub icon: svg::Handle,
    pub tip: signex_i18n::Localized,
    /// `None` draws the action disabled.
    pub on_press: Option<PanelMsg>,
}

/// The title-bar actions of `kind`: Clear of the ERC panel (#85) while it
/// holds results, Copy and Clear of the Messages panel (#86), Recompile of
/// Compiled Design (mockup row 154), Refresh of Annotations (mockup row 165),
/// Create Union of Unions (mockup row 158), the + of Projects (mockup row 19): Add New,
/// or New Project with no project open, and Pause, Save, Clear and More of
/// the Command Log (mockup row 161).
pub fn title_actions(kind: PanelKind, ctx: &PanelContext) -> Vec<TitleAction> {
    use signex_i18n::{ids, menu_ids, t};
    let tid = ctx.theme_id;
    match kind {
        PanelKind::CommandLog => command_log::title_actions(ctx),
        PanelKind::Projects => vec![TitleAction {
            icon: crate::icons::icon_plus(tid),
            tip: if ctx.project_tree.is_empty() {
                t!(ctx.i18n, ids::PANELS_PROJECTS_NEW_PROJECT)
            } else {
                t!(ctx.i18n, menu_ids::CTX_TREE_ADD_NEW)
            },
            on_press: Some(PanelMsg::Projects(ProjectsMsg::AddNewPressed)),
        }],
        PanelKind::Erc if ctx.erc_ran || !ctx.erc_diagnostics.is_empty() => vec![TitleAction {
            icon: crate::icons::icon_dd_delete(tid),
            tip: t!(ctx.i18n, ids::PANELS_ERC_CLEAR_TOOLTIP),
            on_press: Some(PanelMsg::ClearErc),
        }],
        PanelKind::Messages => vec![
            TitleAction {
                icon: crate::icons::icon_dd_copy(tid),
                tip: t!(ctx.i18n, ids::PANELS_MESSAGES_COPY_TOOLTIP),
                on_press: Some(PanelMsg::MessagesView(MessagesPanelMsg::Copied)),
            },
            TitleAction {
                icon: crate::icons::icon_dd_delete(tid),
                tip: t!(ctx.i18n, ids::PANELS_MESSAGES_CLEAR_TOOLTIP),
                on_press: Some(PanelMsg::MessagesView(MessagesPanelMsg::Cleared)),
            },
        ],
        PanelKind::CompiledDesign => vec![TitleAction {
            icon: crate::icons::icon_chrome_refresh(tid),
            tip: t!(ctx.i18n, ids::PANELS_COMPILED_DESIGN_RECOMPILE),
            on_press: (ctx.compiled_design.panel.status
                != compiled_design::CompiledStatus::NoProject)
                .then(|| compiled_design::msg(crate::app::CompiledDesignMsg::CompileRequested)),
        }],
        PanelKind::History if ctx.history.active_path.is_some() => vec![TitleAction {
            icon: crate::icons::icon_chrome_refresh(tid),
            tip: t!(ctx.i18n, ids::PANELS_HISTORY_REFRESH),
            on_press: Some(PanelMsg::History(history::HistoryPanelMsg::RefreshPressed)),
        }],
        PanelKind::Annotations if ctx.annotations.status == annotations::ListStatus::Ready => {
            vec![TitleAction {
                icon: crate::icons::icon_chrome_refresh(tid),
                tip: t!(ctx.i18n, ids::ANNOTATIONS_REFRESH),
                on_press: Some(PanelMsg::Annotations(AnnotationsMsg::RefreshRequested)),
            }]
        }
        PanelKind::Unions if ctx.unions.has_sheet => vec![TitleAction {
            icon: crate::icons::icon_plus(tid),
            tip: t!(ctx.i18n, ids::UNIONS_PANEL_CREATE),
            on_press: ctx.unions.can_create.then_some(PanelMsg::Unions(
                crate::app::unions::UnionsMsg::CreateRequested,
            )),
        }],
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
        PanelKind::Navigator => navigator::view(ctx),
        PanelKind::Properties => view_properties(ctx),
        // The active editor's selection filter (Graphite mockup #55): a schematic's kinds
        // and presets (mockup rows 40 and 55) or a board's kinds, else the empty card.
        PanelKind::Filter => selection_filter::filter_view(ctx),
        PanelKind::Erc => view_erc(ctx),
        PanelKind::Messages => view_messages(ctx),
        // The log keeps its header and footer in place and scrolls its rows.
        PanelKind::CommandLog => return command_log::view(ctx),
        PanelKind::Drc => drc_results::view(ctx),
        // Opens as a document tab (`show_panel`); never docked.
        PanelKind::LayerStack => iced::widget::Space::new().boxed(),
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
        PanelKind::ObjectList => return object_list::view(ctx),
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
        // Its list and detail scroll on their own.
        PanelKind::PcbNets => return pcb_nets::view(ctx),
        PanelKind::LengthMatching => paired::length_matching_view(ctx),
        PanelKind::History => return history::view_history(ctx),
        PanelKind::NetInspector => return net_inspector::view(ctx),
        PanelKind::CompiledDesign => return compiled_design::view(ctx),
        PanelKind::Unions => return unions::view(ctx),
        PanelKind::Annotations => return annotations::view(ctx),
        // The dock and the detached window draw it with the extension's
        // view hook; this arm is only reached by a surface that does not.
        PanelKind::Extension(_) => return iced::widget::Space::new().boxed(),
    };

    let scroller = panel_scrollable(content);
    if kind == PanelKind::Properties {
        return scroller.id(PROPERTIES_SCROLL_ID).boxed();
    }
    scroller.boxed()
}
