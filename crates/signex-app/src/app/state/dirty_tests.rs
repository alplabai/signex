//! The dirty-state owner (`dirty.rs`): one stored flag per document, the
//! rest derived.

use std::path::Path;

use crate::app::{DocumentState, FootprintDocument, Signex, SymbolDocument};

fn state() -> DocumentState {
    Signex::new().0.document_state
}

fn with_symbol(path: &Path) -> DocumentState {
    let mut d = state();
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
    d.symbol_editors.insert(
        path.to_path_buf(),
        SymbolDocument::new(path.to_path_buf(), file),
    );
    d
}

fn with_footprint(path: &Path) -> DocumentState {
    let mut d = state();
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("F"));
    d.footprint_editors.insert(
        path.to_path_buf(),
        FootprintDocument::new(path.to_path_buf(), file),
    );
    d
}

#[test]
fn a_document_without_a_draft_is_marked_in_the_set() {
    let mut d = state();
    let sheet = Path::new("/p/a.snxsch");
    d.mark_document_dirty(sheet);
    assert!(d.is_document_dirty(sheet));
    assert_eq!(d.dirty_document_count(), 1);
    d.mark_document_clean(sheet);
    assert!(!d.is_document_dirty(sheet));
    assert!(!d.has_dirty_documents());
}

#[test]
fn a_draft_takes_the_flag_and_clean_clears_it() {
    let sym = Path::new("/p/a.snxsym");
    let mut d = with_symbol(sym);
    d.mark_document_dirty(sym);
    assert!(d.symbol_editors[sym].is_dirty());
    assert!(d.is_document_dirty(sym));
    d.mark_document_clean(sym);
    assert!(!d.symbol_editors[sym].is_dirty());
    assert!(!d.is_document_dirty(sym));

    let fp = Path::new("/p/a.snxfpt");
    let mut d = with_footprint(fp);
    d.mark_document_dirty(fp);
    assert!(d.footprint_editors[fp].is_dirty());
    d.mark_document_clean(fp);
    assert!(!d.is_document_dirty(fp));
}

#[test]
fn an_editors_own_edit_is_visible_without_any_mirror() {
    let fp = Path::new("/p/a.snxfpt");
    let mut d = with_footprint(fp);
    d.footprint_editors.get_mut(fp).unwrap().mark_dirty();
    assert!(d.is_document_dirty(fp));
    assert_eq!(d.dirty_document_count(), 1);
    assert!(d.dirty_document_paths().contains(fp));
    d.footprint_editors.get_mut(fp).unwrap().set_dirty(false);
    assert!(!d.is_document_dirty(fp));
    assert!(!d.has_dirty_documents());
}

#[test]
fn a_save_in_flight_keeps_a_draft_dirty_until_it_lands() {
    let sym = Path::new("/p/a.snxsym");
    let mut d = with_symbol(sym);
    d.mark_document_dirty(sym);
    // The snapshot resets the edit flag and holds the mark.
    d.symbol_editors.get_mut(sym).unwrap().set_dirty(false);
    d.keep_document_dirty_until_saved(sym);
    assert!(d.is_document_dirty(sym));
    d.mark_document_clean(sym);
    assert!(!d.is_document_dirty(sym));
}

#[test]
fn a_dirty_document_is_counted_once_however_many_places_know_it() {
    let sym = Path::new("/p/a.snxsym");
    let mut d = with_symbol(sym);
    d.mark_document_dirty(sym);
    d.keep_document_dirty_until_saved(sym);
    d.mark_document_dirty(Path::new("/p/b.snxsch"));
    assert_eq!(d.dirty_document_count(), 2);
}

#[test]
fn rekey_moves_the_stored_mark_and_leaves_clean_documents_clean() {
    let mut d = state();
    let (from, to) = (Path::new("/p/a.snxsch"), Path::new("/p/b.snxsch"));
    assert!(!d.rekey_document_dirty(from, to));
    assert!(!d.has_dirty_documents());
    d.mark_document_dirty(from);
    assert!(d.rekey_document_dirty(from, to));
    assert!(!d.is_document_dirty(from));
    assert!(d.is_document_dirty(to));
}

#[test]
fn clean_where_clears_every_match_across_set_and_drafts() {
    let sym = Path::new("/p/lib/a.snxsym");
    let mut d = with_symbol(sym);
    d.mark_document_dirty(sym);
    d.mark_document_dirty(Path::new("/p/lib/b.snxsch"));
    d.mark_document_dirty(Path::new("/p/other.snxsch"));
    d.mark_documents_clean_where(|p| p.starts_with("/p/lib"));
    assert_eq!(
        d.dirty_document_paths(),
        [Path::new("/p/other.snxsch").to_path_buf()].into()
    );
}
