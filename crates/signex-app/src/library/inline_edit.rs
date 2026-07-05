//! Component-Preview inline-edit reducer, extracted from the library
//! dispatcher (issue #98 — decomposing the 10k-line
//! `app/dispatch/library.rs`). Pure state reduction over a
//! `ComponentPreviewState`; moved verbatim with no behavioural change.

use crate::library::messages::{EditorMsg, ParamKindMsg};
use crate::library::state::ComponentPreviewState;

/// Apply inline-edit messages directly to a Component Preview state.
/// Tab switching, save, and async-bounce variants are handled before
/// reaching here — this is the catch-all for in-place row mutations
/// (parameters / supply / datasheet / pin-map / simulation).
pub(crate) fn apply_inline_edit(state: &mut ComponentPreviewState, msg: EditorMsg) {
    match msg {
        EditorMsg::SelectTab(tab) => state.active_tab = tab,
        // Component-level setters
        EditorMsg::SetLifecycle(s) => {
            state.row.state = s;
            state.dirty = true;
        }
        // Datasheet
        EditorMsg::DatasheetSetMode(mode) => {
            use crate::library::editor::datasheet_picker::DatasheetMode;
            match mode {
                DatasheetMode::Url => match &state.row.datasheet {
                    signex_library::DatasheetRef::Url { .. } => {}
                    _ => {
                        state.row.datasheet = signex_library::DatasheetRef::default();
                        state.dirty = true;
                    }
                },
                DatasheetMode::PinnedPdf => match &state.row.datasheet {
                    signex_library::DatasheetRef::HashPinned { .. } => {}
                    _ => {
                        state.row.datasheet = signex_library::DatasheetRef::HashPinned {
                            hash: String::new(),
                            filename: String::new(),
                        };
                        state.dirty = true;
                    }
                },
            }
        }
        EditorMsg::DatasheetSetUrl(s) => {
            let trimmed = s.trim();
            state.row.datasheet = if trimmed.is_empty() {
                signex_library::DatasheetRef::default()
            } else {
                signex_library::DatasheetRef::url(trimmed)
            };
            state.dirty = true;
        }
        EditorMsg::DatasheetUploadResult(payload) => {
            if let Some((bytes, filename)) = payload {
                use sha2::Digest;
                let mut hasher = sha2::Sha256::new();
                hasher.update(&bytes);
                let hash = format!("{:x}", hasher.finalize());
                state.row.datasheet = signex_library::DatasheetRef::hash_pinned(hash, filename);
                state.dirty = true;
            }
        }
        // Pin Map
        EditorMsg::PinMapAutoMatchByNumber | EditorMsg::PinMapClearOverrides => {
            state.row.pin_map_overrides.clear();
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        EditorMsg::PinMapAutoMatchByName => {
            tracing::warn!(
                target: "signex::library",
                "Pin Map: Auto-Match by Name is stubbed; awaiting heuristic implementation"
            );
        }
        EditorMsg::PinMapOpenOverrideEdit(pin) => {
            let seed = state
                .row
                .pin_map_overrides
                .iter()
                .find(|o| o.symbol_pin_number == pin)
                .map(|o| o.footprint_pad_number.clone())
                .unwrap_or_default();
            state.pin_map_state.expanded_row = Some(pin);
            state.pin_map_state.override_buf = seed;
        }
        EditorMsg::PinMapOverrideBufChanged { pin, value } => {
            if state.pin_map_state.expanded_row.as_deref() == Some(pin.as_str()) {
                state.pin_map_state.override_buf = value;
            }
        }
        EditorMsg::PinMapAddOverride { pin, pad } => {
            let trimmed = pad.trim();
            if trimmed.is_empty() {
                state
                    .row
                    .pin_map_overrides
                    .retain(|o| o.symbol_pin_number != pin);
            } else if let Some(existing) = state
                .row
                .pin_map_overrides
                .iter_mut()
                .find(|o| o.symbol_pin_number == pin)
            {
                existing.footprint_pad_number = trimmed.to_string();
            } else {
                state
                    .row
                    .pin_map_overrides
                    .push(signex_library::PinPadOverride::new(pin, trimmed));
            }
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        EditorMsg::PinMapCancelOverrideEdit => {
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
        }
        EditorMsg::PinMapRemoveOverride { pin } => {
            state
                .row
                .pin_map_overrides
                .retain(|o| o.symbol_pin_number != pin);
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        // Supply — primary
        EditorMsg::SupplyPrimarySetManufacturer(s) => {
            state.row.primary_mpn.manufacturer = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetMpn(s) => {
            state.row.primary_mpn.mpn = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetStatus(s) => {
            state.row.primary_mpn.status = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetNotes(s) => {
            state.row.primary_mpn.notes = if s.trim().is_empty() { None } else { Some(s) };
            state.dirty = true;
        }
        // Supply — alternates
        EditorMsg::SupplyAlternateAdd => {
            let mut alt = signex_library::ManufacturerPart::draft("", "");
            alt.status = signex_library::AlternateStatus::Approved;
            state.row.alternates.push(alt);
            state.dirty = true;
        }
        EditorMsg::SupplyAlternateSetManufacturer { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.manufacturer = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetMpn { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.mpn = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetStatus { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.status = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetNotes { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.notes = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateRemove { idx } => {
            if idx < state.row.alternates.len() {
                state.row.alternates.remove(idx);
                state.dirty = true;
            }
        }
        // Supply — listings
        EditorMsg::SupplyListingAdd => {
            state.row.supply.push(signex_library::DistributorListing {
                distributor: String::new(),
                sku: String::new(),
                url: None,
                moq: None,
            });
            state.dirty = true;
        }
        EditorMsg::SupplyListingSetDistributor { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.distributor =
                    crate::library::editor::supply::distributor_source_to_string(value);
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetSku { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.sku = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetUrl { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.url = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingRemove { idx } => {
            if idx < state.row.supply.len() {
                state.row.supply.remove(idx);
                state.dirty = true;
            }
        }
        // Parameters
        EditorMsg::ParamSetText { name, value } => {
            if !name.is_empty() {
                state
                    .row
                    .parameters
                    .insert(name.clone(), signex_library::ParamValue::Text(value));
                state.dirty = true;
            }
        }
        EditorMsg::ParamSetNumberBuf { name, buf } => {
            state.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitNumber { name } => {
            if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
                let trimmed = buf.trim();
                if let Ok(v) = trimmed.parse::<f64>() {
                    state
                        .row
                        .parameters
                        .insert(name, signex_library::ParamValue::Number(v));
                    state.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetMeasurementBuf { name, buf } => {
            state.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitMeasurement { name, unit } => {
            if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
                let trimmed = buf.trim();
                if let Ok(v) = trimmed.parse::<f64>() {
                    state.row.parameters.insert(
                        name,
                        signex_library::ParamValue::Measurement { value: v, unit },
                    );
                    state.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetBool { name, value } => {
            state
                .row
                .parameters
                .insert(name, signex_library::ParamValue::Bool(value));
            state.dirty = true;
        }
        EditorMsg::ParamRemove { name } => {
            state.row.parameters.remove(&name);
            state.dirty = true;
        }
        EditorMsg::ParamAddCustom { name, kind } => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return;
            }
            let value = match kind {
                ParamKindMsg::Text => signex_library::ParamValue::Text(String::new()),
                ParamKindMsg::Number => signex_library::ParamValue::Number(0.0),
                ParamKindMsg::Bool => signex_library::ParamValue::Bool(false),
                ParamKindMsg::Measurement(unit) => {
                    signex_library::ParamValue::Measurement { value: 0.0, unit }
                }
            };
            state.row.parameters.insert(trimmed.to_string(), value);
            state.dirty = true;
        }
        // Sim
        EditorMsg::SimSetEnabled(enabled) => {
            if enabled {
                if state.row.sim_ref.is_none() {
                    let sim = signex_library::SimModel {
                        uuid: uuid::Uuid::now_v7(),
                        name: state.row.internal_pn.as_str().to_string(),
                        kind: signex_library::SimKind::Spice3,
                        body: String::new(),
                        default_node_map: std::collections::BTreeMap::new(),
                        // Stage 14: every primitive carries its own
                        // semver string + released flag. Defaults match
                        // the serde defaults so reads of pre-Stage-14
                        // `.snxsim` files work.
                        version: "0.0.1".into(),
                        released: false,
                        created: chrono::Utc::now(),
                        updated: chrono::Utc::now(),
                    };
                    state.row.sim_ref = Some(signex_library::PrimitiveRef::new(
                        state.row.symbol_ref.library_id,
                        sim.uuid,
                    ));
                    state.sim_body = Some(iced::widget::text_editor::Content::new());
                    state.sim = Some(sim);
                }
            } else {
                state.row.sim_ref = None;
                state.sim = None;
                state.sim_body = None;
            }
            state.dirty = true;
        }
        EditorMsg::SimSetKind(kind) => {
            if let Some(sim) = state.sim.as_mut() {
                sim.kind = kind;
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        EditorMsg::SimSetName(name) => {
            if let Some(sim) = state.sim.as_mut() {
                sim.name = name;
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        EditorMsg::SimBodyAction(action) => {
            if let Some(content) = state.sim_body.as_mut() {
                content.perform(action);
                if let Some(sim) = state.sim.as_mut() {
                    sim.body = content.text();
                    sim.updated = chrono::Utc::now();
                }
                state.dirty = true;
            }
        }
        EditorMsg::SimSetPinNode { pin_number, value } => {
            if let Some(sim) = state.sim.as_mut() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    sim.default_node_map.remove(&pin_number);
                } else {
                    sim.default_node_map.insert(pin_number, trimmed.to_string());
                }
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        // Variants below are kept around for the standalone primitive
        // editors (`.snxsym` / `.snxfpt` document tabs); they're
        // never fired through the Component Preview surface but stay
        // defined to keep the message tree backwards-compatible.
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
