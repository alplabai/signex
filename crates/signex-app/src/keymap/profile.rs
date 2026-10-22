use crate::keymap::{
    AppCommandId, KeyBindingSource, KeyStroke, ShortcutBinding, ShortcutBindingAction,
    ShortcutContext, ShortcutTrigger,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
};

const ALTIUM_PROFILE_TOML: &str = include_str!("../../assets/keyboard-shortcuts/altium.toml");
const CLASSIC_PROFILE_TOML: &str = include_str!("../../assets/keyboard-shortcuts/classic.toml");
const USER_SHORTCUTS_FILE_NAME: &str = "keyboard_shortcuts.toml";

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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompiledKeymap {
    bindings: Vec<CompiledBinding>,
}

impl CompiledKeymap {
    pub fn compile(profile: &ShortcutProfile) -> Self {
        let source = match profile.kind {
            ShortcutProfileKind::BuiltIn => KeyBindingSource::BuiltIn(profile.id.clone()),
            ShortcutProfileKind::Custom => KeyBindingSource::Custom(profile.id.clone()),
        };
        let mut bindings = Vec::new();
        for binding in &profile.bindings {
            for trigger in &binding.triggers {
                if let ShortcutTrigger::KeySequence(sequence) = trigger {
                    bindings.push(CompiledBinding {
                        action: binding.action.clone(),
                        context: binding.context,
                        sequence: sequence.clone(),
                        source: source.clone(),
                    });
                }
            }
        }
        Self { bindings }
    }

    pub fn lookup(&self, input: &[KeyStroke], contexts: &[ShortcutContext]) -> KeyLookup {
        let mut matches = self
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| {
                let pending = binding.matches_input(input)?;
                let depth = context_depth(binding.context, contexts)?;
                Some((pending, depth, index, binding))
            })
            .collect::<Vec<_>>();

        let pending = matches.iter().any(|(pending, ..)| *pending);
        matches.retain(|(pending, ..)| !*pending);
        let matched = !matches.is_empty();
        matches.sort_by(|(_, depth_a, index_a, _), (_, depth_b, index_b, _)| {
            depth_b.cmp(depth_a).then(index_b.cmp(index_a))
        });

        let command = resolve_matched_command(matches);
        KeyLookup {
            command,
            pending,
            matched,
        }
    }

    pub fn shortcut_label(&self, command: &AppCommandId) -> Option<String> {
        // The last matching binding wins (later profile layers / sections
        // override earlier ones), hence the reversed scan.
        let binding = self
            .bindings
            .iter()
            .rev()
            .find(|binding| binding.action.command() == Some(command))?;
        match &binding.action {
            ShortcutBindingAction::Command(_) => Some(
                binding
                    .sequence
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            ShortcutBindingAction::Unbind(_) | ShortcutBindingAction::NoAction => None,
        }
    }

    pub fn conflicts(&self) -> Vec<BindingConflict> {
        let mut seen: BTreeMap<(ShortcutContext, Vec<KeyStroke>), &CompiledBinding> =
            BTreeMap::new();
        let mut conflicts = Vec::new();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| matches!(binding.action, ShortcutBindingAction::Command(_)))
        {
            let key = (binding.context, binding.sequence.clone());
            if let Some(existing) = seen.insert(key, binding)
                && existing.action != binding.action
            {
                let Some(first_command) = existing.action.command().cloned() else {
                    continue;
                };
                let Some(second_command) = binding.action.command().cloned() else {
                    continue;
                };
                conflicts.push(BindingConflict {
                    context: binding.context,
                    trigger: binding
                        .sequence
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" "),
                    first_command,
                    second_command,
                });
            }
        }
        conflicts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompiledBinding {
    action: ShortcutBindingAction,
    context: ShortcutContext,
    sequence: Vec<KeyStroke>,
    source: KeyBindingSource,
}

impl CompiledBinding {
    fn matches_input(&self, input: &[KeyStroke]) -> Option<bool> {
        if input.len() > self.sequence.len() {
            return None;
        }
        self.sequence
            .iter()
            .zip(input)
            .all(|(expected, actual)| expected == actual)
            .then_some(input.len() < self.sequence.len())
    }
}

