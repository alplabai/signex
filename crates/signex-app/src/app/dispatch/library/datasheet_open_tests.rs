//! Datasheet tab "Open" (#1954): it launches off the update thread, stays
//! available on a read-only library, and never edits the row.

use std::path::PathBuf;
use std::sync::Mutex;

use signex_library::{
    ComponentRow, DatasheetDoc, DatasheetRef, LibraryAdapter, LibraryError, Manifest, RowId,
};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::path_ops::settle_task;
use crate::library::component_preview::test_row;
use crate::library::messages::{DatasheetMsg, EditorMsg};
use crate::library::placed_part::DatasheetLocation;
use crate::library::state::{
    ComponentPreviewState, EditorAddress, LibraryDisplaySettings, OpenLibrary,
};
use crate::library::test_adapters::ScriptedSymbolsAdapter;

/// Every launch the recorder saw: the location and the part name it got.
/// The opener is a plain `fn`, so the record is a static; each test uses
/// its own part name and reads only its own entries.
static LAUNCHES: Mutex<Vec<(DatasheetLocation, String)>> = Mutex::new(Vec::new());

fn record(location: &DatasheetLocation, part: &str) -> Result<(), String> {
    LAUNCHES
        .lock()
        .expect("launch record")
        .push((location.clone(), part.to_string()));
    Ok(())
}

fn launches_for(part: &str) -> Vec<DatasheetLocation> {
    LAUNCHES
        .lock()
        .expect("launch record")
        .iter()
        .filter(|(_, p)| p == part)
        .map(|(l, _)| l.clone())
        .collect()
}

struct Shelf {
    manifest: Manifest,
    read_only: bool,
}

impl LibraryAdapter for Shelf {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(Vec::new())
    }
}

struct Fixture {
    app: Signex,
    address: EditorAddress,
    before: ComponentRow,
}

/// A mounted library with an open preview of a row whose documents are
/// `docs` and whose MPN is `part`.
fn fixture(part: &str, read_only: bool, docs: Vec<DatasheetDoc>) -> Fixture {
    let library_id = Uuid::now_v7();
    let root = PathBuf::from(format!("/tmp/datasheet-open/{part}/Lib.snxlib"));
    let mut row = test_row();
    row.primary_mpn.mpn = part.to_string();
    row.set_documents(docs);
    let address = EditorAddress::new(root.clone(), "R".into(), RowId::from_uuid(row.row_id));
    let manifest = ScriptedSymbolsAdapter::new(library_id).0.manifest().clone();
    let (mut app, _task) = Signex::new();
    app.library.open_libraries.clear();
    app.library
        .set
        .mount(Box::new(Shelf {
            manifest,
            read_only,
        }))
        .expect("mount");
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Lib".into(),
        library_id,
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
    });
    app.library.datasheet_opener = record;
    app.library.editors.insert(
        address.clone(),
        ComponentPreviewState::from_row(root, "R".into(), row.clone()),
    );
    Fixture {
        app,
        address,
        before: row,
    }
}

impl Fixture {
    fn open(&mut self, index: usize) {
        let task = self.app.handle_editor_event(
            self.address.clone(),
            EditorMsg::Datasheet(DatasheetMsg::Open(index)),
        );
        settle_task(&mut self.app, task);
    }

    fn assert_untouched(&self) {
        let state = &self.app.library.editors[&self.address];
        assert!(!state.dirty && !state.has_unsaved_edits());
        assert_eq!(state.row, self.before, "the draft row is unchanged");
        assert!(self.app.library.dirty_editors().is_empty());
        assert_eq!(self.app.document_state.dirty_document_count(), 0);
        assert!(self.app.document_state.error_notice.is_none());
    }
}

fn link(url: &str) -> DatasheetDoc {
    DatasheetDoc {
        label: "Datasheet".into(),
        source: DatasheetRef::url(url),
    }
}

fn install_logger() {
    use crate::ignore::IgnoreResult;
    crate::diagnostics::init_logging()
        .ignore("another test in this binary already installed the same logger");
}

#[test]
fn open_launches_a_link_on_a_read_only_library_and_edits_nothing() {
    let part = "OPEN-RO-1";
    let mut f = fixture(part, true, vec![link("https://x.test/d.pdf")]);

    f.open(0);

    assert_eq!(
        launches_for(part),
        vec![DatasheetLocation::Url("https://x.test/d.pdf".into())]
    );
    f.assert_untouched();
}

#[test]
fn open_launches_a_link_on_an_editable_library_and_edits_nothing() {
    let part = "OPEN-RW-1";
    let mut f = fixture(part, false, vec![link("http://x.test/d.pdf")]);

    f.open(0);

    assert_eq!(launches_for(part).len(), 1);
    f.assert_untouched();
}

#[test]
fn a_link_with_a_bad_scheme_is_logged_and_never_launched() {
    install_logger();
    let part = "OPEN-BAD-1";
    let mut f = fixture(part, false, vec![link("file:///etc/passwd")]);
    let newest = crate::diagnostics::newest_entry_id().unwrap_or(0);

    f.open(0);

    assert!(launches_for(part).is_empty());
    let warned = crate::diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > newest && e.message.contains(part) && e.message.contains("https://"));
    assert!(warned, "the refusal reaches the Messages panel");
    f.assert_untouched();
}

#[test]
fn an_index_past_the_last_document_does_nothing() {
    let part = "OPEN-IDX-1";
    let mut f = fixture(part, true, vec![link("https://x.test/d.pdf")]);

    f.open(5);

    assert!(launches_for(part).is_empty());
    f.assert_untouched();
}

fn pinned(hash: &str) -> DatasheetDoc {
    DatasheetDoc {
        label: "Datasheet".into(),
        source: DatasheetRef::HashPinned {
            hash: hash.to_string(),
            filename: "d.pdf".into(),
        },
    }
}

#[test]
fn a_pinned_pdf_that_is_only_staged_asks_to_save_first() {
    install_logger();
    let part = "OPEN-STAGED-1";
    let hash = "a".repeat(64);
    let mut f = fixture(part, false, vec![pinned(&hash)]);
    f.app
        .library
        .editors
        .get_mut(&f.address)
        .expect("editor")
        .staged_datasheets
        .insert(
            hash,
            signex_library::PdfAsset::from_bytes(b"%PDF-1.7 staged".to_vec()).expect("pdf"),
        );
    let newest = crate::diagnostics::newest_entry_id().unwrap_or(0);

    f.open(0);

    assert!(launches_for(part).is_empty());
    let advised = crate::diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > newest && e.message.contains(part) && e.message.contains("Save"));
    assert!(advised, "the advice says to save first");
}

#[test]
fn a_stored_pinned_pdf_launches_its_library_file() {
    let part = "OPEN-PIN-1";
    let mut f = fixture(part, true, vec![pinned(&"b".repeat(64))]);

    f.open(0);

    assert!(matches!(
        launches_for(part).as_slice(),
        [DatasheetLocation::Pinned { .. }]
    ));
}

#[test]
fn advice_names_the_part_by_mpn_then_internal_pn_then_a_fallback() {
    use super::datasheet_save::part_name;
    let mut row = test_row();
    row.primary_mpn.mpn = "  LM358  ".into();
    assert_eq!(part_name(&row), "LM358");

    row.primary_mpn.mpn = "  ".into();
    row.internal_pn = signex_library::InternalPn::new("R0805_10k");
    assert_eq!(part_name(&row), "R0805_10k");

    row.internal_pn = signex_library::InternalPn::new(" ");
    assert_eq!(part_name(&row), "this component");
}
