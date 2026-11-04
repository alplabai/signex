//! A draft is saved, cascaded and refreshed through the DEEPEST library that
//! holds it, whatever order the libraries were mounted in (#1916).

use std::path::{Path, PathBuf};

use signex_library::RowId;
use uuid::Uuid;

use crate::app::Signex;
use crate::library::state::{LibraryDisplaySettings, OpenLibrary};

fn bare(root: &str) -> OpenLibrary {
    OpenLibrary {
        root: PathBuf::from(root),
        display_name: root.into(),
        library_id: Uuid::now_v7(),
        tables: std::collections::HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    }
}

fn app_with(roots: &[&str]) -> Signex {
    let (mut app, _task) = Signex::new();
    app.library.open_libraries = roots.iter().map(|r| bare(r)).collect();
    app
}

const OUTER: &str = "/p/lib.snxlib";
const INNER: &str = "/p/vendor/v.snxlib";
const NESTED_DRAFT: &str = "/p/vendor/symbols/a.snxsym";

fn both_orders() -> [[&'static str; 2]; 2] {
    [[OUTER, INNER], [INNER, OUTER]]
}

#[test]
fn the_post_save_refresh_targets_the_inner_library() {
    for order in both_orders() {
        let app = app_with(&order);
        assert_eq!(
            app.library_root_containing(Path::new(NESTED_DRAFT)),
            Some(PathBuf::from(INNER)),
            "{order:?}"
        );
        assert_eq!(
            app.library_root_containing(Path::new("/p/symbols/b.snxsym")),
            Some(PathBuf::from(OUTER)),
            "{order:?}"
        );
    }
}

#[test]
fn the_cascade_runs_in_the_inner_library() {
    let rows = [RowId::from_uuid(Uuid::now_v7())];
    for order in both_orders() {
        let app = app_with(&order);
        assert_eq!(
            app.cascade_root(Path::new(NESTED_DRAFT), &rows),
            Some(PathBuf::from(INNER)),
            "{order:?}"
        );
    }
}

#[test]
fn the_blocking_save_waits_on_the_inner_librarys_writes() {
    for order in both_orders() {
        let mut app = app_with(&order);
        let _running = app.library.writes.turn(Path::new(INNER));
        let error = app
            .save_primitive_tab_blocking(Path::new(NESTED_DRAFT))
            .expect_err("the inner library still has a write running");
        assert!(
            error
                .to_string()
                .contains("a library write is still running"),
            "{order:?}: {error}"
        );
    }
}

#[test]
fn libraries_sharing_a_folder_resolve_to_the_sorted_first_in_any_mount_order() {
    for order in [
        ["/p/b.snxlib", "/p/a.snxlib"],
        ["/p/a.snxlib", "/p/b.snxlib"],
    ] {
        let app = app_with(&order);
        assert_eq!(
            app.library_root_containing(Path::new("/p/symbols/s.snxsym")),
            Some(PathBuf::from("/p/a.snxlib")),
            "{order:?}"
        );
    }
}