fn resolve_matched_command(
    matches: Vec<(bool, usize, usize, &CompiledBinding)>,
) -> Option<AppCommandId> {
    let mut unbound = Vec::new();
    for (_, _, _, binding) in matches {
        match &binding.action {
            ShortcutBindingAction::Command(command) => {
                if !unbound.iter().any(|blocked| blocked == command) {
                    return Some(command.clone());
                }
            }
            ShortcutBindingAction::Unbind(command) => unbound.push(command.clone()),
            ShortcutBindingAction::NoAction => return None,
        }
    }
    None
}

fn context_depth(context: ShortcutContext, contexts: &[ShortcutContext]) -> Option<usize> {
    if context == ShortcutContext::Global {
        return Some(0);
    }
    contexts
        .iter()
        .position(|candidate| *candidate == context)
        .map(|position| position + 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyLookup {
    pub command: Option<AppCommandId>,
    pub pending: bool,
    pub matched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingConflict {
    pub context: ShortcutContext,
    pub trigger: String,
    pub first_command: AppCommandId,
    pub second_command: AppCommandId,
}

pub fn config_path() -> Option<PathBuf> {
    crate::config_root::config_root().map(|root| root.join(USER_SHORTCUTS_FILE_NAME))
}

pub fn config_path_for_dir(base: &Path) -> PathBuf {
    crate::config_root::config_root_for_dir(base).join(USER_SHORTCUTS_FILE_NAME)
}

pub fn load_profile_set() -> Result<ShortcutProfileSet, ProfileLoadError> {
    let Some(path) = config_path() else {
        return ShortcutProfileSet::built_ins();
    };
    load_profile_set_at(&path)
}

pub fn load_profile_set_at(path: &Path) -> Result<ShortcutProfileSet, ProfileLoadError> {
    let mut set = ShortcutProfileSet::built_ins()?;
    if !path.exists() {
        return Ok(set);
    }

    let source = std::fs::read_to_string(path).map_err(ProfileLoadError::Io)?;
    let config = TomlShortcutConfig::parse(&source)?;
    config.apply_to(&mut set)?;
    Ok(set)
}

pub fn save_profile_set(set: &ShortcutProfileSet) -> Result<(), ProfileLoadError> {
    let Some(path) = config_path() else {
        return Err(ProfileLoadError::NoConfigDir);
    };
    save_profile_set_at(&path, set)
}

/// Crash-safe: [`signex_types::atomic_io::atomic_write`] writes to a temp
/// sibling, fsyncs it and renames over the destination, so a crash mid-save
/// leaves the user's previously saved keymap profiles intact rather than a
/// truncated file. It also creates the parent directory, so no separate
/// `create_dir_all` here.
pub fn save_profile_set_at(path: &Path, set: &ShortcutProfileSet) -> Result<(), ProfileLoadError> {
    let source = export_custom_profiles(set)?;
    signex_types::atomic_io::atomic_write(path, source.as_bytes()).map_err(ProfileLoadError::Io)
}

/// Sibling `.bak` path — appended to the WHOLE file name, so
/// `keyboard_shortcuts.toml` backs up to `keyboard_shortcuts.toml.bak`
/// rather than the `keyboard_shortcuts.bak` that `set_extension` would
/// produce. Built through [`std::ffi::OsString`] so a non-UTF-8 config
/// directory name survives instead of being mangled by a lossy
/// round-trip. `None` only when `path` has no file name at all.
fn backup_path_for(path: &Path) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(".bak");
    Some(path.with_file_name(name))
}

/// Copy the existing shortcuts file aside before a save that would
/// otherwise overwrite profiles this process failed to load (#595).
///
/// Three outcomes, all load-bearing:
///
/// * `path` does not exist — `Ok(None)`, there is nothing to preserve.
/// * the `.bak` sibling already exists — `Ok(None)` **without copying**.
///   That existing backup is the original. A second Apply would
///   otherwise copy the already-overwritten file over it and destroy the
///   only surviving copy of the user's profiles.
/// * otherwise — copy `path` aside and return `Ok(Some(bak_path))`.
///
/// An `Err` means the copy could not be made, and the caller must then
/// refuse to save: overwriting after a failed backup is the data loss
/// this function exists to prevent, not a fallback.
pub fn back_up_profile_file_at(path: &Path) -> Result<Option<PathBuf>, ProfileLoadError> {
    if !path.exists() {
        return Ok(None);
    }
    let Some(bak) = backup_path_for(path) else {
        return Ok(None);
    };
    if bak.exists() {
        return Ok(None);
    }
    std::fs::copy(path, &bak).map_err(ProfileLoadError::Io)?;
    Ok(Some(bak))
}

/// [`back_up_profile_file_at`] against the resolved user config path,
/// mirroring the [`save_profile_set`] / [`save_profile_set_at`] pair.
pub fn back_up_profile_file() -> Result<Option<PathBuf>, ProfileLoadError> {
    let Some(path) = config_path() else {
        return Err(ProfileLoadError::NoConfigDir);
    };
    back_up_profile_file_at(&path)
}

/// The `.bak` sibling of the resolved shortcuts file, whether or not it
/// exists. `None` only when there is no config directory at all.
///
/// Exposed so the Preferences pane can say the backup is there after the
/// load-error banner has gone (#603) — before this, the only mention of
/// the file was a transient status line at the moment it was written.
pub fn backup_profiles_path() -> Option<PathBuf> {
    config_path().and_then(|path| backup_path_for(&path))
}

/// [`backup_profiles_path`] filtered to a backup that is actually on
/// disk — what the UI asks before offering Restore and Discard.
///
/// `Path::exists()` is the right predicate here and only here: it
/// decides whether to *show* two controls, so treating an un-stat-able
/// file as absent costs a hidden row, not a lost file. The destructive
/// paths ([`free_aside_path`], [`discard_profile_backup_at`]) go through
/// `symlink_metadata` and never through this.
pub fn existing_backup_profiles_path() -> Option<PathBuf> {
    backup_profiles_path().filter(|bak| bak.exists())
}

/// How many aside slots to try before giving up, matching
/// `fonts::prefs_file`'s ladder. A user who has restored a hundred times
/// has a problem no rename will fix.
const MAX_ASIDE_SLOTS: u32 = 100;

/// First free sibling name to move the live shortcuts file aside to
/// before a restore overwrites it.
///
/// Slot 1 is the plain `.bak`; the ladder numbers from `.bak.2`, so this
/// covers exactly [`MAX_ASIDE_SLOTS`] names and the exhaustion message
/// below stays true. A slot counts as free only when
/// `symlink_metadata` answers `NotFound`; any other error aborts
/// carrying that errno rather than laddering past it, because
/// `Path::exists()` answers `false` for a name it merely cannot stat and
/// `std::fs::rename` silently replaces its destination — the file that
/// vanished would be the user's earlier backup.
fn free_aside_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    let Some(file_name) = path.file_name() else {
        return Err(std::io::Error::other(format!(
            "no name to move {} aside to: the path has no file name",
            path.display()
        )));
    };
    let mut base = file_name.to_os_string();
    base.push(".bak");
    for slot in 1..=MAX_ASIDE_SLOTS {
        let mut name = base.clone();
        if slot > 1 {
            name.push(format!(".{slot}"));
        }
        let candidate = path.with_file_name(name);
        match std::fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
    }
    Err(std::io::Error::other(format!(
        "no free name to move {} aside to: all {MAX_ASIDE_SLOTS} .bak slots \
         beside it are already taken",
        path.display()
    )))
}

/// What a restore recovered out of a backup, and what it could not.
pub struct RestoredProfiles {
    /// The profile set the backup describes, built-ins included.
    pub set: ShortcutProfileSet,
    /// `Some(id)` when the backup's `active_profile` named a profile
    /// that is not in it. The custom profiles are all present and this
    /// is the one thing that could not be honoured, so the set comes
    /// back with the built-in default active rather than failing the
    /// whole restore over a dangling pointer. That is the single most
    /// likely reason the file failed to load in the first place.
    pub active_profile_reset: Option<String>,
}

/// Read the profiles out of a backup file, through the normal loader.
///
/// Deliberately NOT "copy the `.bak` back over the live file". The
/// backup holds the file that failed to load; putting it back would
/// reproduce the same failure on the next launch. Parsing it instead
/// means an unrepairable backup fails here, cleanly, with both files
/// untouched — and a backup whose only fault was a dangling
/// `active_profile` is recovered in full.
pub fn read_backup_profiles_at(bak: &Path) -> Result<RestoredProfiles, ProfileLoadError> {
    let source = std::fs::read_to_string(bak).map_err(ProfileLoadError::Io)?;
    let mut set = ShortcutProfileSet::built_ins()?;
    let active_profile_reset = TomlShortcutConfig::parse(&source)?.apply_to_recovering(&mut set)?;
    Ok(RestoredProfiles {
        set,
        active_profile_reset,
    })
}

/// Move the live shortcuts file aside to a free slot, then write `set`
/// in its place. Returns where the previous file went, or `Ok(None)`
/// when there was no live file to preserve.
///
/// The move is what makes a restore safe to offer at any time. The
/// `.bak` is never cleaned up, so it outlives the failure that produced
/// it: months later the user may have built a whole new set of profiles
/// and still have that ancient backup sitting there. Writing straight
/// over the live file would throw the new work away — the same shape as
/// the `prefs.json` clobber in #594.
pub fn restore_profiles_at(
    path: &Path,
    set: &ShortcutProfileSet,
) -> Result<Option<PathBuf>, ProfileLoadError> {
    let aside = match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(ProfileLoadError::Io(error)),
        Ok(_) => {
            let target = free_aside_path(path).map_err(ProfileLoadError::Io)?;
            std::fs::rename(path, &target).map_err(ProfileLoadError::Io)?;
            Some(target)
        }
    };
    save_profile_set_at(path, set)?;
    Ok(aside)
}

