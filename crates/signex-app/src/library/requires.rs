//! Libraries a mounted library's manifest `requires` (#3112). Mounting one
//! whose required library is not open reports it in Messages and carries on;
//! offering to mount it is #3114's (A10) job.

use std::path::Path;

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
}

/// The Messages line for a library that needs library `missing`.
pub fn missing_required_message(library: &str, missing: Uuid) -> String {
    format!(
        "Library {library} needs library {missing}, which is not open; its parts that use it will not resolve"
    )
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
}
