//! Key bindings from operands (#2495): `keymap_assign` and `keymap_clear`
//! edit one custom keyboard profile through the model the Preferences page
//! edits ([`KeymapEditorModel`]), write `keyboard_shortcuts.toml` with
//! `atomic_write` and answer what they did. The built-in profiles stay
//! read-only. The active profile is not switched, and the running keymap
//! picks the change up at once. See `docs/KEYBOARD_SHORTCUTS.md`.

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::ids;
use signex_i18n_ids::Translatable;

use super::{CommandArgs, CommandOutcome, queries::text_operand};
use crate::app::{Message, Signex};
use crate::keymap::{
    AppCommandId, KeymapEditorModel, ProfileLoadError, ShortcutContext, ShortcutProfileSet,
    ShortcutTrigger, metadata_for,
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

/// A context as the profile file writes it (`global`, `pcb`, `board_3d`).
fn context_operand(text: &str) -> Result<ShortcutContext, Translatable> {
    serde_json::from_value(Value::String(text.trim().to_string())).map_err(|_| {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_BAD_CONTEXT).arg("context", text.to_string())
    })
}

/// Run `edit` on the custom profile `profile` of the live set and write the
/// file. The edit sees `profile` as the active one; the set keeps its own.
fn edit_profile<T>(
    app: &Signex,
    profile: &str,
    edit: impl FnOnce(&mut KeymapEditorModel) -> Result<T, ProfileLoadError>,
) -> Result<(T, ShortcutProfileSet), Translatable> {
    // The running set is the built-in fallback while the file on disk could
    // not be read: writing now would lose the profiles still in it (#595).
    if app.ui_state.keymap_load_error.is_some() {
        return Err(Translatable::new(
            ids::PREFERENCES_KEYMAP_CMD_FILE_NOT_LOADED,
        ));
    }
    let live = app.ui_state.keymap_profiles.clone();
    let active = live.active_profile_id().to_string();
    let mut editor = KeymapEditorModel::new(live);
    let refused = |error: ProfileLoadError| match error {
        ProfileLoadError::BuiltInMutation(_) => {
            Translatable::new(ids::PREFERENCES_KEYMAP_CMD_BUILT_IN)
                .arg("profile", profile.to_string())
        }
        _ => Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NO_PROFILE)
            .arg("profile", profile.to_string()),
    };
    editor.set_active_profile(profile).map_err(refused)?;
    let done = edit(&mut editor).map_err(refused)?;
    editor.set_active_profile(active).map_err(refused)?;
    let set = editor.into_profiles();
    // Synchronous, as the Preferences save (`commit_keymap_draft`) is, so
    // the answer says whether the file was written.
    crate::keymap::save_profile_set(&set).map_err(|error| {
        Translatable::new(ids::PREFERENCES_KEYMAP_CMD_NOT_SAVED).arg("error", error.to_string())
    })?;
    Ok((done, set))
}

/// Make `set` the running keymap. An open Preferences page that held no
/// change of its own shows it; one with changes keeps them.
fn commit(app: &mut Signex, set: ShortcutProfileSet) {
    let page_was_clean = !app
        .ui_state
        .preferences_keymap_editor
        .differs_from(&app.ui_state.keymap_profiles);
    if page_was_clean {
        app.ui_state.preferences_keymap_recorder = None;
        app.ui_state.preferences_keymap_editor = KeymapEditorModel::new(set.clone());
    }
    app.ui_state.set_keymap_profiles(set);
    app.ui_state.active_keymap = std::sync::Arc::new(
        app.ui_state
            .keymap_profiles
            .compile_active_in(&app.ui_state.i18n),
    );
    app.recompute_preferences_dirty();
}

fn file_path() -> Value {
    crate::keymap::config_path().map_or(Value::Null, |path| json!(path.display().to_string()))
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
    let config = keys.config_text();
    let (_, set) = edit_profile(app, &profile, |editor| {
        editor.assign_active_trigger(command.clone(), context, keys)
    })?;
    commit(app, set);
    Ok(json!({
        "profile": profile,
        "command": command.as_str(),
        "keys": config,
        "context": context,
        "file": file_path(),
    }))
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
    let (removed, set) = edit_profile(app, &profile, |editor| match &keys {
        Some(keys) => editor.clear_active_command_key(&command, keys),
        None => editor.clear_active_command_keys(&command),
    })?;
    commit(app, set);
    Ok(json!({
        "profile": profile,
        "command": command.as_str(),
        "keys": keys.map(|keys| keys.config_text()),
        "removed": removed,
        "file": file_path(),
    }))
}
