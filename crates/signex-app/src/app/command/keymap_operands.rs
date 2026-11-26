//! Key bindings from operands (#2495): `keymap_assign` and `keymap_clear`
//! edit one custom keyboard profile through the model the Preferences page
//! edits ([`KeymapEditorModel`]), write `keyboard_shortcuts.toml` with
//! `atomic_write` and answer what they did. The built-in profiles stay
//! read-only. The active profile is not switched, and the running keymap
//! picks the change up at once. See `docs/KEYBOARD_SHORTCUTS.md`, "Key
//! bindings from the CLI".

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::ids;
use signex_i18n_ids::Translatable;

use super::{CommandArgs, CommandOutcome, queries::text_operand};
use crate::app::{Message, Signex};
use crate::keymap::{
    AppCommandId, BindingConflict, KeymapEditorModel, ProfileLoadError, ShortcutContext,
    ShortcutProfileSet, ShortcutTrigger, metadata_for,
};

type Ran = (CommandOutcome, Task<Message>);

/// The answer, or the refusal; nothing is changed on a refusal.
fn ran(result: Result<Value, Translatable>) -> Ran {
    let outcome = match result {
        Ok(answer) => CommandOutcome::Answered(answer),
        Err(reason) => CommandOutcome::Disabled { reason },
    };
    (outcome, Task::none())
}

/// The text operand `name` of `command`, or the refusal for its absence.
fn required<'a>(args: &'a CommandArgs, command: &str, name: &str) -> Result<&'a str, Translatable> {
    text_operand(args, name).ok_or_else(|| {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NEEDS)
            .arg("command", command.to_string())
            .arg("operand", name.to_string())
    })
}

/// The command a binding names: a catalog id, or an extension id; with
/// `installed_only` only an installed extension's (a kept binding to an
/// extension not installed can still be cleared).
fn command_operand(
    app: &Signex,
    text: &str,
    installed_only: bool,
) -> Result<AppCommandId, Translatable> {
    let unknown = || {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_UNKNOWN_COMMAND)
            .arg("command", text.to_string())
    };
    let command = AppCommandId::new(text.trim()).map_err(|_| unknown())?;
    let installed = app.ui_state.extension_commands.get(&command).is_some();
    let known = metadata_for(&command).is_some()
        || (command.is_extension() && (installed || !installed_only));
    if known { Ok(command) } else { Err(unknown()) }
}

fn keys_operand(text: &str) -> Result<ShortcutTrigger, Translatable> {
    let refused = |error: String| {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_BAD_KEYS)
            .arg("keys", text.to_string())
            .arg("error", error)
    };
    match ShortcutTrigger::parse(text) {
        Ok(trigger @ ShortcutTrigger::KeySequence(_)) => Ok(trigger),
        Ok(ShortcutTrigger::PointerGesture(_)) => Err(refused("a pointer gesture".to_string())),
        Err(error) => Err(refused(error.to_string())),
    }
}

/// A context's name as the profile file writes it (`global`, `board_3d`).
fn context_name(context: ShortcutContext) -> String {
    match serde_json::to_value(context) {
        Ok(Value::String(name)) => name,
        _ => format!("{context:?}"),
    }
}

/// A context as the profile file writes it; the refusal lists every name.
fn context_operand(text: &str) -> Result<ShortcutContext, Translatable> {
    ShortcutContext::ALL
        .into_iter()
        .find(|context| context_name(*context) == text.trim())
        .ok_or_else(|| {
            let names: Vec<String> = ShortcutContext::ALL.into_iter().map(context_name).collect();
            Translatable::new(ids::PREFERENCES_KEYMAP_CMD_BAD_CONTEXT)
                .arg("context", text.to_string())
                .arg("contexts", names.join(", "))
        })
}

/// One custom profile opened for an edit, made the editor's active one.
struct Opened {
    editor: KeymapEditorModel,
    /// The set's own active profile, put back before the write.
    active: String,
    /// The file the set is written to.
    path: PathBuf,
}

fn file_not_loaded(error: impl ToString) -> Translatable {
    Translatable::new(ids::PREFERENCES_KEYMAP_CMD_FILE_NOT_LOADED).arg("error", error.to_string())
}

/// Open the custom profile `profile` for an edit. In a headless `signex-cli`
/// run that set `UiState::keymap_command_file`, the
/// set is the user's own file, read now; otherwise the live set and its
/// file. Refused while the Keyboard shortcuts page holds work of its own
/// (Apply would write it over this edit), while the file could not be read,
/// and for a built-in or unknown profile.
fn open_profile(app: &Signex, profile: &str) -> Result<Opened, Translatable> {
    let ui = &app.ui_state;
    let page = &ui.preferences_keymap_editor;
    if ui.preferences_open && (page.differs_from(&ui.keymap_profiles) || page.has_trigger_drafts())
    {
        return Err(Translatable::new(ids::PREFERENCES_KEYMAP_CMD_PAGE_UNSAVED));
    }
    let (set, path) = match &ui.keymap_command_file {
        Some(path) => (
            crate::keymap::load_profile_set_at(path).map_err(file_not_loaded)?,
            path.clone(),
        ),
        None => {
            // The running set is the built-in fallback while the file on
            // disk could not be read: writing now would lose the profiles
            // still in it (#595).
            if let Some(error) = &ui.keymap_load_error {
                return Err(file_not_loaded(error));
            }
            let path = crate::keymap::config_path().ok_or_else(|| {
                Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NOT_SAVED)
                    .arg("error", ProfileLoadError::NoConfigDir.to_string())
            })?;
            (ui.keymap_profiles.clone(), path)
        }
    };
    let active = set.active_profile_id().to_string();
    let mut editor = KeymapEditorModel::new(set);
    let no_profile = || {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NO_PROFILE)
            .arg("profile", profile.to_string())
    };
    editor
        .set_active_profile(profile)
        .map_err(|_| no_profile())?;
    if !editor.active_profile_is_custom() {
        return Err(Translatable::new(ids::PREFERENCES_KEYMAP_CMD_BUILT_IN)
            .arg("profile", profile.to_string()));
    }
    Ok(Opened {
        editor,
        active,
        path,
    })
}

