//! Library subsystem message tree.
//!
//! Mirrors the existing `Message` → dispatcher → handler split used across
//! the rest of `signex-app`. The top-level `LibraryMessage` is folded into
//! [`crate::app::contracts::Message::Library`]; each sub-enum routes to a
//! purpose-built handler.
//!
//! Keep variants small and copy-cheap where possible — these messages
//! ride through the entire iced update tree, including for the multi-
//! window editor surface (one editor window per `ComponentId`).

use std::path::PathBuf;

use signex_library::{
    AlternateStatus, BodyShape, ComponentClass, ComponentSummary, DistributorSource,
    LifecycleState, PrimitiveRef, SimKind, SimModel,
};
use uuid::Uuid;

use super::state::PreviewTab;

// WS-5 (DBLib): kept as type aliases until WS-6 retargets the editor
// at `ComponentPreviewState`. The original `ComponentId` was a
// `uuid::Uuid` newtype; `Version` was a `u32` revision counter. Both
// fold away once the row tier ships everywhere.
#[allow(dead_code)]
pub type ComponentId = Uuid;
#[allow(dead_code)]
pub type Version = u32;

mod library_message;
pub use library_message::LibraryMessage;

/// User choice from the close-library confirmation modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CloseLibraryChoice {
    SaveAll,
    DiscardAll,
    Cancel,
}

