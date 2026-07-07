//! Update logic for the Component Preview inline-edit surface.
//!
//! [`apply_inline_edit`] is a thin routing table: each [`EditorMsg`] that
//! mutates the previewed component row is dispatched to the concern
//! module that owns the affected field group — [`datasheet`], [`pin_map`],
//! [`supply`], [`parameters`], or [`sim`]. Trivial component-level
//! setters are handled inline.
//!
//! The long tail of variants belonging to the standalone symbol/footprint
//! canvases is matched explicitly (rather than with a `_` wildcard) so a
//! newly added [`EditorMsg`] variant is a compile error here until it is
//! deliberately routed or ignored.

mod datasheet;
mod parameters;
mod pin_map;
mod sim;
mod supply;

use crate::library::ComponentPreviewState;
use crate::library::messages::EditorMsg;

/// Apply an inline-edit message to a Component Preview state.
///
/// Tab switching, save, and async-bounce variants are handled by the
/// dispatcher before reaching here; this is the reducer for in-place row
/// mutations across the datasheet, pin-map, supply, parameters, and
/// simulation field groups.
pub(crate) fn apply_inline_edit(state: &mut ComponentPreviewState, msg: EditorMsg) {
    match msg {
        // ── Component-level ──────────────────────────────────────────
        EditorMsg::SelectTab(tab) => state.active_tab = tab,
        EditorMsg::SetLifecycle(lifecycle) => {
            state.row.state = lifecycle;
            state.dirty = true;
        }

        // ── Datasheet ────────────────────────────────────────────────
        EditorMsg::DatasheetSetMode(mode) => datasheet::set_mode(state, mode),
        EditorMsg::DatasheetSetUrl(url) => datasheet::set_url(state, url),
        EditorMsg::DatasheetUploadResult(payload) => {
            datasheet::apply_upload_result(state, payload)
        }

        // ── Pin map ──────────────────────────────────────────────────
        EditorMsg::PinMapAutoMatchByNumber | EditorMsg::PinMapClearOverrides => {
            pin_map::clear_overrides(state)
        }
        EditorMsg::PinMapAutoMatchByName => pin_map::warn_auto_match_by_name(),
        EditorMsg::PinMapOpenOverrideEdit(pin) => pin_map::open_override_edit(state, pin),
        EditorMsg::PinMapOverrideBufChanged { pin, value } => {
            pin_map::set_override_buf(state, pin, value)
        }
        EditorMsg::PinMapAddOverride { pin, pad } => pin_map::add_override(state, pin, pad),
        EditorMsg::PinMapCancelOverrideEdit => pin_map::cancel_override_edit(state),
        EditorMsg::PinMapRemoveOverride { pin } => pin_map::remove_override(state, pin),

        // ── Supply: primary part ─────────────────────────────────────
        EditorMsg::SupplyPrimarySetManufacturer(value) => {
            supply::set_primary_manufacturer(state, value)
        }
        EditorMsg::SupplyPrimarySetMpn(value) => supply::set_primary_mpn(state, value),
        EditorMsg::SupplyPrimarySetStatus(value) => supply::set_primary_status(state, value),
        EditorMsg::SupplyPrimarySetNotes(value) => supply::set_primary_notes(state, value),

        // ── Supply: alternates ───────────────────────────────────────
        EditorMsg::SupplyAlternateAdd => supply::add_alternate(state),
        EditorMsg::SupplyAlternateSetManufacturer { idx, value } => {
            supply::set_alternate_manufacturer(state, idx, value)
        }
        EditorMsg::SupplyAlternateSetMpn { idx, value } => {
            supply::set_alternate_mpn(state, idx, value)
        }
        EditorMsg::SupplyAlternateSetStatus { idx, value } => {
            supply::set_alternate_status(state, idx, value)
        }
        EditorMsg::SupplyAlternateSetNotes { idx, value } => {
            supply::set_alternate_notes(state, idx, value)
        }
        EditorMsg::SupplyAlternateRemove { idx } => supply::remove_alternate(state, idx),

        // ── Supply: distributor listings ─────────────────────────────
        EditorMsg::SupplyListingAdd => supply::add_listing(state),
        EditorMsg::SupplyListingSetDistributor { idx, value } => {
            supply::set_listing_distributor(state, idx, value)
        }
        EditorMsg::SupplyListingSetSku { idx, value } => {
            supply::set_listing_sku(state, idx, value)
        }
        EditorMsg::SupplyListingSetUrl { idx, value } => {
            supply::set_listing_url(state, idx, value)
        }
        EditorMsg::SupplyListingRemove { idx } => supply::remove_listing(state, idx),

        // ── Parameters ───────────────────────────────────────────────
        EditorMsg::ParamSetText { name, value } => parameters::set_text(state, name, value),
        EditorMsg::ParamSetNumberBuf { name, buf } => {
            parameters::set_number_buf(state, name, buf)
        }
        EditorMsg::ParamCommitNumber { name } => parameters::commit_number(state, name),
        EditorMsg::ParamSetMeasurementBuf { name, buf } => {
            parameters::set_measurement_buf(state, name, buf)
        }
        EditorMsg::ParamCommitMeasurement { name, unit } => {
            parameters::commit_measurement(state, name, unit)
        }
        EditorMsg::ParamSetBool { name, value } => parameters::set_bool(state, name, value),
        EditorMsg::ParamRemove { name } => parameters::remove(state, name),
        EditorMsg::ParamAddCustom { name, kind } => parameters::add_custom(state, name, kind),

        // ── Simulation ───────────────────────────────────────────────
        EditorMsg::SimSetEnabled(enabled) => sim::set_enabled(state, enabled),
        EditorMsg::SimSetKind(kind) => sim::set_kind(state, kind),
        EditorMsg::SimSetName(name) => sim::set_name(state, name),
        EditorMsg::SimBodyAction(action) => sim::apply_body_action(state, action),
        EditorMsg::SimSetPinNode { pin_number, value } => {
            sim::set_pin_node(state, pin_number, value)
        }

        // ── Owned by other surfaces ──────────────────────────────────
        // These variants drive the standalone `.snxsym` / `.snxfpt`
        // document tabs and the async save/review plumbing; they never
        // reach the Component Preview inline surface but stay enumerated
        // so this match is exhaustive over `EditorMsg`.
        EditorMsg::CloseEditor
        | EditorMsg::SaveDraft
        | EditorMsg::Commit
        | EditorMsg::SubmitForReview
        | EditorMsg::SubmitForReviewNotesChanged(_)
        | EditorMsg::SubmitForReviewCancel
        | EditorMsg::SubmitForReviewConfirm
        | EditorMsg::SubmitForReviewResult(_)
        | EditorMsg::OpenWhereUsedTab
        | EditorMsg::DatasheetUploadDialog
        | EditorMsg::SymbolPickAiPdf
        | EditorMsg::SymbolPickedAiPdf(_)
        | EditorMsg::SymbolSetTool(_)
        | EditorMsg::SymbolAddPin { .. }
        | EditorMsg::SymbolSelect(_)
        | EditorMsg::SymbolDeselect
        | EditorMsg::SymbolMoveSelected { .. }
        | EditorMsg::SymbolMoveAll { .. }
        | EditorMsg::SymbolDeleteSelected
        | EditorMsg::SymbolSetField { .. }
        | EditorMsg::SymbolSetPinNumber { .. }
        | EditorMsg::SymbolSetPinName { .. }
        | EditorMsg::SymbolApplyAiPreview
        | EditorMsg::SymbolDismissAiPreview
        | EditorMsg::SaveSymbol(_, _)
        | EditorMsg::FootprintAddPad { .. }
        | EditorMsg::FootprintAddHole { .. }
        | EditorMsg::FootprintAddText { .. }
        | EditorMsg::FootprintAddTextFrame { .. }
        | EditorMsg::FootprintTrackClick { .. }
        | EditorMsg::FootprintTrackCancel
        | EditorMsg::FootprintArcClick { .. }
        | EditorMsg::FootprintArcCancel
        | EditorMsg::FootprintPolygonClick { .. }
        | EditorMsg::FootprintPolygonCommit
        | EditorMsg::FootprintPolygonCancel
        | EditorMsg::FootprintSelectSilkF(_)
        | EditorMsg::FootprintDeleteSilkF
        | EditorMsg::FootprintSketchPlacePoint { .. }
        | EditorMsg::FootprintSketchToolClick { .. }
        | EditorMsg::FootprintSketchToolEscape
        | EditorMsg::FootprintSketchPlacementInputChar(_)
        | EditorMsg::FootprintSketchPlacementInputBackspace
        | EditorMsg::FootprintSketchPlacementInputEnter
        | EditorMsg::FootprintSketchPlacementInputEscape
        | EditorMsg::FootprintSketchPlacementInputTab
        | EditorMsg::FootprintSketchSelect { .. }
        | EditorMsg::FootprintSketchMovePoint { .. }
        | EditorMsg::FootprintSketchMoveLine { .. }
        | EditorMsg::FootprintSketchResizeRoundPad { .. }
        | EditorMsg::FootprintSetSelectionMode2d(_)
        | EditorMsg::FootprintSelectAllOnLayer
        | EditorMsg::FootprintAddVia { .. }
        | EditorMsg::FootprintSelectOffGridPads
        | EditorMsg::FootprintRecomputeCourtyardOutline
        | EditorMsg::FootprintLassoArm
        | EditorMsg::FootprintLassoAddVertex { .. }
        | EditorMsg::FootprintLassoCommit
        | EditorMsg::FootprintLassoCancel
        | EditorMsg::FootprintTouchingLineArm
        | EditorMsg::FootprintTouchingLineFirst { .. }
        | EditorMsg::FootprintTouchingLineCommit { .. }
        | EditorMsg::FootprintTouchingLineCancel
        | EditorMsg::FootprintSelectOverlapped
        | EditorMsg::FootprintSelectNextOverlapped
        | EditorMsg::FootprintMovePad { .. }
        | EditorMsg::FootprintCursorAt { .. }
        | EditorMsg::FootprintSelectPad(_)
        | EditorMsg::FootprintSelectPads(_)
        | EditorMsg::FootprintSketchSelectMany(_)
        | EditorMsg::FootprintDeleteSelected
        | EditorMsg::FootprintToggleLayer(_)
        | EditorMsg::FootprintToggleAutoFit
        | EditorMsg::FootprintSetPadsTool(_)
        | EditorMsg::FootprintSketchSetTool(_)
        | EditorMsg::FootprintSketchToggleConstruction
        | EditorMsg::FootprintSketchToggleCenterline
        | EditorMsg::FootprintTogglePlacementPause
        | EditorMsg::FootprintShowContextMenu { .. }
        | EditorMsg::FootprintCloseContextMenu
        | EditorMsg::FootprintContextMenuOpenSubmenu(_)
        | EditorMsg::FootprintContextMenuAction(_)
        | EditorMsg::FootprintFitConsumed
        | EditorMsg::FootprintCopyPad
        | EditorMsg::FootprintCutPad
        | EditorMsg::FootprintPastePad
        | EditorMsg::FootprintActiveBarRotateSelection
        | EditorMsg::FootprintActiveBarFlipSelection
        | EditorMsg::FootprintSketchSetRole { .. }
        | EditorMsg::SaveFootprint(_, _)
        | EditorMsg::SetBodyHeight(_)
        | EditorMsg::SetBodyOffsetZ(_)
        | EditorMsg::SetBodyTopColor(_)
        | EditorMsg::SetBodySideColor(_)
        | EditorMsg::SetBodyShape(_)
        | EditorMsg::StepAttachDialog
        | EditorMsg::StepAttachResult(_)
        | EditorMsg::StepAttachRemove
        | EditorMsg::SaveSim(_, _) => {}
    }
}
