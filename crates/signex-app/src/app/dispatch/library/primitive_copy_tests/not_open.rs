//! #1869 and lib-10 — a symbol file that belongs to a library that is not
//! open: the picker groups it under that library, and the question offers
//! to link it (open the library, bind its symbol) or copy it.

use super::*;

/// A second, unmounted library `x.snxlib` holding `symbols/Held.snxsym`,
/// listed in the project like the loose file (#1869).
fn unmounted_library_symbol(f: &mut Fixture, dir: &Path) -> PathBuf {
    let snxlib = dir.join("vendor").join("x.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "x".into(),
            description: None,
            ..Default::default()
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    drop(
        LocalGitAdapter::init(
            &snxlib,
            manifest,
            LibraryInitOptions {
                enable_git: false,
                use_lfs: false,
            },
        )
        .expect("init"),
    );
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    let held = snxlib.parent().unwrap().join("symbols").join("Held.snxsym");
    std::fs::create_dir_all(held.parent().unwrap()).unwrap();
    std::fs::write(
        &held,
        SymbolFile::from_symbol(Symbol::empty("HELD"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    f.app.document_state.projects[0]
        .data
        .libraries
        .push(LibraryEntry {
            path: held,
            kind: LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        });
    snxlib
}

fn headers(f: &Fixture) -> Vec<String> {
    let picker = f.app.library.primitive_picker.as_ref().unwrap();
    crate::library::primitive_picker::header_labels(&f.app.library, picker)
}

/// Every listed row as `(group head, row words)`.
fn rows(f: &Fixture) -> Vec<(String, String)> {
    let picker = f.app.library.primitive_picker.as_ref().unwrap();
    crate::library::primitive_picker::row_labels(&f.app.library, picker)
}

/// Whether a row of `group` reads as a path ending in `file`.
fn listed_under(rows: &[(String, String)], group: &str, file: &str) -> bool {
    rows.iter()
        .any(|(g, words)| g == group && Path::new(words).ends_with(file))
}

fn chosen_held(f: &mut Fixture) -> crate::library::loose_symbols::LooseSymbol {
    let held = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "HELD")
        .cloned()
        .unwrap();
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(held.clone()));
    held
}

fn opening(f: &Fixture) -> Option<PathBuf> {
    f.app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .opening_library
        .clone()
}

#[test]
fn a_symbol_of_an_unmounted_library_is_grouped_under_it_and_offers_link_and_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);

    let listed = rows(&f);
    assert!(
        listed.contains(&(
            "In library x (not open)".to_string(),
            "Held.snxsym · HELD".to_string()
        )),
        "{listed:?}"
    );
    assert!(
        headers(&f).contains(&"In library x (not open) (1)".to_string()),
        "{:?}",
        headers(&f)
    );
    assert!(
        listed_under(&listed, "Not in a library", "SymbolLibrary.snxsym"),
        "the truly loose file stays under Not in a library: {listed:?}"
    );

    let held = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "HELD")
        .cloned()
        .unwrap();
    assert_eq!(held.library.as_deref(), Some(snxlib.as_path()));
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(held.clone()));
    // The question names the other library: it offers the link as well as
    // the copy (lib-10).
    let question = copy_question(&f).expect("question raised");
    assert_eq!(question.library.as_deref(), Some(snxlib.as_path()));

    // Link without copying: the library opens and the row binds its symbol.
    picker(&mut f.app, PrimitivePickerMsg::OpenLibraryConfirmed);

    let other = f
        .app
        .library
        .library_at(&snxlib)
        .expect("mounted")
        .library_id;
    assert_eq!(bound_symbol(&f), PrimitiveRef::new(other, held.uuid));
    assert!(
        f.app.library.primitive_picker.is_none(),
        "picker closed by the bind"
    );
    assert!(
        !f.address
            .library_path
            .parent()
            .unwrap()
            .join("symbols")
            .exists(),
        "nothing was copied into the row's library"
    );
    // Opened again, the symbol is no longer offered as not open.
    open_picker(&mut f);
    let labels = headers(&f);
    assert!(
        labels.iter().all(|l| !l.contains("not open")),
        "now listed under the open library: {labels:?}"
    );
}

/// lib-10: a symbol of a library that is not open can be copied into the
/// row's library instead of linked; that library is not opened.
#[test]
fn copying_from_a_not_open_library_copies_into_the_rows_library() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    let held = chosen_held(&mut f);

    picker(&mut f.app, PrimitivePickerMsg::CopyConfirmed);

    let bound = bound_symbol(&f);
    assert_eq!(bound.library_id, f.library_id, "bound to the row's library");
    assert_eq!(bound.uuid, held.uuid, "a free uuid is kept");
    let copy = f
        .app
        .library
        .primitive_file(&bound, "symbols", "snxsym")
        .expect("library is open");
    assert!(
        copy.is_file(),
        "the copy is a file of the library: {copy:?}"
    );
    assert!(f.app.library.library_at(&snxlib).is_none(), "not opened");
    assert!(f.app.library.primitive_picker.is_none(), "picker closed");
}

/// Link without copying, when the other library was opened while the
/// question was up, binds at once without waiting for a mount.
#[test]
fn linking_to_an_open_library_binds_at_once() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    let held = chosen_held(&mut f);
    f.app.library.open_library(snxlib.clone()).expect("mount");
    let other = f.app.library.library_at(&snxlib).unwrap().library_id;

    picker(&mut f.app, PrimitivePickerMsg::OpenLibraryConfirmed);

    assert_eq!(bound_symbol(&f), PrimitiveRef::new(other, held.uuid));
    assert!(f.app.library.primitive_picker.is_none());
}