/// Component Preview inner messages. The surface is preview-only
/// for Symbol/Footprint; the canvas messages stay defined here so
/// the standalone `.snxsym` / `.snxfpt` document tabs can reuse
/// them, but they no longer dispatch through the Component Preview
/// tab.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditorMsg {
    /// User clicked a Preview tab pill (Preview / Parameters / Supply /
    /// Datasheet / Simulation).
    SelectTab(PreviewTab),
    /// Save the current row to the table — calls
    /// `adapter.update_row(&table, &row, "edit message")`.
    SaveDraft,
    /// Same as [`SaveDraft`] for the Component Preview surface — kept
    /// distinct so future Commit semantics (lifecycle promotion etc.)
    /// can layer in without renaming the SaveDraft message.
    Commit,
    /// Open the review-request UI.
    SubmitForReview,
    SubmitForReviewNotesChanged(String),
    SubmitForReviewConfirm,
    SubmitForReviewCancel,
    SubmitForReviewResult(Result<(), String>),
    /// Footer "Where Used" — switches the active preview tab to
    /// Preview (the where-used footer line lives there).
    OpenWhereUsedTab,
    /// User dismissed the preview tab (Close X or Ctrl+W).
    CloseEditor,

    // ── Datasheet tab ────────────────────────────────────────
    /// Switch the datasheet picker between URL / Pinned PDF modes.
    DatasheetSetMode(crate::library::editor::datasheet_picker::DatasheetMode),
    /// Live edit of the URL field on the Datasheet tab.
    DatasheetSetUrl(String),
    /// Open the Pinned-PDF upload dialog.
    DatasheetUploadDialog,
    /// Async result of the Pinned-PDF upload — `Some((bytes, filename))`
    /// on pick, `None` on cancel.
    DatasheetUploadResult(Option<(Vec<u8>, String)>),

    // ── Component-level setters ─────────────────────────────
    /// Set the row's lifecycle state from the Preview tab header.
    SetLifecycle(LifecycleState),

    // ── Pin Map (Preview-tab inline subsection) ─────────────
    /// Toolbar — clear every override and revert to default 1:1 by
    /// pin/pad number equality.
    PinMapAutoMatchByNumber,
    /// Toolbar — match by pin name → pad number where unambiguous.
    /// Stub: emits a tracing warn until the name-based heuristic
    /// ships in a follow-up patch (see plan §12 task list).
    PinMapAutoMatchByName,
    /// Toolbar — drop every entry in `Revision::pin_map_overrides`.
    /// Equivalent to `PinMapAutoMatchByNumber` for the v0.9 algorithm.
    PinMapClearOverrides,
    /// Click "[Override]" on a row — expands the inline editor for
    /// that pin's row. Carries the symbol pin number.
    PinMapOpenOverrideEdit(String),
    /// Live edit of the override pad-number text input. The dispatcher
    /// keeps the buffer on `PinMapTabState.override_buf`.
    PinMapOverrideBufChanged {
        pin: String,
        value: String,
    },
    /// User clicked "Save" inside the inline editor — push a
    /// `PinPadOverride` onto the active draft.
    PinMapAddOverride {
        pin: String,
        pad: String,
    },
    /// User clicked "Cancel" inside the inline editor — discard the
    /// edit buffer + collapse the row.
    PinMapCancelOverrideEdit,
    /// User clicked "Remove" on an overridden row — drops that pin's
    /// entry from `Revision::pin_map_overrides`.
    PinMapRemoveOverride {
        pin: String,
    },
    /// Fire-and-forget save of the active symbol primitive — typically
    /// chained off SaveDraft via the dispatcher. Boxed so the
    /// containing enum stays cheap to clone and propagate.
    SaveSymbol(uuid::Uuid, Box<signex_library::Symbol>),
    // ── Footprint canvas (used by the standalone .snxfpt tab) ──
    /// Namespaced footprint-canvas edit (ADR-0001 D3). The footprint
    /// canvas program emits this; `footprint_msg_to_primitive_msg`
    /// bridges it to `PrimitiveEdit::Footprint` for the standalone
    /// `.snxfpt` tab.
    Footprint(FootprintEditorMsg),
    /// Fire-and-forget save of the active footprint primitive. Boxed
    /// so the containing enum stays cheap to clone and propagate.
    SaveFootprint(uuid::Uuid, Box<signex_library::Footprint>),
    /// Body 3D editor pane — set extruded body height (mm).
    SetBodyHeight(f32),
    /// Body 3D editor pane — set body offset above PCB (mm).
    SetBodyOffsetZ(f32),
    /// Body 3D editor pane — set the body-top RGBA colour.
    SetBodyTopColor([f32; 4]),
    /// Body 3D editor pane — set the body-side RGBA colour.
    SetBodySideColor([f32; 4]),
    /// Body 3D editor pane — set the procedural shape variant.
    SetBodyShape(BodyShape),
    /// STEP attach — open the system file picker.
    StepAttachDialog,
    /// Async file picker returned. `Some((bytes, filename))` on pick,
    /// `None` on cancel.
    StepAttachResult(Option<(Vec<u8>, String)>),
    /// Drop the existing STEP attachment from the footprint primitive.
    StepAttachRemove,

    // ── Supply tab ────────────────────────────────────────────
    // Primary MPN
    /// Edit the primary MPN's manufacturer string.
    SupplyPrimarySetManufacturer(String),
    /// Edit the primary MPN's MPN string.
    SupplyPrimarySetMpn(String),
    /// Pick the primary MPN's approval status.
    SupplyPrimarySetStatus(AlternateStatus),
    /// Edit the primary MPN's free-form notes.
    SupplyPrimarySetNotes(String),

    // Alternates
    /// Append a fresh blank alternate row.
    SupplyAlternateAdd,
    /// Edit the manufacturer of the alternate at `idx`.
    SupplyAlternateSetManufacturer {
        idx: usize,
        value: String,
    },
    /// Edit the MPN of the alternate at `idx`.
    SupplyAlternateSetMpn {
        idx: usize,
        value: String,
    },
    /// Pick the approval status of the alternate at `idx`.
    SupplyAlternateSetStatus {
        idx: usize,
        value: AlternateStatus,
    },
    /// Edit the free-form notes of the alternate at `idx`.
    SupplyAlternateSetNotes {
        idx: usize,
        value: String,
    },
    /// Drop the alternate row at `idx`.
    SupplyAlternateRemove {
        idx: usize,
    },

    // Distributor listings
    /// Append a fresh blank distributor listing row.
    SupplyListingAdd,
    /// Pick the distributor source for the listing at `idx`. The
    /// dispatcher converts `DistributorSource` to the canonical string
    /// stored on `DistributorListing.distributor`.
    SupplyListingSetDistributor {
        idx: usize,
        value: DistributorSource,
    },
    /// Edit the SKU of the distributor listing at `idx`.
    SupplyListingSetSku {
        idx: usize,
        value: String,
    },
    /// Edit the URL of the distributor listing at `idx`. Empty string
    /// clears the field back to `None`.
    SupplyListingSetUrl {
        idx: usize,
        value: String,
    },
    /// Drop the distributor listing row at `idx`.
    SupplyListingRemove {
        idx: usize,
    },
    // ── Parameters tab ────────────────────────────────────────
    /// Set a `ParamValue::Text` parameter's value directly. Text inputs
    /// can flush on every keystroke without a parse step.
    ParamSetText {
        name: String,
        value: String,
    },
    /// Live-update the per-row edit buffer for a `ParamValue::Number`
    /// row. The buffer lives on `ComponentEditorState.params_edit_buf`;
    /// the value is committed via `ParamCommitNumber`.
    ParamSetNumberBuf {
        name: String,
        buf: String,
    },
    /// Commit the live buffer for a `ParamValue::Number` row.
    ParamCommitNumber {
        name: String,
    },
    /// Live-update the per-row edit buffer for a `ParamValue::Measurement`
    /// row's value field.
    ParamSetMeasurementBuf {
        name: String,
        buf: String,
    },
    /// Commit the live buffer for a `ParamValue::Measurement` row.
    ParamCommitMeasurement {
        name: String,
        unit: String,
    },
    /// Toggle a `ParamValue::Bool` parameter.
    ParamSetBool {
        name: String,
        value: bool,
    },
    /// Drop a parameter from `draft.parameters`.
    ParamRemove {
        name: String,
    },
    /// Add a custom parameter row with an empty value of the chosen kind.
    ParamAddCustom {
        name: String,
        kind: ParamKindMsg,
    },
    // ── Simulation tab ────────────────────────────────────────
    /// Toggle the "Has SPICE Model" checkbox. `true` constructs a fresh
    /// `SimModel` and binds it via `Revision::sim_ref`; `false` clears
    /// both `editor.sim` and `editor.draft.sim_ref`.
    SimSetEnabled(bool),
    /// SPICE dialect picker — Spice3 / Ngspice / LtSpice / VerilogA.
    SimSetKind(SimKind),
    /// Live edit of the SimModel `name` field.
    SimSetName(String),
    /// Multi-line edit on the SPICE deck `text_editor`. Action is
    /// applied to `editor.sim_body`; the resulting text mirrors back
    /// onto `editor.sim?.body` so persistence picks it up on save.
    SimBodyAction(iced::widget::text_editor::Action),
    /// Set or clear the SPICE node binding for one symbol pin number.
    /// Empty `value` removes the key from `default_node_map`.
    SimSetPinNode {
        pin_number: String,
        value: String,
    },
    /// Fire-and-forget save of the active SimModel primitive.
    SaveSim(uuid::Uuid, Box<SimModel>),
}

