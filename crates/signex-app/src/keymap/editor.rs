use crate::keymap::{
    AppCommandId, BindingConflict, CommandGroup, CompiledKeymap, ProfileLoadError,
    ShortcutBindingAction, ShortcutContext, ShortcutProfile, ShortcutProfileKind,
    ShortcutProfileSet, ShortcutTrigger, fallback_label, metadata_for,
};
use std::collections::{BTreeMap, BTreeSet};

/// Identity of one trigger row: its position in the active profile's
/// bindings, and in that binding's trigger list (#1378).
///
/// Several rows can share a command and context (`place_wire` is bound to
/// both `P W` and `Ctrl+W`), so neither names the row on its own. Edits
/// replace a trigger in place and never reorder, so a slot stays valid for
/// as long as the active profile does; drafts are dropped on any profile
/// switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingSlot {
    pub binding: usize,
    pub trigger: usize,
}

type TriggerEditKey = BindingSlot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapEditorModel {
    profiles: ShortcutProfileSet,
    trigger_drafts: BTreeMap<TriggerEditKey, String>,
    invalid_trigger_drafts: BTreeSet<TriggerEditKey>,
}

impl KeymapEditorModel {
    pub fn new(profiles: ShortcutProfileSet) -> Self {
        Self {
            profiles,
            trigger_drafts: BTreeMap::new(),
            invalid_trigger_drafts: BTreeSet::new(),
        }
    }

    pub fn built_ins() -> Result<Self, ProfileLoadError> {
        Ok(Self::new(ShortcutProfileSet::built_ins()?))
    }

    pub fn profiles(&self) -> Vec<KeymapEditorProfile> {
        self.profiles
            .profiles()
            .map(|profile| KeymapEditorProfile {
                id: profile.id.clone(),
                name: profile.name.clone(),
                kind: profile.kind,
                binding_count: profile.bindings.len(),
                active: profile.id == self.profiles.active_profile().id,
            })
            .collect()
    }

