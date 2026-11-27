//! The Libraries page's Version control column (#2591) and row ⋯ menu
//! button (#2592), on the fixture of `tests.rs`.

use std::path::PathBuf;

use signex_library::library_vcs::{LibraryVcsState, VcsAttention};

use super::tests::{Fixture, MOCK_WINDOW, plain, sent};
use super::*;
use crate::library::state::ComponentsMountSource;

fn with_vcs(name: &str, scope: ComponentsMountSource, vcs: LibraryVcsState) -> LoadedLibrary {
    LoadedLibrary {
        vcs: Some(vcs),
        ..libraries_page::sample(name, scope, 1)
    }
}

/// Decided 2026-11-27: Stock reads `—`, a clean repository a `git` pill, a
/// folder without one `off`, and a repository with a problem "needs
/// attention" over its reason, inline.
#[test]
fn every_row_shows_its_version_control_state() {
    let mut fx = Fixture::new(&[]).on(StartPage::Libraries);
    fx.libraries = vec![
        libraries_page::sample("Stock", ComponentsMountSource::Stock, 34),
        with_vcs(
            "tracked",
            ComponentsMountSource::Global,
            LibraryVcsState::Git,
        ),
        with_vcs("plain", ComponentsMountSource::Global, LibraryVcsState::Off),
        with_vcs(
            "dirty",
            ComponentsMountSource::Installed,
            LibraryVcsState::NeedsAttention(VcsAttention::Uncommitted(1)),
        ),
        with_vcs(
            "no-lfs",
            ComponentsMountSource::Global,
            LibraryVcsState::NeedsAttention(VcsAttention::LfsNotFound),
        ),
    ];
    let texts: Vec<String> = fx
        .texts(MOCK_WINDOW)
        .iter()
        .map(|t| plain(&t.text))
        .collect();
    for label in [
        "VERSION CONTROL",
        "—",
        "git",
        "off",
        "1 uncommitted change",
        "uses Git LFS, but git-lfs is not on PATH",
    ] {
        assert!(texts.iter().any(|t| t == label), "{label}: {texts:#?}");
    }
    let attention = texts.iter().filter(|t| *t == "needs attention").count();
    assert_eq!(attention, 2, "{texts:#?}");
}

/// A row whose state is not read yet draws nothing in the column rather
/// than a guess.
#[test]
fn a_row_not_read_yet_shows_no_state() {
    let mut fx = Fixture::new(&[]).on(StartPage::Libraries);
    fx.libraries = vec![libraries_page::sample(
        "fresh",
        ComponentsMountSource::Global,
        3,
    )];
    let texts = fx.texts(MOCK_WINDOW);
    for label in ["git", "off", "needs attention"] {
        assert!(texts.iter().all(|t| plain(&t.text) != label), "{label}");
    }
}

/// Decided 2026-11-27: "off" is a link that opens Enable Version Control for
/// that library.
#[test]
fn off_opens_enable_version_control_for_its_library() {
    let mut fx = Fixture::new(&[]).on(StartPage::Libraries);
    fx.libraries = vec![with_vcs(
        "plain",
        ComponentsMountSource::Global,
        LibraryVcsState::Off,
    )];
    let sent_messages = fx.click(MOCK_WINDOW, "off");
    assert!(
        sent(
            &sent_messages,
            &StartScreenMsg::LibraryVcsEnableClicked(PathBuf::from("plain.snxlib"))
        ),
        "{sent_messages:?}"
    );
}

/// Decided 2026-11-27: Global and Installed rows have a ⋯ menu (Remove from
/// Signex); Stock has none.
#[test]
fn global_and_installed_rows_have_a_menu_button_and_stock_has_none() {
    let mut fx = Fixture::new(&[]).on(StartPage::Libraries);
    fx.libraries = vec![libraries_page::sample(
        "Stock",
        ComponentsMountSource::Stock,
        34,
    )];
    assert!(
        fx.texts(MOCK_WINDOW).iter().all(|t| t.text != "⋯"),
        "Stock is not removable"
    );
    for scope in [
        ComponentsMountSource::Global,
        ComponentsMountSource::Installed,
    ] {
        fx.libraries = vec![libraries_page::sample("mine", scope, 1)];
        let pressed = fx.click(MOCK_WINDOW, "⋯");
        assert!(
            sent(
                &pressed,
                &StartScreenMsg::LibraryMenuOpened(PathBuf::from("mine.snxlib"))
            ),
            "{scope:?}: {pressed:?}"
        );
    }
    assert!(!libraries_page::is_removable(ComponentsMountSource::Stock));
    assert!(!libraries_page::is_removable(
        ComponentsMountSource::Project
    ));
}

/// The state is looked up by the folder holding the `.snxlib`, the folder
/// Enable Version Control scopes.
#[test]
fn the_state_is_keyed_by_the_folder_holding_the_library_file() {
    let file = PathBuf::from("libs")
        .join("passives")
        .join("passives.snxlib");
    assert_eq!(
        library_folder(&file),
        Some(PathBuf::from("libs").join("passives").as_path())
    );
}
