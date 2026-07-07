//! Simulation-model edits for a Component Preview row.
//!
//! Enabling simulation mints a fresh SPICE `SimModel` and binds it to the
//! row; the remaining actions edit the model's kind, name, body text, and
//! per-pin node map while it exists.

use crate::library::ComponentPreviewState;

/// Enable or disable the row's simulation model.
///
/// Enabling mints a fresh SPICE3 `SimModel` (and an editor buffer) when
/// the row has none; disabling clears the model, its binding, and the
/// buffer.
pub(super) fn set_enabled(state: &mut ComponentPreviewState, enabled: bool) {
    if enabled {
        if state.row.sim_ref.is_none() {
            let sim = signex_library::SimModel {
                uuid: uuid::Uuid::now_v7(),
                name: state.row.internal_pn.as_str().to_string(),
                kind: signex_library::SimKind::Spice3,
                body: String::new(),
                default_node_map: std::collections::BTreeMap::new(),
                // Stage 14: every primitive carries its own semver string
                // + released flag. Defaults match the serde defaults so
                // reads of pre-Stage-14 `.snxsim` files work.
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

/// Set the simulation model's kind, touching its `updated` timestamp.
pub(super) fn set_kind(state: &mut ComponentPreviewState, kind: signex_library::SimKind) {
    if let Some(sim) = state.sim.as_mut() {
        sim.kind = kind;
        sim.updated = chrono::Utc::now();
        state.dirty = true;
    }
}

/// Set the simulation model's name, touching its `updated` timestamp.
pub(super) fn set_name(state: &mut ComponentPreviewState, name: String) {
    if let Some(sim) = state.sim.as_mut() {
        sim.name = name;
        sim.updated = chrono::Utc::now();
        state.dirty = true;
    }
}

/// Apply a text-editor action to the simulation body, mirroring the new
/// text back onto the model.
pub(super) fn apply_body_action(
    state: &mut ComponentPreviewState,
    action: iced::widget::text_editor::Action,
) {
    if let Some(content) = state.sim_body.as_mut() {
        content.perform(action);
        if let Some(sim) = state.sim.as_mut() {
            sim.body = content.text();
            sim.updated = chrono::Utc::now();
        }
        state.dirty = true;
    }
}

/// Set (or clear, when blank) the default node mapping for one pin.
pub(super) fn set_pin_node(
    state: &mut ComponentPreviewState,
    pin_number: String,
    value: String,
) {
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
