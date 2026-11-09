//! The flat library list: source chip order, library order, and the badge and
//! count each library row carries. Replaces the four-section order test.

use super::tests::{fixture_row, library_with};
use super::*;

/// One library per source, mounted in a scrambled order (global, installed
/// A, project, stock, installed B) with 1, 2, 3, 4 and 5 components, so a
/// dropped sort or a wrong count shows.
fn every_source() -> (LibraryState, Vec<PathBuf>) {
    let mut state = LibraryState::default();
    for (name, rows) in [("gl", 1), ("ia", 2), ("pr", 3), ("st", 4), ("ib", 5)] {
        let mut one = library_with((0..rows).map(|_| fixture_row("PN", "", "Mfr")).collect());
        let mut lib = one.open_libraries.remove(0);
        lib.root = PathBuf::from(format!("/tmp/{name}.snxlib"));
        lib.display_name = name.into();
        state.open_libraries.push(lib);
    }
    state.stock_library_root = Some(PathBuf::from("/tmp/st.snxlib"));
    state.installed_libraries = vec![
        PathBuf::from("/tmp/ia.snxlib"),
        PathBuf::from("/tmp/ib.snxlib"),
    ];
    state.global_libraries = vec![global_prefs::GlobalLibraryEntry {
        path: PathBuf::from("/tmp/gl.snxlib"),
        remote: None,
        auto_pull: None,
    }];
    (state, vec![PathBuf::from("/tmp/pr.snxlib")])
}

#[test]
fn chips_and_libraries_follow_the_source_order_whatever_the_mount_order() {
    let (mut state, project) = every_source();
    let listed = listed_libraries(&state, &project);
    let names: Vec<_> = listed
        .iter()
        .map(|(_, l)| l.display_name.as_str())
        .collect();
    assert_eq!(names, ["st", "pr", "ia", "ib", "gl"]);

    use ComponentsMountSource::{Global, Installed, Project, Stock};
    assert_eq!(
        SourceCounts::of(&listed).chips(),
        [
            None,
            Some(Stock),
            Some(Project),
            Some(Installed),
            Some(Global)
        ]
    );

    // A source with no library has no chip; All stays.
    state.open_libraries.retain(|l| l.display_name != "pr");
    let listed = listed_libraries(&state, &project);
    assert_eq!(
        SourceCounts::of(&listed).chips(),
        [None, Some(Stock), Some(Installed), Some(Global)]
    );
}

#[test]
fn each_library_row_shows_its_source_badge_then_its_component_count() {
    use signex_types::theme::{ThemeId, theme_tokens};
    let (mut state, project) = every_source();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let window = iced::Size::new(400.0, 200.0);
    for lib in &state.open_libraries {
        state
            .components_panel
            .collapsed_libraries
            .insert(lib.root.clone());
    }
    let expected = [
        ("st", "STOCK", "4"),
        ("pr", "PROJECT", "3"),
        ("ia", "INSTALLED", "2"),
        ("ib", "INSTALLED", "5"),
        ("gl", "GLOBAL", "1"),
    ];
    let listed = listed_libraries(&state, &project);
    assert_eq!(listed.len(), expected.len());
    for ((source, lib), (name, badge, count)) in listed.into_iter().zip(expected) {
        assert_eq!(lib.display_name, name);
        let header = view_library_block(
            lib,
            source,
            &state.components_panel,
            "",
            &tokens,
            &signex_i18n::I18n::en_us(),
        );
        let texts = crate::layout_probe::lay_out_texts(header, window);
        let at = |label| crate::layout_probe::find(&texts, label).bounds;
        let (n, b, c) = (at(name), at(badge), at(count));
        assert!(n.x < b.x && b.x < c.x, "{name}: name, badge, count order");
    }
}
