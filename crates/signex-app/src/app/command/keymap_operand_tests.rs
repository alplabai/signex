//! #2495: `keymap_assign` and `keymap_clear` edit a custom keyboard profile
//! from operands, in a window and headless, write `keyboard_shortcuts.toml`
//! and answer with JSON; a built-in profile is refused.

use serde_json::Value;
use signex_i18n::RenderTranslatable as _;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::config_root::isolated::IsolatedConfigRoot;
use crate::keymap::{
    AppCommandId, ExtensionCommandName, KeymapEditorModel, ShortcutBindingAction,
    ShortcutProfileSet,
};

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn args(command: &str, operands: &[&str]) -> CommandArgs {
    CommandArgs::parse_operands(&id(command), operands).expect("operands parse")
}

/// An app whose live set holds the custom profile `mine`, a copy of the
/// standard profile, while the standard profile stays active.
fn app_with_a_custom_profile() -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut editor = KeymapEditorModel::new(app.ui_state.keymap_profiles.clone());
    let active = editor.active_profile().id.clone();
    editor.create_custom_from_active("mine", "Mine").unwrap();
    editor.set_active_profile(active).unwrap();
    let set = editor.into_profiles();
    app.ui_state.set_keymap_profiles(set.clone());
    app.ui_state.preferences_keymap_editor = KeymapEditorModel::new(set);
    app
}

/// The operand run in a window app (Preferences may be open), as an
/// in-process request with operands runs it. A key or a menu row carries
/// no operands; see `without_operands_a_key_or_the_palette_is_refused`.
fn run_in_window(app: &mut Signex, command: &str, operands: &[&str]) -> CommandOutcome {
    app.run_operands(&id(command), &args(command, operands)).0
}

