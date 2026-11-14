//! Shortcut profiles: the profile model and set (here), the compiled
//! lookup table (`compiled`), the TOML file format (`schema`), reading and
//! writing the user's file (`storage`) and the load error (`error`).

mod compiled;
mod error;
mod schema;
mod storage;

pub use compiled::{BindingConflict, CompiledKeymap, KeyLookup};
pub use error::ProfileLoadError;
pub use schema::TomlShortcutProfile;
pub use storage::{
    RestoredProfiles, config_path, config_path_for_dir, export_custom_profile,
    export_custom_profiles, import_custom_profile, load_profile_set, load_profile_set_at,
    read_backup_profiles_at, save_profile_set, save_profile_set_at,
};

use schema::validate_profile_id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::keymap::ShortcutBinding;

const ALTIUM_PROFILE_TOML: &str = include_str!("../../../assets/keyboard-shortcuts/altium.toml");
const CLASSIC_PROFILE_TOML: &str = include_str!("../../../assets/keyboard-shortcuts/classic.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutProfileKind {
    BuiltIn,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutProfile {
    pub id: String,
    pub name: String,
    pub kind: ShortcutProfileKind,
    pub schema_version: u32,
    pub description: Option<String>,
    #[serde(default)]
    pub base_profile: Option<String>,
    pub bindings: Vec<ShortcutBinding>,
}

impl ShortcutProfile {
    pub fn copy_as_custom(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<Self, ProfileLoadError> {
        let id = id.into();
        validate_profile_id(&id)?;
        Ok(Self {
            id,
            name: name.into(),
            kind: ShortcutProfileKind::Custom,
            schema_version: self.schema_version,
            description: Some(format!("Custom profile copied from {}.", self.name)),
            base_profile: Some(self.id.clone()),
            bindings: self.bindings.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltInProfile {
    Altium,
    Classic,
}

impl BuiltInProfile {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Altium => "altium",
            Self::Classic => "classic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutProfileSet {
    profiles: BTreeMap<String, ShortcutProfile>,
    active_profile_id: String,
}

impl ShortcutProfileSet {
    pub fn built_ins() -> Result<Self, ProfileLoadError> {
        let profiles = [
            TomlShortcutProfile::parse(ALTIUM_PROFILE_TOML)?.into_profile()?,
            TomlShortcutProfile::parse(CLASSIC_PROFILE_TOML)?.into_profile()?,
        ];
        Self::new(profiles, BuiltInProfile::Altium.id())
    }

    pub fn new(
        profiles: impl IntoIterator<Item = ShortcutProfile>,
        active_profile_id: impl Into<String>,
    ) -> Result<Self, ProfileLoadError> {
        let mut map = BTreeMap::new();
        for profile in profiles {
            validate_profile_id(&profile.id)?;
            if map.insert(profile.id.clone(), profile).is_some() {
                return Err(ProfileLoadError::DuplicateProfileId);
            }
        }
        let active_profile_id = active_profile_id.into();
        if !map.contains_key(&active_profile_id) {
            return Err(ProfileLoadError::UnknownActiveProfile(active_profile_id));
        }
        Ok(Self {
            profiles: map,
            active_profile_id,
        })
    }

    pub fn active_profile(&self) -> &ShortcutProfile {
        self.profiles
            .get(&self.active_profile_id)
            .expect("active profile is validated when profile set is constructed")
    }

    pub fn active_profile_mut(&mut self) -> &mut ShortcutProfile {
        self.profiles
            .get_mut(&self.active_profile_id)
            .expect("active profile is validated when profile set is constructed")
    }

    pub fn active_profile_id(&self) -> &str {
        &self.active_profile_id
    }

    pub fn profiles(&self) -> impl Iterator<Item = &ShortcutProfile> {
        self.profiles.values()
    }

    pub fn profile(&self, id: &str) -> Option<&ShortcutProfile> {
        self.profiles.get(id)
    }

    pub fn set_active_profile(&mut self, id: impl Into<String>) -> Result<(), ProfileLoadError> {
        let id = id.into();
        if !self.profiles.contains_key(&id) {
            return Err(ProfileLoadError::UnknownActiveProfile(id));
        }
        self.active_profile_id = id;
        Ok(())
    }

    pub fn insert_custom_profile(
        &mut self,
        profile: ShortcutProfile,
    ) -> Result<(), ProfileLoadError> {
        if profile.kind != ShortcutProfileKind::Custom {
            return Err(ProfileLoadError::BuiltInMutation(profile.id));
        }
        validate_profile_id(&profile.id)?;
        if self
            .profiles
            .get(&profile.id)
            .is_some_and(|existing| existing.kind == ShortcutProfileKind::BuiltIn)
        {
            return Err(ProfileLoadError::BuiltInMutation(profile.id));
        }
        self.profiles.insert(profile.id.clone(), profile);
        Ok(())
    }

    pub fn delete_custom_profile(&mut self, id: &str) -> Result<(), ProfileLoadError> {
        let profile = self
            .profiles
            .get(id)
            .ok_or_else(|| ProfileLoadError::UnknownActiveProfile(id.to_string()))?;
        if profile.kind != ShortcutProfileKind::Custom {
            return Err(ProfileLoadError::BuiltInMutation(id.to_string()));
        }
        self.profiles.remove(id);
        if self.active_profile_id == id {
            self.active_profile_id = BuiltInProfile::Altium.id().to_string();
        }
        Ok(())
    }

    pub fn compile_active(&self) -> CompiledKeymap {
        CompiledKeymap::compile(self.active_profile())
    }

    /// [`Self::compile_active`] with the labels in the language of `i18n`:
    /// what the app keeps as its live keymap.
    pub fn compile_active_in(&self, i18n: &signex_i18n::I18n) -> CompiledKeymap {
        CompiledKeymap::compile_in(self.active_profile(), i18n)
    }
}