/// Put the set's active profile back and write the file. Synchronous, as
/// the Preferences save (`commit_keymap_draft`) is, so the answer says
/// whether the file was written; its move into a task is an issue to be
/// filed.
fn write(opened: Opened) -> Result<(ShortcutProfileSet, PathBuf), Translatable> {
    let Opened {
        mut editor,
        active,
        path,
    } = opened;
    editor.set_active_profile(active).map_err(|error| {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NOT_SAVED).arg("error", error.to_string())
    })?;
    let set = editor.into_profiles();
    crate::keymap::save_profile_set_at(&path, &set).map_err(|error| {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NOT_SAVED).arg("error", error.to_string())
    })?;
    Ok((set, path))
}

/// Make `set` the running keymap and the Keyboard shortcuts page's working
/// copy (which [`open_profile`] found clean); an open recorder closes, since
/// the rows it pointed at may have moved.
fn commit(app: &mut Signex, set: ShortcutProfileSet) {
    app.ui_state.preferences_keymap_recorder = None;
    app.ui_state.preferences_keymap_editor = KeymapEditorModel::new(set.clone());
    app.ui_state.set_keymap_profiles(set);
    app.ui_state.active_keymap = std::sync::Arc::new(
        app.ui_state
            .keymap_profiles
            .compile_active_in(&app.ui_state.i18n),
    );
    app.recompute_preferences_dirty();
}

/// The conflicts as the answer lists them.
fn conflicts_json(conflicts: &[BindingConflict]) -> Value {
    Value::Array(
        conflicts
            .iter()
            .map(|conflict| {
                json!({
                    "keys": conflict.trigger,
                    "context": conflict.context,
                    "commands": [conflict.first_command.as_str(), conflict.second_command.as_str()],
                    "reserved_by_os": conflict.system_reserved,
                })
            })
            .collect(),
    )
}

/// `keymap_assign command=.. keys=.. profile=.. [context=..]`: what OK in
/// the recorder does for a command with no key, then Apply.
pub(super) fn keymap_assign(app: &mut Signex, args: &CommandArgs) -> Ran {
    ran(assign(app, args))
}

fn assign(app: &mut Signex, args: &CommandArgs) -> Result<Value, Translatable> {
    const ID: &str = "keymap_assign";
    let command = command_operand(app, required(args, ID, "command")?, true)?;
    let keys = keys_operand(required(args, ID, "keys")?)?;
    let profile = required(args, ID, "profile")?.trim().to_string();
    let context = match text_operand(args, "context") {
        Some(text) => context_operand(text)?,
        None => crate::keymap::default_context(&command),
    };
    let mut opened = open_profile(app, &profile)?;
    let answer = |unchanged: bool, conflicts: &[BindingConflict], path: &PathBuf| {
        json!({
            "profile": profile,
            "command": command.as_str(),
            "keys": keys.config_text(),
            "context": context,
            "unchanged": unchanged,
            "conflicts": conflicts_json(conflicts),
            "file": path.display().to_string(),
        })
    };
    // The same key on the same command and context is not added twice.
    if opened.editor.has_active_key(&command, context, &keys) {
        let conflicts = opened.editor.key_conflicts(&command, context, &keys);
        return Ok(answer(true, &conflicts, &opened.path));
    }
    opened
        .editor
        .assign_active_trigger(command.clone(), context, keys.clone())
        .map_err(|error| {
            Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NOT_SAVED).arg("error", error.to_string())
        })?;
    let conflicts = opened.editor.key_conflicts(&command, context, &keys);
    let (set, path) = write(opened)?;
    commit(app, set);
    Ok(answer(false, &conflicts, &path))
}

/// `keymap_clear command=.. profile=.. [keys=..]`: the key `keys`, or every
/// key, taken from the command, as Clear then OK in the recorder does.
pub(super) fn keymap_clear(app: &mut Signex, args: &CommandArgs) -> Ran {
    ran(clear(app, args))
}

fn clear(app: &mut Signex, args: &CommandArgs) -> Result<Value, Translatable> {
    const ID: &str = "keymap_clear";
    let command = command_operand(app, required(args, ID, "command")?, false)?;
    let keys = text_operand(args, "keys").map(keys_operand).transpose()?;
    let profile = required(args, ID, "profile")?.trim().to_string();
    let mut opened = open_profile(app, &profile)?;
    let removed = match &keys {
        Some(keys) => opened.editor.clear_active_command_key(&command, keys),
        None => opened.editor.clear_active_command_keys(&command),
    }
    .map_err(|error| {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NOT_SAVED).arg("error", error.to_string())
    })?;
    let path = if removed == 0 {
        opened.path
    } else {
        let (set, path) = write(opened)?;
        commit(app, set);
        path
    };
    Ok(json!({
        "profile": profile,
        "command": command.as_str(),
        "keys": keys.map(|keys| keys.config_text()),
        "removed": removed,
        "unchanged": removed == 0,
        "file": path.display().to_string(),
    }))
}