/// A headless run, as `signex-cli` does it.
fn run_headless(app: &mut Signex, command: &str, operands: &[&str]) -> CommandOutcome {
    app.run_headless_command_in_test(&id(command), args(command, operands))
        .0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn saved() -> ShortcutProfileSet {
    crate::keymap::load_profile_set_at(&crate::keymap::config_path().unwrap()).unwrap()
}

fn keys_of(set: &ShortcutProfileSet, profile: &str, command: &str) -> Vec<String> {
    let profile = set.profiles().find(|p| p.id == profile).expect("profile");
    profile
        .bindings
        .iter()
        .filter(|binding| binding.action == ShortcutBindingAction::Command(id(command)))
        .flat_map(|binding| binding.triggers.iter().map(|t| t.config_text()))
        .collect()
}

/// A catalog command the standard profile gives no key.
const UNBOUND: &str = "open_compiled_design";

#[test]
fn assign_in_a_window_writes_the_custom_profile_and_runs_at_once() {
    let _root = IsolatedConfigRoot::new();
    let mut app = app_with_a_custom_profile();
    let _ = app.update(crate::app::Message::Preferences(
        crate::app::PreferencesMsg::Open,
    ));
    assert!(
        !app.ui_state.preferences_dirty,
        "fixture: Preferences opens clean"
    );
    assert!(keys_of(&app.ui_state.keymap_profiles, "mine", UNBOUND).is_empty());
    let active_before = app.ui_state.keymap_profiles.active_profile_id().to_string();

    let answer = answered(run_in_window(
        &mut app,
        "keymap_assign",
        &[
            &format!("command={UNBOUND}"),
            "keys=Ctrl+Alt+Q",
            "profile=mine",
        ],
    ));

    assert_eq!(answer["profile"], "mine");
    assert_eq!(answer["command"], UNBOUND);
    assert_eq!(answer["keys"], "Ctrl+Alt+Q");
    assert_eq!(answer["context"], "global");
    assert_eq!(
        answer["file"],
        crate::keymap::config_path().unwrap().display().to_string()
    );
    assert_eq!(keys_of(&saved(), "mine", UNBOUND), ["Ctrl+Alt+Q"]);
    assert_eq!(
        keys_of(&app.ui_state.keymap_profiles, "mine", UNBOUND),
        ["Ctrl+Alt+Q"]
    );
    assert_eq!(
        app.ui_state.keymap_profiles.active_profile_id(),
        active_before,
        "the active profile is not switched"
    );
    assert_eq!(
        app.ui_state.preferences_keymap_editor.profile_set(),
        &app.ui_state.keymap_profiles,
        "a Preferences page with no change of its own shows the new key"
    );
    assert!(!app.ui_state.preferences_dirty);
}

#[test]
fn assign_and_clear_headless_as_the_cli_runs_them() {
    let _root = IsolatedConfigRoot::new();
    let mut app = app_with_a_custom_profile();

    let assigned = answered(run_headless(
        &mut app,
        "keymap_assign",
        &[
            &format!("command={UNBOUND}"),
            "keys=P X",
            "profile=mine",
            "context=pcb",
        ],
    ));
    assert_eq!(assigned["context"], "pcb");
    assert_eq!(keys_of(&saved(), "mine", UNBOUND), ["P X"]);

    let cleared = answered(run_headless(
        &mut app,
        "keymap_clear",
        &[&format!("command={UNBOUND}"), "profile=mine"],
    ));
    assert_eq!(cleared["removed"], 1);
    assert_eq!(cleared["keys"], Value::Null);
    assert!(keys_of(&saved(), "mine", UNBOUND).is_empty());
}

#[test]
fn clear_with_keys_takes_only_that_key() {
    let _root = IsolatedConfigRoot::new();
    let mut app = app_with_a_custom_profile();
    let wire = keys_of(&app.ui_state.keymap_profiles, "mine", "place_wire");
    assert!(wire.len() >= 2, "place_wire has several keys: {wire:?}");
    let gone = wire[0].clone();

    let answer = answered(run_headless(
        &mut app,
        "keymap_clear",
        &[
            "command=place_wire",
            &format!("keys={gone}"),
            "profile=mine",
        ],
    ));

    assert_eq!(answer["removed"], 1);
    let left = keys_of(&saved(), "mine", "place_wire");
    assert_eq!(left, wire[1..]);
    assert_eq!(
        keys_of(
            &app.ui_state.keymap_profiles,
            "signex_standard",
            "place_wire"
        ),
        wire,
        "the built-in profile it was copied from is untouched"
    );
}

#[test]
fn a_built_in_profile_is_refused_and_nothing_is_written() {
    let _root = IsolatedConfigRoot::new();
    let mut app = app_with_a_custom_profile();
    let before = app.ui_state.keymap_profiles.clone();
    let path = crate::keymap::config_path().unwrap();

    for (command, operands) in [
        (
            "keymap_assign",
            vec![format!("command={UNBOUND}"), "keys=Ctrl+Alt+Q".into()],
        ),
        ("keymap_clear", vec!["command=place_wire".to_string()]),
    ] {
        for profile in ["signex_standard", "classic"] {
            let mut given: Vec<String> = operands.clone();
            given.push(format!("profile={profile}"));
            let given: Vec<&str> = given.iter().map(String::as_str).collect();
            for outcome in [
                run_headless(&mut app, command, &given),
                run_in_window(&mut app, command, &given),
            ] {
                let reason = refusal(outcome);
                assert!(reason.contains("read-only"), "{command}: {reason}");
            }
        }
    }
    assert_eq!(app.ui_state.keymap_profiles, before);
    assert!(!path.exists(), "nothing was written");
}

#[test]
fn unknown_profiles_commands_keys_and_contexts_are_refused() {
    let _root = IsolatedConfigRoot::new();
    let mut app = app_with_a_custom_profile();
    let before = app.ui_state.keymap_profiles.clone();
    let cases: [(&[&str], &str); 5] = [
        (
            &["command=find", "keys=F9", "profile=nobody"],
            "No keyboard profile",
        ),
        (
            &["command=not_a_command", "keys=F9", "profile=mine"],
            "not a command",
        ),
        (
            &["command=ext.sample.missing", "keys=F9", "profile=mine"],
            "not a command",
        ),
        (
            &["command=find", "keys=Ctrl+Nope", "profile=mine"],
            "not a key sequence",
        ),
        (
            &["command=find", "keys=F9", "profile=mine", "context=nowhere"],
            "not a shortcut context",
        ),
    ];
    for (operands, fragment) in cases {
        let reason = refusal(run_headless(&mut app, "keymap_assign", operands));
        assert!(reason.contains(fragment), "{operands:?}: {reason}");
    }
    assert_eq!(app.ui_state.keymap_profiles, before);
}

#[test]
fn nothing_is_written_over_a_file_that_failed_to_load() {
    let _root = IsolatedConfigRoot::new();
    let mut app = app_with_a_custom_profile();
    app.ui_state.keymap_load_error = Some("broken".into());

    let reason = refusal(run_headless(
        &mut app,
        "keymap_assign",
        &["command=find", "keys=F9", "profile=mine"],
    ));

    assert!(reason.contains("could not be loaded"), "{reason}");
    assert!(!crate::keymap::config_path().unwrap().exists());
}

#[test]
fn an_installed_extension_command_is_assigned_and_a_kept_one_cleared() {
    let _root = IsolatedConfigRoot::new();
    let mut app = app_with_a_custom_profile();
    let hello = id("ext.sample.say_hello");
    app.ui_state.extension_commands.insert(
        hello.clone(),
        ExtensionCommandName {
            extension: "sample".into(),
            label: signex_i18n::shortcut_ids::PREF_KEYMAP_OK,
            category: "sample",
        },
    );

    answered(run_headless(
        &mut app,
        "keymap_assign",
        &[
            "command=ext.sample.say_hello",
            "keys=Ctrl+Alt+H",
            "profile=mine",
        ],
    ));
    assert_eq!(
        keys_of(&saved(), "mine", "ext.sample.say_hello"),
        ["Ctrl+Alt+H"]
    );

    // Uninstalled, the binding is kept; it can still be cleared by its id.
    app.ui_state.extension_commands = Default::default();
    let cleared = answered(run_headless(
        &mut app,
        "keymap_clear",
        &["command=ext.sample.say_hello", "profile=mine"],
    ));
    assert_eq!(cleared["removed"], 1);
    assert!(keys_of(&saved(), "mine", hello.as_str()).is_empty());
}

#[test]
fn without_operands_a_key_or_the_palette_is_refused() {
    let (mut app, _boot) = Signex::new();
    for command in ["keymap_assign", "keymap_clear"] {
        let (outcome, _) = app.dispatch_command_reporting(&id(command), CommandArgs::none(), None);
        let reason = refusal(outcome);
        assert!(reason.contains("Keyboard shortcuts"), "{command}: {reason}");
    }
}