/// Pure-data alias for `ParamKind` so messages don't depend on
/// `signex_library::ParamKind` at the message layer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ParamKindMsg {
    Text,
    Number,
    Bool,
    /// Carries the unit string ("ohm", "F", "V", …).
    Measurement(String),
}

/// Tool selection on the Symbol canvas — pure-data alias for the
/// canvas's own `SymbolTool` so messages don't depend on the canvas
/// module type tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SymbolToolMsg {
    Select,
    AddPin,
    PlaceRectangle,
    PlaceLine,
    PlaceCircle,
    PlaceArc,
    PlaceText,
    PlacePolygon,
}

/// v0.13.3 — selection-aware constraint kind tag. The dispatcher
/// resolves these against the editor's primary + secondary
/// selection slots into the matching `ConstraintKind` and emits the
/// SketchEdit. Tags that don't apply to the current selection are
/// no-ops in the dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchConstraintTag {
    /// 1 Point selected → fix it in place.
    Fixed,
    /// 2 Points selected → make them coincident.
    Coincident,
    /// 2 Points selected + dimension input → DistancePtPt(target_mm).
    DistancePtPt,
    /// 1 Line selected → horizontal.
    Horizontal,
    /// 1 Line selected → vertical.
    Vertical,
    /// 2 Lines selected → parallel.
    Parallel,
    /// 2 Lines selected → perpendicular.
    Perpendicular,
    /// 2 Lines selected → equal length.
    EqualLength,
    /// 1 Point + 1 Line selected → point on line.
    PointOnLine,
    /// 1 Point + 1 Line selected → midpoint.
    Midpoint,
    /// 1 Line + 1 Arc selected → line tangent to arc.
    TangentLineArc,
    /// 2 Arcs selected → arcs tangent to each other (external).
    TangentArcArc,
    /// 2 Lines selected + dimension input → Angle(target_deg).
    Angle,
    /// 2 Circles/Arcs selected → equal radius.
    EqualRadius,
    /// 1 Point + 1 Arc selected → point on arc.
    PointOnArc,
    /// 1 Point + 1 Line selected + dimension input →
    /// DistancePtLine(target_mm).
    DistancePtLine,
    /// 1 Point + 1 Circle/Arc selected + dimension input →
    /// DistancePtCircle(target_mm).
    DistancePtCircle,
    /// 2 Points + 1 Line (in the extra slot) → symmetric about line.
    SymmetricAboutLine,
    /// 3 Points (third in the extra slot) → symmetric about the
    /// third (centre) point.
    SymmetricAboutPoint,
}

