//! The TOML shape of a shortcut file and its conversion to and from profiles.

use super::{ProfileLoadError, ShortcutProfile, ShortcutProfileKind, ShortcutProfileSet};
use crate::keymap::{
    AppCommandId, Platform, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
    ShortcutTrigger,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct TomlShortcutProfile {
    signex_settings: TomlSignexSettings,
    keyboard_shortcuts: TomlKeyboardShortcuts,
}

impl TomlShortcutProfile {
    pub fn parse(source: &str) -> Result<Self, ProfileLoadError> {
        toml::from_str(source).map_err(ProfileLoadError::Toml)
    }

    pub fn into_profile(self) -> Result<ShortcutProfile, ProfileLoadError> {
        self.signex_settings.validate()?;
        self.keyboard_shortcuts.into_profile()
    }

    pub(super) fn from_profile(profile: &ShortcutProfile) -> Result<Self, ProfileLoadError> {
        Ok(Self {
            signex_settings: TomlSignexSettings::default(),
            keyboard_shortcuts: TomlKeyboardShortcuts::from_profile(profile)?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct TomlShortcutConfig {
    signex_settings: TomlSignexSettings,
    keyboard_shortcuts: TomlKeyboardShortcutsConfig,
}

impl TomlShortcutConfig {
    pub(super) fn parse(source: &str) -> Result<Self, ProfileLoadError> {
        toml::from_str(source).map_err(ProfileLoadError::Toml)
    }

    pub(super) fn from_profile_set(set: &ShortcutProfileSet) -> Result<Self, ProfileLoadError> {
        let profiles = set
            .profiles()
            .filter(|profile| profile.kind == ShortcutProfileKind::Custom)
            .map(TomlKeyboardShortcuts::from_profile)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            signex_settings: TomlSignexSettings::default(),
            keyboard_shortcuts: TomlKeyboardShortcutsConfig {
                active_profile: set.active_profile_id().to_string(),
                profiles,
            },
        })
    }

    pub(super) fn apply_to(self, set: &mut ShortcutProfileSet) -> Result<(), ProfileLoadError> {
        // Delegates so the loading and the recovering path cannot drift:
        // a dangling `active_profile` is a hard error here, and the only
        // difference in `apply_to_recovering` is that it hands the id
        // back instead.
        match self.apply_to_recovering(set)? {
            Some(id) => Err(ProfileLoadError::UnknownActiveProfile(id)),
            None => Ok(()),
        }
    }

    /// Apply the config, treating an unresolvable `active_profile` as
    /// recoverable rather than fatal (#603).
    ///
    /// Order is load-bearing and already was: every custom profile is
    /// inserted before `set_active_profile` runs, so by the time the
    /// active pointer can fail the profiles are all in `set`. That is
    /// what makes recovery possible at all — the one thing that failed
    /// is the pointer, and the built-in default is a fine substitute
    /// for it.
    ///
    /// Returns the id that could not be honoured, or `None` when the
    /// config applied in full.
    pub(super) fn apply_to_recovering(
        self,
        set: &mut ShortcutProfileSet,
    ) -> Result<Option<String>, ProfileLoadError> {
        self.signex_settings.validate()?;
        for profile in self.keyboard_shortcuts.profiles {
            set.insert_custom_profile(profile.into_profile()?)?;
        }
        let wanted = self.keyboard_shortcuts.active_profile;
        match set.set_active_profile(wanted.clone()) {
            Ok(()) => Ok(None),
            Err(ProfileLoadError::UnknownActiveProfile(_)) => Ok(Some(wanted)),
            Err(other) => Err(other),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TomlSignexSettings {
    application: String,
    file_kind: String,
    version: u32,
}

impl Default for TomlSignexSettings {
    fn default() -> Self {
        Self {
            application: "signex".to_string(),
            file_kind: "keyboard_shortcuts".to_string(),
            version: 1,
        }
    }
}

impl TomlSignexSettings {
    fn validate(&self) -> Result<(), ProfileLoadError> {
        if self.application != "signex" {
            return Err(ProfileLoadError::InvalidHeader(
                "application must be `signex`".to_string(),
            ));
        }
        if self.file_kind != "keyboard_shortcuts" {
            return Err(ProfileLoadError::InvalidHeader(
                "file_kind must be `keyboard_shortcuts`".to_string(),
            ));
        }
        if self.version != 1 {
            return Err(ProfileLoadError::UnsupportedHeaderVersion(self.version));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TomlKeyboardShortcutsConfig {
    active_profile: String,
    #[serde(default)]
    profiles: Vec<TomlKeyboardShortcuts>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TomlKeyboardShortcuts {
    schema_version: u32,
    profile_id: String,
    profile_name: String,
    profile_kind: ShortcutProfileKind,
    description: Option<String>,
    #[serde(default)]
    base_profile: Option<String>,
    #[serde(default)]
    sections: Vec<TomlKeymapSection>,
}

impl TomlKeyboardShortcuts {
    /// Every section is parsed and validated on every OS, so a typo in a
    /// section for another OS fails here too; its bindings keep their tag.
    fn into_profile(self) -> Result<ShortcutProfile, ProfileLoadError> {
        validate_profile_id(&self.profile_id)?;
        if self.schema_version != 1 {
            return Err(ProfileLoadError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        let bindings = self
            .sections
            .into_iter()
            .flat_map(TomlKeymapSection::into_bindings)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ShortcutProfile {
            id: self.profile_id,
            name: self.profile_name,
            kind: self.profile_kind,
            schema_version: self.schema_version,
            description: self.description,
            base_profile: self.base_profile,
            bindings,
        })
    }

    fn from_profile(profile: &ShortcutProfile) -> Result<Self, ProfileLoadError> {
        validate_profile_id(&profile.id)?;
        if profile.schema_version != 1 {
            return Err(ProfileLoadError::UnsupportedSchemaVersion(
                profile.schema_version,
            ));
        }
        let sections = profile
            .bindings
            .iter()
            .map(TomlKeymapSection::from_binding)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            schema_version: profile.schema_version,
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            profile_kind: profile.kind,
            description: profile.description.clone(),
            base_profile: profile.base_profile.clone(),
            sections,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TomlKeymapSection {
    /// A section tagged with a platform applies on that platform only; it is
    /// kept and saved back on every other OS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    platform: Option<Platform>,
    #[serde(default)]
    context: ShortcutContext,
    #[serde(default)]
    bindings: BTreeMap<String, AppCommandId>,
    #[serde(default)]
    unbind: BTreeMap<String, AppCommandId>,
}

impl TomlKeymapSection {
    fn into_bindings(self) -> impl Iterator<Item = Result<ShortcutBinding, ProfileLoadError>> {
        let (context, platform) = (self.context, self.platform);
        self.bindings
            .into_iter()
            .map(move |(trigger, command)| {
                Ok(ShortcutBinding {
                    action: ShortcutBindingAction::Command(command),
                    context,
                    triggers: vec![
                        ShortcutTrigger::parse(&trigger).map_err(ProfileLoadError::KeyParse)?,
                    ],
                    platform,
                })
            })
            .chain(self.unbind.into_iter().map(move |(trigger, command)| {
                Ok(ShortcutBinding {
                    action: ShortcutBindingAction::Unbind(command),
                    context,
                    triggers: vec![
                        ShortcutTrigger::parse(&trigger).map_err(ProfileLoadError::KeyParse)?,
                    ],
                    platform,
                })
            }))
    }

    fn from_binding(binding: &ShortcutBinding) -> Result<Self, ProfileLoadError> {
        let mut bindings = BTreeMap::new();
        let mut unbind = BTreeMap::new();
        for trigger in &binding.triggers {
            match &binding.action {
                ShortcutBindingAction::Command(command) => {
                    bindings.insert(trigger.config_text(), command.clone());
                }
                ShortcutBindingAction::Unbind(command) => {
                    unbind.insert(trigger.config_text(), command.clone());
                }
                ShortcutBindingAction::NoAction => return Err(ProfileLoadError::NoActionExport),
            }
        }
        Ok(Self {
            platform: binding.platform,
            context: binding.context,
            bindings,
            unbind,
        })
    }
}

pub(super) fn validate_profile_id(id: &str) -> Result<(), ProfileLoadError> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(ProfileLoadError::InvalidProfileId(id.to_string()));
    }
    Ok(())
}
