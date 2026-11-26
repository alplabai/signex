//! Libraries a mounted library's manifest `requires` (#3112). Once a batch of
//! mounts has settled, each one whose required library is not open is
//! reported in Messages and stays mounted; offering to mount it is #3114's
//! (A10) job.

use std::path::Path;

use signex_i18n::{Arg, I18n, ids};
use uuid::Uuid;

use super::state::LibraryState;

impl LibraryState {
    /// The library ids the library at `root` requires that no open library
    /// has, in manifest order. Empty when `root` is not open.
    pub fn missing_required_libraries(&self, root: &Path) -> Vec<Uuid> {
        let Some(lib) = self.library_at(root) else {
            return Vec::new();
        };
        lib.provenance
            .requires
            .iter()
            .copied()
            .filter(|id| {
                !self
                    .open_libraries
                    .iter()
                    .any(|open| open.library_id == *id)
            })
            .collect()
    }

    /// One Messages line per library required by a library mounted since
    /// the last call that no open library is, naming the library that needs
    /// it. Clears the list of libraries to check.
    pub fn missing_required_messages(&mut self, i18n: &I18n) -> Vec<String> {
        let roots = std::mem::take(&mut self.requires_unchecked);
        let mut lines = Vec::new();
        for root in roots {
            let name = self.library_label(&root);
            for id in self.missing_required_libraries(&root) {
                let required = id.to_string();
                lines.push(i18n.message_plain(
                    ids::LIBRARY_REQUIRES_MISSING,
                    &[
                        ("library", Arg::from(name.as_str())),
                        ("required", Arg::from(required.as_str())),
                    ],
                ));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::library::state::OpenLibrary;

    fn open(state: &mut LibraryState, root: &str, id: u128, requires: Vec<Uuid>) {
        state.open_libraries.push(OpenLibrary {
            root: PathBuf::from(root),
            display_name: root.into(),
            library_id: Uuid::from_u128(id),
            tables: HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
            provenance: signex_library::LibraryProvenance {
                requires,
                ..Default::default()
            },
        });
    }

    #[test]
    fn only_required_libraries_that_are_not_open_are_missing() {
        let mut state = LibraryState::default();
        open(
            &mut state,
            "/a/Parts.snxlib",
            1,
            vec![Uuid::from_u128(2), Uuid::from_u128(3)],
        );
        open(&mut state, "/a/Footprints.snxlib", 2, Vec::new());
        assert_eq!(
            state.missing_required_libraries(Path::new("/a/Parts.snxlib")),
            [Uuid::from_u128(3)]
        );
        assert!(
            state
                .missing_required_libraries(Path::new("/a/Footprints.snxlib"))
                .is_empty()
        );
        assert!(
            state
                .missing_required_libraries(Path::new("/nowhere.snxlib"))
                .is_empty()
        );
    }

    /// Checked once the batch settles: each library is reported once, by
    /// name.
    #[test]
    fn messages_name_the_library_and_are_given_once() {
        let mut state = LibraryState::default();
        open(
            &mut state,
            "/a/Parts.snxlib",
            1,
            vec![Uuid::from_u128(2), Uuid::from_u128(3)],
        );
        open(&mut state, "/a/Footprints.snxlib", 2, Vec::new());
        state.requires_unchecked = vec![PathBuf::from("/a/Parts.snxlib")];
        let i18n = I18n::en_us();
        let lines = state.missing_required_messages(&i18n);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("/a/Parts.snxlib"), "{}", lines[0]);
        assert!(
            lines[0].contains(&Uuid::from_u128(3).to_string()),
            "{}",
            lines[0]
        );
        assert!(state.missing_required_messages(&i18n).is_empty());
    }
}
