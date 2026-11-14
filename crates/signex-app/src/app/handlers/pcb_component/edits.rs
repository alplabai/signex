//! The Component view's setting labels (#2365) and the Add parameter edit
//! (#2369), which takes two drafts at once.

use std::path::Path;

use signex_engine::pcb::component_edit::{self, ComponentSetting, TypedSetting};
use signex_i18n::{MessageId, ids, t};
use uuid::Uuid;

use crate::app::Signex;
use crate::panels::ComponentField;

/// The label a setting's edit is named by: its row's label.
pub(super) fn setting_label(setting: &ComponentSetting) -> MessageId {
    match setting {
        ComponentSetting::Type(_) => ids::PCB_COMPONENT_TYPE,
        ComponentSetting::Height(_) => ids::PCB_COMPONENT_HEIGHT,
        ComponentSetting::BodyOpacity(_) => ids::PCB_COMPONENT_BODY_OPACITY,
        ComponentSetting::LockPrimitives(_) => ids::PCB_COMPONENT_LOCK_PRIMITIVES,
        ComponentSetting::LockStrings(_) => ids::PCB_COMPONENT_LOCK_STRINGS,
        ComponentSetting::PinSwapping(_) => ids::PCB_COMPONENT_PIN_SWAP,
        ComponentSetting::PartSwapping(_) => ids::PCB_COMPONENT_PART_SWAP,
        ComponentSetting::ChannelOffset(_) => ids::PCB_COMPONENT_CHANNEL_OFFSET,
        ComponentSetting::Description(_) => ids::PCB_COMPONENT_DESCRIPTION,
    }
}

/// The label of a typed setting's row.
pub(super) fn typed_label(setting: TypedSetting) -> MessageId {
    match setting {
        TypedSetting::Height => ids::PCB_COMPONENT_HEIGHT,
        TypedSetting::BodyOpacity => ids::PCB_COMPONENT_BODY_OPACITY,
        TypedSetting::ChannelOffset => ids::PCB_COMPONENT_CHANNEL_OFFSET,
        TypedSetting::Description => ids::PCB_COMPONENT_DESCRIPTION,
    }
}

impl Signex {
    /// Add the parameter typed into the Parameters tab's Name and Value
    /// fields to `footprints` on `board`, as one step. The two drafts go only
    /// once it is added, so a refused name can be corrected.
    pub(super) fn add_component_parameter(&mut self, board: &Path, footprints: &[Uuid]) {
        let typed = |field: &ComponentField| {
            self.interaction_state
                .pcb_component
                .draft
                .as_ref()
                .filter(|d| d.is_for(board, footprints))
                .and_then(|d| d.texts.get(field).cloned())
                .unwrap_or_default()
        };
        let name = typed(&ComponentField::NewParameterName);
        let value = typed(&ComponentField::NewParameterValue);
        let what = t!(
            self.ui_state.i18n,
            ids::PCB_COMPONENT_PARAMETER_ADDED,
            "key" => name.trim()
        )
        .into_string();
        let added = self.run_component_edit(board, footprints, &what, |b, ids| {
            component_edit::add_parameter_commands(b, ids, &name, &value)
        });
        if !added {
            return;
        }
        let slot = &mut self.interaction_state.pcb_component.draft;
        if let Some(draft) = slot.as_mut().filter(|d| d.is_for(board, footprints)) {
            draft.texts.remove(&ComponentField::NewParameterName);
            draft.texts.remove(&ComponentField::NewParameterValue);
        }
        if slot.as_ref().is_some_and(|d| d.texts.is_empty()) {
            *slot = None;
        }
    }
}