/// v0.16.2 — role tag attached to a sketch entity. The Sketch-mode
/// inspector emits one of these via
/// [`FootprintEditorMsg::SketchSetRole`]; the dispatcher
/// clears every `*Attr` slot on the target entity and writes the
/// matching one with sensible defaults. Bake auto-emits whatever
/// geometry the role implies (pad / silk segment / courtyard
/// polygon / mask opening / pour / paste aperture / keepout / board
/// cutout). `Pad` is only valid on a Point — non-Point entities
/// fall through as a silent no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleTag {
    /// Clear every `*Attr` slot on the entity.
    Unassigned,
    /// `PadAttr` — bakes as an SMD pad. Point-only.
    Pad,
    /// `SilkAttr { layer: TopSilk }` — top-side silkscreen line/arc.
    SilkTop,
    /// `SilkAttr { layer: BottomSilk }`.
    SilkBottom,
    /// `CourtyardAttr` — closed loop becomes the courtyard polygon.
    Courtyard,
    /// `KeepoutAttr` with `NO_ROUTING` defaults on TopCopper.
    Keepout,
    /// `BoardCutoutAttr { through: true }` — board cutout polygon.
    Cutout,
    /// `MaskOpeningAttr { layer: TopSolderMask }`.
    MaskOpeningTop,
    /// `MaskOpeningAttr { layer: BottomSolderMask }`.
    MaskOpeningBottom,
    /// `MaskExcludeAttr { layer: TopSolderMask }`.
    MaskExcludeTop,
    /// `MaskExcludeAttr { layer: BottomSolderMask }`.
    MaskExcludeBottom,
    /// `PourAttr { layer: TopCopper, .. }` with thermal-relief defaults.
    PourTop,
    /// `PourAttr { layer: BottomCopper, .. }`.
    PourBottom,
    /// `PasteApertureAttr { layer: TopPaste }`.
    PasteApertureTop,
    /// `PasteApertureAttr { layer: BottomPaste }`.
    PasteApertureBottom,
}

