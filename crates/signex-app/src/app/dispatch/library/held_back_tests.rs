//! #1729 review: a commit made through an adapter the app opens for one
//! job (a container save) names the pending files it had to leave out,
//! like a commit of the mounted adapter does.

use super::deferred_close_tests::dirty_symbol_in_library;
use super::*;

/// Unix only: the unreadable file is made with `chmod 000`.
#[cfg(unix)]
#[test]
fn a_container_save_past_an_unreadable_pending_file_names_it_in_messages() {
    use std::os::unix::fs::PermissionsExt;
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let symbols = f.file.parent().unwrap().join("symbols");
    std::fs::create_dir_all(&symbols).unwrap();
    let name = format!("bad-{}.snxsym", Uuid::now_v7());
    let bad = symbols.join(&name);
    std::fs::write(&bad, "unreadable").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&bad).is_ok() {
        // Running as root: permissions are not enforced.
        return;
    }
    let path = dirty_symbol_in_library(&mut f);

    let save = f.app.save_primitive_tab_at(&path).expect("save starts");
    run_task(&mut f.app, save);

    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|entry| entry
                .message
                .starts_with("Not committed, kept pending in library Lib: could not read")
                && entry.message.contains(&name)),
        "the app helper names the file the save's commit left out"
    );
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o644)).unwrap();
}