#[test]
fn a_shared_symbol_file_outside_the_project_is_not_claimed_by_a_stray_snxlib_above_it() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let mut f = fixture(&project);
    // A `.snxlib` the file is not inside, higher up than the project.
    std::fs::write(tmp.path().join("stray.snxlib"), "").unwrap();
    let shared = tmp.path().join("shared").join("sym").join("Shared.snxsym");
    std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
    std::fs::write(
        &shared,
        SymbolFile::from_symbol(Symbol::empty("SHARED"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    f.app.document_state.projects[0]
        .data
        .libraries
        .push(LibraryEntry {
            path: shared,
            kind: LibraryEntryKind::Shared,
            library_id: None,
            unknown: Default::default(),
        });
    open_picker(&mut f);

    let listed = rows(&f);
    assert!(
        listed_under(&listed, "Not in a library", "Shared.snxsym"),
        "the shared file is loose: {listed:?}"
    );
    let picked = f.app.library.primitive_picker.as_ref().unwrap();
    assert!(picked.loose.iter().all(|s| s.library.is_none()));
}

#[test]
fn a_failed_open_of_the_symbols_library_shows_in_the_picker() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    let held = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "HELD")
        .cloned()
        .unwrap();
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(held));
    std::fs::remove_file(&snxlib).unwrap();
    picker(&mut f.app, PrimitivePickerMsg::OpenLibraryConfirmed);

    let open = f.app.library.primitive_picker.as_ref().expect("still open");
    assert!(open.opening_library.is_none());
    assert!(
        open.error
            .as_deref()
            .is_some_and(|e| e.starts_with("Could not open library x")),
        "{:?}",
        open.error
    );
}

#[test]
fn a_mount_cancelled_while_preparing_clears_the_opening_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    chosen_held(&mut f);
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib.clone());

    // The mount finishes after its request was withdrawn (library closed).
    let task = f
        .app
        .update(Message::Library(LibraryMessage::MountFinished {
            path: snxlib,
            prepared: crate::library::mount::PreparedMountCell::new(Err(
                signex_library::LibraryError::Backend("x".into()),
            )),
        }));
    settle(&mut f.app, task);

    assert_eq!(opening(&f), None);
    let open = f.app.library.primitive_picker.as_ref().unwrap();
    assert!(
        open.error
            .as_deref()
            .is_some_and(|e| e.contains("cancelled"))
    );
}

#[test]
fn cancel_while_opening_clears_the_opening_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    chosen_held(&mut f);
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib);
    picker(&mut f.app, PrimitivePickerMsg::CopyDeclined);
    assert_eq!(opening(&f), None);
}

#[test]
fn picking_another_symbol_while_opening_drops_the_opening_state_and_keeps_the_new_pick() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    chosen_held(&mut f);
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib.clone());
    let loose = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "GATEDRV8")
        .cloned()
        .unwrap();

    picker(&mut f.app, PrimitivePickerMsg::PickLoose(loose.clone()));
    assert_eq!(opening(&f), None);

    // The mount of the first library still lands: the new pick stays.
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib.clone());
    let before = bound_symbol(&f);
    let link = f.app.picker_link_after_mount(&snxlib);
    settle(&mut f.app, link);
    f.app.picker_mount_settled(&snxlib, None);
    let open = f.app.library.primitive_picker.as_ref().unwrap();
    assert_eq!(open.copy_confirm.as_ref(), Some(&loose));
    assert_eq!(bound_symbol(&f), before, "the newer pick is not linked");
}

/// Link without copying binds only a symbol the library lists. A file in
/// the library's folder that the library skipped (it did not parse, or its
/// uuid clashes) would leave the row on a Broken binding: the picker says
/// so instead and nothing is bound, whether the library was already open or
/// opens for the link.
#[test]
fn linking_a_held_file_the_library_does_not_list_binds_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    let held = chosen_held(&mut f);
    let before = bound_symbol(&f);
    f.app.library.open_library(snxlib.clone()).expect("mount x");
    f.app
        .library
        .library_at_mut(&snxlib)
        .expect("mounted")
        .cached_symbols
        .retain(|s| s.uuid != held.uuid);

    // Already open: the link is refused at once.
    picker(&mut f.app, PrimitivePickerMsg::OpenLibraryConfirmed);
    assert_eq!(bound_symbol(&f), before, "nothing bound");
    let open = f.app.library.primitive_picker.as_ref().expect("still open");
    let error = open.error.clone().expect("the reason is shown");
    assert!(
        error.contains("does not list") && error.contains("HELD"),
        "{error}"
    );

    // Opening for the link: the mount lands, the link is refused the same way.
    // (The mount request above may have read the library again.)
    f.app
        .library
        .library_at_mut(&snxlib)
        .expect("mounted")
        .cached_symbols
        .retain(|s| s.uuid != held.uuid);
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(held.clone()));
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib.clone());
    let link = f.app.picker_link_after_mount(&snxlib);
    settle(&mut f.app, link);
    f.app.picker_mount_settled(&snxlib, None);
    assert_eq!(bound_symbol(&f), before, "nothing bound after the mount");
    let open = f.app.library.primitive_picker.as_ref().expect("still open");
    assert!(
        open.error
            .as_deref()
            .is_some_and(|e| e.contains("does not list")),
        "{:?}",
        open.error
    );
}