impl RoleTag {
    /// Display order for the inspector's pick_list. Mirrors the
    /// docstring order on the enum.
    pub const ALL: &'static [RoleTag] = &[
        RoleTag::Unassigned,
        RoleTag::Pad,
        RoleTag::SilkTop,
        RoleTag::SilkBottom,
        RoleTag::Courtyard,
        RoleTag::Keepout,
        RoleTag::Cutout,
        RoleTag::MaskOpeningTop,
        RoleTag::MaskOpeningBottom,
        RoleTag::MaskExcludeTop,
        RoleTag::MaskExcludeBottom,
        RoleTag::PourTop,
        RoleTag::PourBottom,
        RoleTag::PasteApertureTop,
        RoleTag::PasteApertureBottom,
    ];

    /// Human-readable label rendered in the inspector dropdown.
    pub fn label(self) -> &'static str {
        match self {
            RoleTag::Unassigned => "Unassigned",
            RoleTag::Pad => "Pad",
            RoleTag::SilkTop => "Silk · Top",
            RoleTag::SilkBottom => "Silk · Bottom",
            RoleTag::Courtyard => "Courtyard",
            RoleTag::Keepout => "Keepout",
            RoleTag::Cutout => "Board Cutout",
            RoleTag::MaskOpeningTop => "Mask Opening · Top",
            RoleTag::MaskOpeningBottom => "Mask Opening · Bottom",
            RoleTag::MaskExcludeTop => "Mask Exclude · Top",
            RoleTag::MaskExcludeBottom => "Mask Exclude · Bottom",
            RoleTag::PourTop => "Pour · Top",
            RoleTag::PourBottom => "Pour · Bottom",
            RoleTag::PasteApertureTop => "Paste · Top",
            RoleTag::PasteApertureBottom => "Paste · Bottom",
        }
    }
}

impl std::fmt::Display for RoleTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Selection target on the Symbol canvas — pure-data version of
/// `editor::symbol::state::SymbolSelection`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SymbolSelectionMsg {
    Pin(usize),
    FieldReference,
    FieldValue,
    /// A placed `SymbolGraphic` at the given index in the active
    /// symbol's `graphics` vector. Drives the right-dock Properties
    /// panel's Graphic branch.
    Graphic(usize),
    /// All pins and graphics — emitted by Ctrl+A on the symbol canvas.
    All,
    /// A subset of pins and graphics from a rubber-band box selection.
    Multiple {
        pin_indices: Vec<usize>,
        graphic_indices: Vec<usize>,
    },
}

/// Symbol field key — pure-data alias of
/// `editor::symbol::state::FieldKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FieldKeyMsg {
    Reference,
    Value,
}

/// Resize-handle identity for a Symbol graphic — pure-data alias of
/// `editor::symbol::state::GraphicHandle`. Carried by
/// `SymbolEditorMsg::MoveGraphicHandle` so the dispatcher
/// knows which handle of which graphic the canvas is dragging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum GraphicHandleMsg {
    /// Rectangle corner — `0=TL, 1=TR, 2=BR, 3=BL` (Standard y-up).
    RectCorner(u8),
    /// Rectangle edge midpoint — `0=Top, 1=Right, 2=Bottom, 3=Left`.
    RectEdge(u8),
    /// Line endpoint — `0=from, 1=to`.
    LineEndpoint(u8),
    /// Circle radius handle.
    CircleRadius,
    /// Arc start point on the circumference.
    ArcStart,
    /// Arc end point on the circumference.
    ArcEnd,
    /// Text anchor / `position` field.
    TextAnchor,
    /// Polygon vertex at the given index — pure-data alias of
    /// `editor::symbol::state::GraphicHandle::PolygonVertex`.
    PolygonVertex(u32),
}

/// Pivot mode for Symbol-graphic rotate operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum SymbolRotatePivotMsg {
    /// Legacy orbit around world origin `(0, 0)`.
    WorldOrigin,
    /// Rotate around each selected graphic's geometry center.
    GeometryCenter,
}

/// What the cursor was over at right-click time — pure-data alias of
/// `editor::symbol::state::SymbolContextTarget`. Carried by
/// `SymbolEditorMsg::ShowContextMenu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolContextTargetMsg {
    Empty,
    Pin(usize),
    Graphic(usize),
}

/// Which context-menu submenu is accordion-expanded — pure-data alias
/// of `editor::symbol::state::SymbolContextSubmenu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolContextSubmenuMsg {
    Place,
}