/// Delete a profile backup. Only ever called from an explicit user
/// action — a successful save must never remove it, because that is
/// exactly the moment the user is most likely to want the old profiles
/// back. `Ok(false)` when there was nothing to delete.
pub fn discard_profile_backup_at(bak: &Path) -> Result<bool, ProfileLoadError> {
    match std::fs::remove_file(bak) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ProfileLoadError::Io(error)),
    }
}

pub fn import_custom_profile(source: &str) -> Result<ShortcutProfile, ProfileLoadError> {
    let profile = TomlShortcutProfile::parse(source)?.into_profile()?;
    if profile.kind != ShortcutProfileKind::Custom {
        return Err(ProfileLoadError::BuiltInMutation(profile.id));
    }
    Ok(profile)
}

pub fn export_custom_profile(profile: &ShortcutProfile) -> Result<String, ProfileLoadError> {
    if profile.kind != ShortcutProfileKind::Custom {
        return Err(ProfileLoadError::BuiltInMutation(profile.id.clone()));
    }
    let document = TomlShortcutProfile::from_profile(profile)?;
    toml::to_string_pretty(&document).map_err(ProfileLoadError::TomlSerialize)
}

pub fn export_custom_profiles(set: &ShortcutProfileSet) -> Result<String, ProfileLoadError> {
    let document = TomlShortcutConfig::from_profile_set(set)?;
    toml::to_string_pretty(&document).map_err(ProfileLoadError::TomlSerialize)
}

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

    fn from_profile(profile: &ShortcutProfile) -> Result<Self, ProfileLoadError> {
        Ok(Self {
            signex_settings: TomlSignexSettings::default(),
            keyboard_shortcuts: TomlKeyboardShortcuts::from_profile(profile)?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TomlShortcutConfig {
    signex_settings: TomlSignexSettings,
    keyboard_shortcuts: TomlKeyboardShortcutsConfig,
}

impl TomlShortcutConfig {
    fn parse(source: &str) -> Result<Self, ProfileLoadError> {
        toml::from_str(source).map_err(ProfileLoadError::Toml)
    }

    fn from_profile_set(set: &ShortcutProfileSet) -> Result<Self, ProfileLoadError> {
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

    fn apply_to(self, set: &mut ShortcutProfileSet) -> Result<(), ProfileLoadError> {
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
    fn apply_to_recovering(
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
    #[serde(default)]
    context: ShortcutContext,
    #[serde(default)]
    bindings: BTreeMap<String, AppCommandId>,
    #[serde(default)]
    unbind: BTreeMap<String, AppCommandId>,
}

impl TomlKeymapSection {
    fn into_bindings(self) -> impl Iterator<Item = Result<ShortcutBinding, ProfileLoadError>> {
        let context = self.context;
        self.bindings
            .into_iter()
            .map(move |(trigger, command)| {
                Ok(ShortcutBinding {
                    action: ShortcutBindingAction::Command(command),
                    context,
                    triggers: vec![
                        ShortcutTrigger::parse(&trigger).map_err(ProfileLoadError::KeyParse)?,
                    ],
                })
            })
            .chain(self.unbind.into_iter().map(move |(trigger, command)| {
                Ok(ShortcutBinding {
                    action: ShortcutBindingAction::Unbind(command),
                    context,
                    triggers: vec![
                        ShortcutTrigger::parse(&trigger).map_err(ProfileLoadError::KeyParse)?,
                    ],
                })
            }))
    }

    fn from_binding(binding: &ShortcutBinding) -> Result<Self, ProfileLoadError> {
        let mut bindings = BTreeMap::new();
        let mut unbind = BTreeMap::new();
        for trigger in &binding.triggers {
            match &binding.action {
                ShortcutBindingAction::Command(command) => {
                    bindings.insert(trigger.display_text(), command.clone());
                }
                ShortcutBindingAction::Unbind(command) => {
                    unbind.insert(trigger.display_text(), command.clone());
                }
                ShortcutBindingAction::NoAction => return Err(ProfileLoadError::NoActionExport),
            }
        }
        Ok(Self {
            context: binding.context,
            bindings,
            unbind,
        })
    }
}

fn validate_profile_id(id: &str) -> Result<(), ProfileLoadError> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(ProfileLoadError::InvalidProfileId(id.to_string()));
    }
    Ok(())
}

#[derive(Debug)]
pub enum ProfileLoadError {
    Toml(toml::de::Error),
    TomlSerialize(toml::ser::Error),
    Io(io::Error),
    KeyParse(crate::keymap::KeyParseError),
    NoConfigDir,
    InvalidHeader(String),
    UnsupportedHeaderVersion(u32),
    UnsupportedSchemaVersion(u32),
    InvalidProfileId(String),
    DuplicateProfileId,
    UnknownActiveProfile(String),
    BuiltInMutation(String),
    NoActionExport,
}

impl fmt::Display for ProfileLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(error) => write!(f, "{error}"),
            Self::TomlSerialize(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::KeyParse(error) => write!(f, "{error}"),
            Self::NoConfigDir => f.write_str("no config directory available"),
            Self::InvalidHeader(error) => write!(f, "invalid shortcut profile header: {error}"),
            Self::UnsupportedHeaderVersion(version) => {
                write!(f, "unsupported Signex settings header version {version}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported keyboard shortcut schema version {version}")
            }
            Self::InvalidProfileId(id) => write!(f, "invalid shortcut profile id `{id}`"),
            Self::DuplicateProfileId => f.write_str("duplicate shortcut profile id"),
            Self::UnknownActiveProfile(id) => write!(f, "unknown shortcut profile `{id}`"),
            Self::BuiltInMutation(id) => write!(f, "built-in profile `{id}` cannot be modified"),
            Self::NoActionExport => f.write_str("no-action shortcut bindings cannot be exported"),
        }
    }
}

impl Error for ProfileLoadError {}
