//! #2495: `keymap_assign` and `keymap_clear` are the one carve-out from a
//! headless run's throwaway config root: they edit the user's own
//! `keyboard_shortcuts.toml` (docs/CLI.md). A temp home stands in for the
//! user's; `dirs::config_dir` reads `XDG_CONFIG_HOME` on Linux and `HOME` on
//! macOS.
#![cfg(unix)]

mod support;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_app::keymap::{
    AppCommandId, KeymapEditorModel, ShortcutBindingAction, ShortcutProfileSet,
};
use support::{fixture, jsonl, run_cli_env, temp_root};

/// A catalog command the standard profile gives no key.
const UNBOUND: &str = "open_compiled_design";

/// The user's shortcuts file under `home`, as the binary resolves it.
fn user_file(home: &Path) -> PathBuf {
    let base = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        home.join(".config")
    };
    signex_app::keymap::config_path_for_dir(&base)
}

/// The user's file seeded with the custom profile `mine`, a copy of the
/// standard profile, which stays active.
fn seed(file: &Path) {
    let mut editor = KeymapEditorModel::built_ins().expect("built-ins");
    let active = editor.active_profile().id.clone();
    editor.create_custom_from_active("mine", "Mine").unwrap();
    editor.set_active_profile(active).unwrap();
    signex_app::keymap::save_profile_set_at(file, &editor.into_profiles()).expect("seed");
}

fn keys_of(file: &Path, command: &str) -> Vec<String> {
    let set: ShortcutProfileSet = signex_app::keymap::load_profile_set_at(file).expect("load");
    let command = AppCommandId::new(command).unwrap();
    let mine = set.profiles().find(|p| p.id == "mine").expect("mine");
    mine.bindings
        .iter()
        .filter(|b| b.action == ShortcutBindingAction::Command(command.clone()))
        .flat_map(|b| b.triggers.iter().map(|t| t.config_text()))
        .collect()
}

fn assign(profile: &str) -> Value {
    json!({"id": "keymap_assign", "args": {
        "command": UNBOUND, "keys": "Ctrl+Alt+Q", "profile": profile,
    }})
}

#[test]
fn keymap_assign_writes_the_users_file_and_a_second_run_sees_the_binding() {
    let (_project_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let (_home_guard, home) = temp_root();
    let file = user_file(&home);
    seed(&file);
    let config = home.join(".config");
    let env = [("HOME", home.as_path()), ("XDG_CONFIG_HOME", config.as_path())];

    let first = run_cli_env(&["run", project, "--save"], jsonl(&[assign("mine")]), &env);
    let line = &first.lines[0];
    assert_eq!(line["status"], "answered", "{}\n{}", first.stdout, first.stderr);
    assert_eq!(line["data"]["unchanged"], false, "{}", first.stdout);
    assert_eq!(line["data"]["file"], file.display().to_string());
    assert_eq!(keys_of(&file, UNBOUND), ["Ctrl+Alt+Q"]);

    // A second run reads the user's file: the binding is there, so the same
    // key is not added twice, and clear finds it.
    let second = run_cli_env(
        &["run", project, "--save"],
        jsonl(&[
            assign("mine"),
            json!({"id": "keymap_clear", "args": {"command": UNBOUND, "profile": "mine"}}),
        ]),
        &env,
    );
    assert_eq!(second.lines[0]["data"]["unchanged"], true, "{}", second.stdout);
    assert_eq!(second.lines[1]["data"]["removed"], 1, "{}", second.stdout);
    assert!(keys_of(&file, UNBOUND).is_empty());
}

#[test]
fn the_save_capability_and_a_built_in_profile_are_still_refused() {
    let (_project_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let (_home_guard, home) = temp_root();
    let file = user_file(&home);
    seed(&file);
    let before = std::fs::read(&file).unwrap();
    let config = home.join(".config");
    let env = [("HOME", home.as_path()), ("XDG_CONFIG_HOME", config.as_path())];

    let no_save = run_cli_env(&["run", project], jsonl(&[assign("mine")]), &env);
    assert_eq!(no_save.lines[0]["status"], "refused", "{}", no_save.stdout);
    let built_in = run_cli_env(
        &["run", project, "--save"],
        jsonl(&[assign("signex_standard")]),
        &env,
    );
    assert_eq!(built_in.lines[0]["status"], "refused", "{}", built_in.stdout);
    assert_eq!(std::fs::read(&file).unwrap(), before, "the file is unchanged");
}

#[test]
fn a_users_file_that_does_not_load_is_not_written_over() {
    let (_project_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let (_home_guard, home) = temp_root();
    let file = user_file(&home);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "this is [not a shortcuts file").unwrap();
    let config = home.join(".config");
    let env = [("HOME", home.as_path()), ("XDG_CONFIG_HOME", config.as_path())];

    let out = run_cli_env(&["run", project, "--save"], jsonl(&[assign("mine")]), &env);

    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "this is [not a shortcuts file"
    );
}