mod footprint;
mod symbol;
pub use footprint::FootprintEditorMsg;
pub use symbol::SymbolEditorMsg;

/// A canvas-editor mutation, namespaced by surface. The former flat
/// `PrimitiveEditorMsg` (134 variants) is split into per-surface enums
/// ([`FootprintEditorMsg`] / [`SymbolEditorMsg`], ADR-0001 D3); `Save` is
/// shared. Carried by [`LibraryMessage::PrimitiveEditorEvent`]; path-keyed
/// dispatch (`handle_primitive_editor_event`) routes each surface to the
/// matching editor state per the active tab's [`crate::app::TabKind`].
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PrimitiveEdit {
    /// Footprint canvas editing.
    Footprint(FootprintEditorMsg),
    /// Symbol canvas editing.
    Symbol(SymbolEditorMsg),
    /// Explicit "Save this primitive tab to disk" — fires from the editor's
    /// Save button; Ctrl+S also routes here for primitive tabs.
    Save,
}

/// Picker modal interaction sub-message tree.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PickerMsg {
    FilterChanged(String),
    SelectComponent(ComponentSummary),
    PlaceSelected,
}

/// Edit Component Details modal sub-message tree — keeps
/// `LibraryMessage` digestible by grouping all the per-field setters
/// under a single sub-enum.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BrowserEditMsg {
    SetInternalPn(String),
    SetClass(ComponentClass),
    SetState(LifecycleState),
    SetDatasheetUrl(String),
    SetManufacturer(String),
    SetMpn(String),
    /// Live edit of a parameter row's value or unit. The dispatcher
    /// keeps the buffer keyed by `key`; commit happens on
    /// `BrowserEditMsg::CommitParam`.
    SetParamValue {
        key: String,
        value: String,
    },
    SetParamUnit {
        key: String,
        unit: String,
    },
    /// Commit the live param buffer for `key` to the draft.
    CommitParam {
        key: String,
    },
    /// Append a fresh blank parameter row.
    AddParam,
    /// Drop the parameter row at `key`.
    DeleteParam {
        key: String,
    },
    /// Live edit of the comma-separated tags string (Stage 18). Stored
    /// as `parameters["tags"]` on save — a free-form `ParamValue::Text`
    /// preserves the raw user-typed list.
    SetTags(String),
    /// Open the Symbol primitive picker scoped to this edit modal.
    OpenSymbolPicker,
    /// Open the Footprint primitive picker scoped to this edit modal.
    OpenFootprintPicker,
    /// Submit the modal — calls `adapter.update_row` and refreshes the
    /// browser cache. On success the modal closes; on failure the
    /// error surfaces inline.
    Save,
    /// Dismiss the modal without saving.
    Cancel,
}

/// Primitive picker modal sub-messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PrimitivePickerMsg {
    /// Live update of the filter text input.
    SetFilter(String),
    /// Commit a picked `PrimitiveRef` — applies to the picker's
    /// configured target.
    Pick(PrimitiveRef),
    /// User clicked "Browse filesystem…" — fires `AsyncFileDialog`.
    Browse,
    /// Result of the filesystem browse. `Some(path)` when the user
    /// picked a `.snxsym` / `.snxfpt` file; `None` when cancelled.
    BrowseResult(Option<PathBuf>),
    /// Dismiss the picker without picking.
    Cancel,
}

/// Settings → Library → Distributor APIs panel messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SettingsMsg {
    DigiKeyConnect,
    DigiKeyCancel,
    DigiKeyOAuthResult {
        /// Generation tag stamped at the time `DigiKeyConnect` spawned
        /// the worker. The handler ignores results whose generation
        /// no longer matches `digikey_flow_generation` — the user has
        /// since cancelled and started a fresh flow, and applying the
        /// stale outcome would clobber the new flow's state.
        generation: u64,
        connected_label: Option<String>,
        error: Option<String>,
    },
    MouserApiKeyChanged(String),
    MouserTest,
    MouserTestResult(Result<(), String>),
    PreferenceUp(DistributorSource),
    PreferenceDown(DistributorSource),
}
