//! #1926: a run under a config-root override never reads or writes the real
//! config root.
//!
//! The override is process-wide and set once, so each scenario runs in a
//! child process of this test binary: [`probe`] does the work when
//! `PROBE_ENV` names a scenario and is a no-op otherwise. In a test build the
//! "real" root is the per-process test redirect, the root `config_root()`
//! answers without an override.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::config_root::{
    ConfigRootOverrideError, config_root, default_config_root, set_config_root_override,
};
use signex_app::library::pricing::resolve_client;
use signex_app::library::settings::digikey_oauth;
use signex_app::menu_bar::MenuMessage;
use signex_app::task_driver::TaskDriver;
use signex_library::{ClientUnavailable, DistributorSource};

const PROBE_ENV: &str = "SIGNEX_CONFIG_ROOT_PROBE";
/// What the paste-designators migration rewrites: the old default.
const OLD_PASTE_PREFS: &str = r#"{"paste_designators":"Keep"}"#;

fn run_probe(scenario: &str) {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, scenario)
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "scenario {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_headless_boot_open_and_save_leave_the_real_config_root_byte_for_byte_untouched() {
    run_probe("untouched");
}

#[test]
fn an_override_is_refused_after_the_first_config_read() {
    run_probe("late");
}

#[test]
fn probe() {
    let Ok(scenario) = std::env::var(PROBE_ENV) else {
        return;
    };
    match scenario.as_str() {
        "untouched" => untouched(),
        "late" => late(),
        other => panic!("unknown scenario {other}"),
    }
}

fn untouched() {
    let real = default_config_root().expect("the test redirect resolves");
    seed_real_root(&real);
    let before = snapshot(&real);

    let isolated = tempfile::tempdir().expect("override root");
    let relative = PathBuf::from("relative/root");
    assert_eq!(
        set_config_root_override(relative.clone()),
        Err(ConfigRootOverrideError::NotAbsolute(relative))
    );
    set_config_root_override(isolated.path().to_path_buf()).expect("first override");
    assert!(matches!(
        set_config_root_override(isolated.path().to_path_buf()),
        Err(ConfigRootOverrideError::AlreadySet(_))
    ));
    assert_eq!(config_root().as_deref(), Some(isolated.path()));
    std::fs::write(isolated.path().join("prefs.json"), OLD_PASTE_PREFS).expect("seed prefs");

    let work = tempfile::tempdir().expect("work dir");
    let mut driver = TaskDriver::new();
    let (mut app, boot) = Signex::boot(Vec::new());
    driver.settle(&mut app, boot).expect("boot settled");
    assert!(
        app.library.global_libraries.is_empty(),
        "the real global_libraries.toml must not be read"
    );
    assert!(
        app.library.settings.preferred_order.is_empty(),
        "a missing distributors.toml configures no distributor"
    );
    let project = work.path().join("Demo.snxprj");
    driver
        .send(&mut app, Message::File(FileMsg::NewProject(Some(project))))
        .expect("new project settled");
    drop(app);

    // A second run opens the project, dirties its sheet and saves it.
    let project = work.path().join("Demo").join("Demo.snxprj");
    let sheet = project.with_extension("snxsch");
    let (mut app, boot) = Signex::boot(vec![project.clone()]);
    driver.settle(&mut app, boot).expect("open settled");
    assert_eq!(app.document_state.projects.len(), 1, "the project opened");
    assert!(
        !app.ui_state
            .recent_projects
            .entries()
            .iter()
            .any(|entry| entry.ends_with("real-root-recent.snxprj")),
        "the real recent-projects list must not be read"
    );
    driver
        .send(
            &mut app,
            Message::File(FileMsg::Opened(Some(sheet.clone()))),
        )
        .expect("sheet open settled");
    assert!(
        app.document_state.tabs.iter().any(|tab| tab.path == sheet),
        "the sheet opened"
    );
    app.document_state.mark_document_dirty(&sheet);
    driver
        .send(&mut app, Message::Menu(MenuMessage::SaveAll))
        .expect("save settled");
    assert!(!app.document_state.is_document_dirty(&sheet), "saved");

    assert_credentials_unused();
    let isolated_prefs =
        std::fs::read_to_string(isolated.path().join("prefs.json")).expect("isolated prefs");
    assert!(
        isolated_prefs.contains("Demo.snxprj"),
        "writes land under the override: {isolated_prefs}"
    );
    assert!(
        !isolated_prefs.contains("paste_designators_schema"),
        "the paste migration must not run under an override: {isolated_prefs}"
    );
    assert_eq!(snapshot(&real), before, "the real config root changed");
}

/// The OS keyring is the user's real credential store: under an override a
/// distributor that keeps its credentials there is not configured.
fn assert_credentials_unused() {
    for (name, source) in [
        ("Mouser", DistributorSource::Mouser),
        ("DigiKey", DistributorSource::DigiKey),
    ] {
        match resolve_client(name, &[source]) {
            Err(ClientUnavailable::NotConfigured(reason)) => {
                assert!(reason.contains("keyring"), "{name}: {reason}")
            }
            Err(other) => panic!("{name}: the keyring was consulted: {other:?}"),
            Ok(_) => panic!("{name}: a client was built from the keyring"),
        }
    }
    // Pre-cancelled: without the refusal the flow stops as Cancelled.
    let cancel = digikey_oauth::CancelHandle::new();
    cancel.cancel();
    let connect = digikey_oauth::run_blocking(
        "client-id".into(),
        "client-secret".into(),
        "http://127.0.0.1:9/authorize".into(),
        "http://127.0.0.1:9/token".into(),
        cancel,
        false,
    );
    assert!(
        matches!(&connect, digikey_oauth::Outcome::Failed { reason } if reason.contains("keyring")),
        "DigiKey connect must not reach the keyring: {connect:?}"
    );
    let removed = digikey_oauth::remove_stored_token();
    assert!(
        removed
            .as_ref()
            .is_err_and(|reason| reason.contains("keyring")),
        "DigiKey token removal must not reach the keyring: {removed:?}"
    );
}

fn late() {
    assert!(config_root().is_some());
    let root = tempfile::tempdir().expect("override root");
    assert_eq!(
        set_config_root_override(root.path().to_path_buf()),
        Err(ConfigRootOverrideError::AfterFirstRead)
    );
    assert_eq!(config_root(), default_config_root(), "no override took");
}

/// Every config file a run would read or write, so a leak shows as a change.
fn seed_real_root(root: &Path) {
    let files: [(&str, &str); 5] = [
        (
            "prefs.json",
            r#"{"paste_designators":"Keep","recent_projects":["/elsewhere/real-root-recent.snxprj"]}"#,
        ),
        (
            "global_libraries.toml",
            "[[libraries]]\npath = \"/elsewhere/real-root-library\"\n",
        ),
        (
            "distributors.toml",
            "[distributor_apis]\npreferred_order = [\"mouser\"]\n",
        ),
        ("keyboard_shortcuts.toml", "# the user's shortcuts\n"),
        ("recovery/keep.txt", "a recovery copy\n"),
    ];
    for (name, body) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, body).expect("seed");
    }
}

/// Every file and folder under `root` with its bytes (`None` for a folder).
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                out.insert(path.clone(), None);
                stack.push(path);
            } else {
                out.insert(path.clone(), Some(std::fs::read(&path).expect("read")));
            }
        }
    }
    out
}