    pub fn rows(&self) -> Vec<KeymapEditorRow> {
        let active = self.profiles.active_profile();
        let source = match active.kind {
            ShortcutProfileKind::BuiltIn => KeymapEditorSource::BuiltIn(active.id.clone()),
            ShortcutProfileKind::Custom => KeymapEditorSource::Custom(active.id.clone()),
        };

        active
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| !matches!(binding.action, ShortcutBindingAction::Unbind(_)))
            .flat_map(|(binding_index, binding)| {
                let source = source.clone();
                binding
                    .triggers
                    .iter()
                    .enumerate()
                    .map(move |(trigger_index, trigger)| {
                        let command = binding.action.command().cloned();
                        let metadata = command.as_ref().and_then(metadata_for);
                        let slot = BindingSlot {
                            binding: binding_index,
                            trigger: trigger_index,
                        };
                        let trigger_text = self
                            .trigger_drafts
                            .get(&slot)
                            .cloned()
                            .unwrap_or_else(|| trigger.display_text());
                        let trigger_valid = !self.invalid_trigger_drafts.contains(&slot);
                        KeymapEditorRow {
                            slot,
                            command,
                            group: metadata
                                .map(|metadata| metadata.group)
                                .unwrap_or(CommandGroup::General),
                            category: metadata
                                .map(|metadata| metadata.category.to_string())
                                .unwrap_or_else(|| "uncategorized".to_string()),
                            label: metadata
                                .map(|metadata| metadata.label.to_string())
                                .or_else(|| binding.action.command().map(fallback_label))
                                .unwrap_or_else(|| "No action".to_string()),
                            context: binding.context,
                            trigger: trigger_text,
                            trigger_valid,
                            keyboard_editable: matches!(trigger, ShortcutTrigger::KeySequence(_)),
                            source: source.clone(),
                        }
                    })
            })
            .collect()
    }

    /// [`Self::rows`] filtered by a case-insensitive search query against
    /// each row's label, command id or trigger text. An empty query
    /// returns every row.
    pub fn filtered_rows(&self, query: &str) -> Vec<KeymapEditorRow> {
        self.rows()
            .into_iter()
            .filter(|row| row.matches_query(query))
            .collect()
    }

    pub fn create_custom_from_active(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<(), ProfileLoadError> {
        let profile = self.profiles.active_profile().copy_as_custom(id, name)?;
        let id = profile.id.clone();
        self.profiles.insert_custom_profile(profile)?;
        self.profiles.set_active_profile(id)?;
        self.clear_trigger_drafts();
        Ok(())
    }

    pub fn delete_custom_profile(&mut self, id: &str) -> Result<(), ProfileLoadError> {
        self.profiles.delete_custom_profile(id)?;
        self.clear_trigger_drafts();
        Ok(())
    }

    pub fn set_active_profile(&mut self, id: impl Into<String>) -> Result<(), ProfileLoadError> {
        self.profiles.set_active_profile(id)?;
        self.clear_trigger_drafts();
        Ok(())
    }

    pub fn insert_custom_profile(
        &mut self,
        profile: ShortcutProfile,
    ) -> Result<(), ProfileLoadError> {
        self.profiles.insert_custom_profile(profile)?;
        self.clear_trigger_drafts();
        Ok(())
    }

    pub fn active_profile(&self) -> &ShortcutProfile {
        self.profiles.active_profile()
    }

    pub fn profile_set(&self) -> &ShortcutProfileSet {
        &self.profiles
    }

    pub fn active_keymap(&self) -> CompiledKeymap {
        self.profiles.compile_active()
    }

    pub fn active_profile_is_custom(&self) -> bool {
        self.profiles.active_profile().kind == ShortcutProfileKind::Custom
    }

    pub fn edit_active_trigger(
        &mut self,
        slot: BindingSlot,
        trigger_text: String,
    ) -> Result<(), ProfileLoadError> {
        let key = slot;
        self.trigger_drafts.insert(key, trigger_text.clone());

        if !self.active_profile_is_custom() {
            self.invalid_trigger_drafts.insert(key);
            return Err(ProfileLoadError::BuiltInMutation(
                self.profiles.active_profile().id.clone(),
            ));
        }

        let trigger = match ShortcutTrigger::parse(&trigger_text) {
            Ok(trigger) => trigger,
            Err(error) => {
                self.invalid_trigger_drafts.insert(key);
                return Err(ProfileLoadError::KeyParse(error));
            }
        };

        self.apply_active_trigger(slot, trigger)?;
        self.invalid_trigger_drafts.remove(&key);
        Ok(())
    }

    pub fn has_invalid_trigger_drafts(&self) -> bool {
        !self.invalid_trigger_drafts.is_empty()
    }

    /// True when this working copy has diverged from the live profile set —
    /// feeds the Preferences dirty comparator. Valid trigger edits are
    /// applied straight into `profiles` (so the set comparison sees them,
    /// including an edit retyped back to the original, which compares
    /// clean); an *invalid* draft counts as dirty on its own because it is
    /// pending user input that Save refuses to commit.
    pub fn differs_from(&self, live: &ShortcutProfileSet) -> bool {
        self.profiles != *live || !self.invalid_trigger_drafts.is_empty()
    }

    pub fn active_conflicts(&self) -> Vec<BindingConflict> {
        self.active_keymap().conflicts()
    }

    pub fn into_profiles(self) -> ShortcutProfileSet {
        self.profiles
    }

    fn apply_active_trigger(
        &mut self,
        slot: BindingSlot,
        trigger: ShortcutTrigger,
    ) -> Result<(), ProfileLoadError> {
        let profile = self.profiles.active_profile_mut();
        let target = profile
            .bindings
            .get_mut(slot.binding)
            .and_then(|binding| binding.triggers.get_mut(slot.trigger))
            .ok_or(ProfileLoadError::UnknownBindingSlot)?;
        *target = trigger;
        Ok(())
    }

    fn clear_trigger_drafts(&mut self) {
        self.trigger_drafts.clear();
        self.invalid_trigger_drafts.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapEditorProfile {
    pub id: String,
    pub name: String,
    pub kind: ShortcutProfileKind,
    pub binding_count: usize,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapEditorRow {
    /// Which binding trigger this row is, so an edit reaches this row and
    /// no other (#1378).
    pub slot: BindingSlot,
    pub command: Option<AppCommandId>,
    /// Coarse editor-surface bucket used to group rows in the editor.
    /// Rows without command metadata fall back to [`CommandGroup::General`].
    pub group: CommandGroup,
    pub category: String,
    pub label: String,
    pub context: ShortcutContext,
    pub trigger: String,
    pub trigger_valid: bool,
    pub keyboard_editable: bool,
    pub source: KeymapEditorSource,
}

impl KeymapEditorRow {
    /// Case-insensitive substring match on the row's label, command id or
    /// current trigger text. An empty (or whitespace-only) query matches
    /// every row. Never panics — safe on empty / odd input.
    pub fn matches_query(&self, query: &str) -> bool {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        if self.label.to_lowercase().contains(&needle) {
            return true;
        }
        if self.trigger.to_lowercase().contains(&needle) {
            return true;
        }
        self.command
            .as_ref()
            .is_some_and(|command| command.as_str().to_lowercase().contains(&needle))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapEditorSource {
    BuiltIn(String),
    Custom(String),
}

impl From<ShortcutProfile> for KeymapEditorModel {
    fn from(profile: ShortcutProfile) -> Self {
        let active = profile.id.clone();
        let profiles = ShortcutProfileSet::new([profile], active)
            .expect("single-profile editor model uses its profile as active");
        Self::new(profiles)
    }
}
