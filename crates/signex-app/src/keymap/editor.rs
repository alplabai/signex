use crate::keymap::{
    AppCommandId, BindingConflict, CommandGroup, CompiledKeymap, ProfileLoadError, ShortcutContext,
    ShortcutBinding, ShortcutBindingAction, ShortcutProfile, ShortcutProfileKind,
    ShortcutProfileSet, ShortcutTrigger, fallback_label, metadata_for,
};
use std::collections::{BTreeMap, BTreeSet};

type TriggerEditKey = (AppCommandId, ShortcutContext);

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
            .flat_map(|binding| {
                let source = source.clone();
                binding.triggers.iter().map(move |trigger| {
                    let command = binding.action.command().cloned();
                    let metadata = command.as_ref().and_then(metadata_for);
                    let draft_key = command
                        .as_ref()
                        .map(|command| (command.clone(), binding.context));
                    let trigger_text = draft_key
                        .as_ref()
                        .and_then(|key| self.trigger_drafts.get(key))
                        .cloned()
                        .unwrap_or_else(|| trigger.display_text());
                    let trigger_valid = draft_key
                        .as_ref()
                        .is_none_or(|key| !self.invalid_trigger_drafts.contains(key));
                    KeymapEditorRow {
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
        command: AppCommandId,
        context: ShortcutContext,
        trigger_text: String,
    ) -> Result<(), ProfileLoadError> {
        let key = (command.clone(), context);
        self.trigger_drafts.insert(key.clone(), trigger_text.clone());

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

        self.apply_active_trigger(command, context, trigger);
        self.invalid_trigger_drafts.remove(&key);
        Ok(())
    }

    pub fn has_invalid_trigger_drafts(&self) -> bool {
        !self.invalid_trigger_drafts.is_empty()
    }

    pub fn active_conflicts(&self) -> Vec<BindingConflict> {
        self.active_keymap().conflicts()
    }

    pub fn into_profiles(self) -> ShortcutProfileSet {
        self.profiles
    }

    fn apply_active_trigger(
        &mut self,
        command: AppCommandId,
        context: ShortcutContext,
        trigger: ShortcutTrigger,
    ) {
        let profile = self.profiles.active_profile_mut();
        if let Some(binding) = profile.bindings.iter_mut().rev().find(|binding| {
            binding.context == context && binding.action.command() == Some(&command)
        }) {
            binding.triggers = vec![trigger];
            return;
        }

        profile.bindings.push(ShortcutBinding {
            action: ShortcutBindingAction::Command(command),
            context,
            triggers: vec![trigger],
        });
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
