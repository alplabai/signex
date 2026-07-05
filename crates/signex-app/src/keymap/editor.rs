use crate::keymap::{
    AppCommandId, BindingConflict, CompiledKeymap, ProfileLoadError, ShortcutContext,
    ShortcutProfile, ShortcutProfileKind, ShortcutProfileSet, ShortcutTrigger, fallback_label,
    metadata_for,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapEditorModel {
    profiles: ShortcutProfileSet,
}

impl KeymapEditorModel {
    pub fn new(profiles: ShortcutProfileSet) -> Self {
        Self { profiles }
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
                    KeymapEditorRow {
                        command,
                        category: metadata
                            .map(|metadata| metadata.category.to_string())
                            .unwrap_or_else(|| "uncategorized".to_string()),
                        label: metadata
                            .map(|metadata| metadata.label.to_string())
                            .or_else(|| binding.action.command().map(fallback_label))
                            .unwrap_or_else(|| "No action".to_string()),
                        context: binding.context,
                        trigger: trigger.display_text(),
                        keyboard_editable: matches!(trigger, ShortcutTrigger::KeySequence(_)),
                        source: source.clone(),
                    }
                })
            })
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
        self.profiles.set_active_profile(id)
    }

    pub fn delete_custom_profile(&mut self, id: &str) -> Result<(), ProfileLoadError> {
        self.profiles.delete_custom_profile(id)
    }

    pub fn set_active_profile(&mut self, id: impl Into<String>) -> Result<(), ProfileLoadError> {
        self.profiles.set_active_profile(id)
    }

    pub fn insert_custom_profile(
        &mut self,
        profile: ShortcutProfile,
    ) -> Result<(), ProfileLoadError> {
        self.profiles.insert_custom_profile(profile)
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

    pub fn active_conflicts(&self) -> Vec<BindingConflict> {
        self.active_keymap().conflicts()
    }

    pub fn into_profiles(self) -> ShortcutProfileSet {
        self.profiles
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
    pub category: String,
    pub label: String,
    pub context: ShortcutContext,
    pub trigger: String,
    pub keyboard_editable: bool,
    pub source: KeymapEditorSource,
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
