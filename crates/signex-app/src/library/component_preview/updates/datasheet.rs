//! Datasheet edits for a Component Preview row.
//!
//! Owns the Datasheet tab's actions over the row's document list
//! (`row.documents()`, primary first, #1231): switching a document
//! between URL and pinned-PDF modes, live URL and label entry, applying
//! an upload result by pinning the picked PDF and staging its bytes (they are
//! written on Save, #1332), and adding,
//! removing and reordering documents.

use signex_library::{DatasheetDoc, DatasheetRef};

use crate::library::ComponentPreviewState;
use crate::library::editor::datasheet_picker::DatasheetMode;
use crate::library::messages::{DatasheetMsg, PickedFile, PickedPdf};

/// Apply one Datasheet-tab message to the row. Opening the upload dialog
/// is the dispatcher's (it starts a task) and changes nothing here.
pub(super) fn apply(state: &mut ComponentPreviewState, msg: DatasheetMsg) {
    match msg {
        DatasheetMsg::SetMode(index, mode) => set_mode(state, index, mode),
        DatasheetMsg::SetUrl(index, url) => set_url(state, index, url),
        DatasheetMsg::SetLabel(index, label) => set_label(state, index, label),
        // The dispatcher's: the dialog and the save write are tasks.
        DatasheetMsg::UploadDialog(_) | DatasheetMsg::AssetsStored(_) => {}
        DatasheetMsg::UploadResult(picked) => {
            let PickedPdf {
                index,
                picked_for,
                file,
            } = *picked;
            apply_upload_result(state, index, &picked_for, file);
        }
        DatasheetMsg::Add => {
            state.datasheet_errors.clear();
            add(state);
        }
        DatasheetMsg::Remove(index) => {
            state.datasheet_errors.clear();
            remove(state, index);
        }
        DatasheetMsg::Move { from, to } => {
            state.datasheet_errors.clear();
            move_to(state, from, to);
        }
    }
}

/// Apply `edit` to document `index` and write the list back. An index
/// past the end (a document removed while its upload dialog was open) is
/// ignored. The row is marked dirty only when the list changed.
fn edit_documents(state: &mut ComponentPreviewState, edit: impl FnOnce(&mut Vec<DatasheetDoc>)) {
    let before = state.row.documents();
    let mut docs = before.clone();
    edit(&mut docs);
    if docs != before {
        state.row.set_documents(docs);
        state.dirty = true;
    }
}

fn edit_document(
    state: &mut ComponentPreviewState,
    index: usize,
    edit: impl FnOnce(&mut DatasheetDoc),
) {
    edit_documents(state, |docs| {
        if let Some(doc) = docs.get_mut(index) {
            edit(doc);
        }
    });
}

/// Switch a document between URL and pinned-PDF modes.
///
/// Resets the reference to a fresh default of the chosen kind only when
/// the current value is of the other kind, so re-selecting the active
/// mode is a no-op and does not clobber the existing value.
pub(super) fn set_mode(state: &mut ComponentPreviewState, index: usize, mode: DatasheetMode) {
    state.datasheet_errors.remove(&index);
    edit_document(state, index, |doc| match (mode, &doc.source) {
        (DatasheetMode::Url, DatasheetRef::Url { .. })
        | (DatasheetMode::PinnedPdf, DatasheetRef::HashPinned { .. }) => {}
        (DatasheetMode::Url, _) => doc.source = DatasheetRef::default(),
        (DatasheetMode::PinnedPdf, _) => doc.source = DatasheetRef::hash_pinned("", ""),
    });
}

/// Set a document to a URL reference, clearing it to the default when
/// the field is emptied.
pub(super) fn set_url(state: &mut ComponentPreviewState, index: usize, url: String) {
    let trimmed = url.trim();
    edit_document(state, index, |doc| {
        doc.source = if trimmed.is_empty() {
            DatasheetRef::default()
        } else {
            DatasheetRef::url(trimmed)
        };
    });
}

/// Set a document's label. Surrounding whitespace is kept while typing
/// and trimmed only when the label is nothing but whitespace.
pub(super) fn set_label(state: &mut ComponentPreviewState, index: usize, label: String) {
    let label = if label.trim().is_empty() {
        String::new()
    } else {
        label
    };
    edit_document(state, index, |doc| doc.label = label);
}

/// Apply the result of a pinned-PDF upload: pin the document the dialog
/// was opened for to the picked PDF and stage its bytes on the editor. The
/// bytes are written to the library only when the row is saved. The list
/// may have changed while the dialog was open, so that document is found
/// again: at `index` if it is still there, else wherever it moved. A
/// document that was removed or edited meanwhile takes nothing (neither a
/// pin nor an error), and the pick is reported as dropped. A cancelled pick
/// (`None`) leaves the row untouched and clears the document's error; a
/// refused file (not a PDF, unreadable) records the reason for the
/// document instead of pinning.
pub(super) fn apply_upload_result(
    state: &mut ComponentPreviewState,
    index: usize,
    picked_for: &DatasheetDoc,
    file: Option<Result<PickedFile, String>>,
) {
    let Some(file) = file else {
        state.datasheet_errors.remove(&index);
        return;
    };
    let docs = state.row.documents();
    let target = if docs.get(index) == Some(picked_for) {
        Some(index)
    } else {
        docs.iter().position(|doc| doc == picked_for)
    };
    let Some(target) = target else {
        tracing::warn!(
            target: "signex::library",
            "the picked PDF was not pinned: the document it was picked for was removed \
             or changed while the file dialog was open"
        );
        return;
    };
    match file {
        Err(reason) => {
            state.datasheet_errors.insert(target, reason);
        }
        Ok(PickedFile { asset, filename }) => {
            state.datasheet_errors.remove(&target);
            let hash = asset.hash.clone();
            state.staged_datasheets.insert(hash.clone(), asset);
            edit_document(state, target, |doc| {
                doc.source = DatasheetRef::hash_pinned(hash, filename);
            });
            // A replaced pick is spent: only PDFs the row pins stay staged.
            let in_use = state.staged_assets_in_use();
            state
                .staged_datasheets
                .retain(|hash, _| in_use.iter().any(|a| &a.hash == hash));
        }
    }
}

/// Append an empty URL document after the last one.
pub(super) fn add(state: &mut ComponentPreviewState) {
    edit_documents(state, |docs| docs.push(DatasheetDoc::default()));
}

/// Remove a document. Removing the primary promotes the next document;
/// removing the only one leaves the row with no datasheet.
pub(super) fn remove(state: &mut ComponentPreviewState, index: usize) {
    edit_documents(state, |docs| {
        if index < docs.len() {
            docs.remove(index);
        }
    });
}

/// Move a document from `from` to `to`; `to == 0` makes it the primary.
pub(super) fn move_to(state: &mut ComponentPreviewState, from: usize, to: usize) {
    edit_documents(state, |docs| {
        if from < docs.len() && to < docs.len() {
            let doc = docs.remove(from);
            docs.insert(to, doc);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> ComponentPreviewState {
        let mut row = crate::library::component_preview::test_row();
        row.datasheet = DatasheetRef::url("https://ti.example/u1.pdf");
        ComponentPreviewState::from_row(PathBuf::from("/tmp/test.snxlib"), "ics".into(), row)
    }

    fn urls(state: &ComponentPreviewState) -> Vec<(String, String)> {
        state
            .row
            .documents()
            .into_iter()
            .map(|d| {
                let url = match d.source {
                    DatasheetRef::Url { url } => url,
                    DatasheetRef::HashPinned { filename, .. } => filename,
                };
                (d.label, url)
            })
            .collect()
    }

    #[test]
    fn added_documents_are_edited_by_index_and_the_primary_stays_first() {
        let mut state = fixture();
        add(&mut state);
        set_url(&mut state, 1, "https://ti.example/errata.pdf".into());
        set_label(&mut state, 1, "Errata".into());
        set_label(&mut state, 0, "TI".into());
        assert!(state.dirty);
        assert_eq!(
            urls(&state),
            vec![
                ("TI".into(), "https://ti.example/u1.pdf".into()),
                ("Errata".into(), "https://ti.example/errata.pdf".into()),
            ]
        );
        assert_eq!(state.row.datasheet_label, "TI");
        assert_eq!(state.row.more_datasheets.len(), 1);
    }

    #[test]
    fn making_a_document_primary_moves_it_with_its_label() {
        let mut state = fixture();
        add(&mut state);
        set_url(&mut state, 1, "https://st.example/u1.pdf".into());
        set_label(&mut state, 1, "ST".into());
        move_to(&mut state, 1, 0);
        assert_eq!(
            state.row.datasheet,
            DatasheetRef::url("https://st.example/u1.pdf")
        );
        assert_eq!(state.row.datasheet_label, "ST");
        assert_eq!(
            urls(&state)[1],
            (String::new(), "https://ti.example/u1.pdf".into())
        );
    }

    #[test]
    fn removing_the_primary_promotes_the_next_and_removing_the_last_clears_it() {
        let mut state = fixture();
        add(&mut state);
        set_url(&mut state, 1, "https://st.example/u1.pdf".into());
        remove(&mut state, 0);
        assert_eq!(
            urls(&state),
            vec![(String::new(), "https://st.example/u1.pdf".into())]
        );
        remove(&mut state, 0);
        assert!(state.row.datasheet.is_empty());
        assert!(state.row.more_datasheets.is_empty());
    }

    fn picked(bytes: &[u8], name: &str) -> PickedFile {
        PickedFile {
            asset: signex_library::PdfAsset::from_bytes(bytes.to_vec()).unwrap(),
            filename: name.into(),
        }
    }

    fn pdf(name: &str) -> Option<Result<PickedFile, String>> {
        Some(Ok(picked(b"%PDF-1.4 datasheet", name)))
    }

    fn refused() -> Option<Result<PickedFile, String>> {
        Some(Err("this file is not a PDF".into()))
    }

    #[test]
    fn a_pick_stages_the_bytes_under_their_hash_and_pins_the_row() {
        let mut state = fixture();
        set_mode(&mut state, 0, DatasheetMode::PinnedPdf);
        let doc = state.row.documents()[0].clone();
        apply_upload_result(&mut state, 0, &doc, pdf("ds.pdf"));
        let DatasheetRef::HashPinned { hash, filename } = state.row.datasheet.clone() else {
            panic!("not pinned");
        };
        assert_eq!(filename, "ds.pdf");
        assert_eq!(
            state.staged_datasheets[&hash].bytes.as_ref(),
            b"%PDF-1.4 datasheet"
        );
        assert_eq!(state.staged_assets_in_use().len(), 1);
        assert!(state.dirty);
    }

    #[test]
    fn a_replaced_pick_is_no_longer_in_use() {
        let mut state = fixture();
        set_mode(&mut state, 0, DatasheetMode::PinnedPdf);
        let doc = state.row.documents()[0].clone();
        apply_upload_result(&mut state, 0, &doc, pdf("a.pdf"));
        let doc = state.row.documents()[0].clone();
        let second = Some(Ok(picked(b"%PDF-1.4 second", "b.pdf")));
        apply_upload_result(&mut state, 0, &doc, second);
        assert_eq!(
            state.staged_datasheets.len(),
            1,
            "the replaced pick is pruned"
        );
        let used = state.staged_assets_in_use();
        assert_eq!(used.len(), 1);
        assert_eq!(used[0].bytes.as_ref(), b"%PDF-1.4 second");
        set_url(&mut state, 0, "https://x.example/d.pdf".into());
        assert!(state.staged_assets_in_use().is_empty());
    }

    #[test]
    fn a_refused_file_pins_nothing_and_its_error_clears_on_cancel_and_mode_change() {
        let mut state = fixture();
        set_mode(&mut state, 0, DatasheetMode::PinnedPdf);
        let doc = state.row.documents()[0].clone();
        state.dirty = false;
        apply_upload_result(&mut state, 0, &doc, refused());
        assert_eq!(state.datasheet_errors[&0], "this file is not a PDF");
        assert!(state.staged_datasheets.is_empty() && !state.dirty);
        assert_eq!(state.row.documents()[0], doc);

        apply_upload_result(&mut state, 0, &doc, None);
        assert!(state.datasheet_errors.is_empty(), "cancel clears");

        apply_upload_result(&mut state, 0, &doc, refused());
        set_mode(&mut state, 0, DatasheetMode::Url);
        assert!(state.datasheet_errors.is_empty(), "mode change clears");

        let current = state.row.documents()[0].clone();
        apply_upload_result(&mut state, 0, &current, refused());
        apply(&mut state, DatasheetMsg::Add);
        assert!(state.datasheet_errors.is_empty(), "reshaping clears");
    }

    #[test]
    fn a_good_pick_clears_the_documents_error() {
        let mut state = fixture();
        set_mode(&mut state, 0, DatasheetMode::PinnedPdf);
        let doc = state.row.documents()[0].clone();
        apply_upload_result(&mut state, 0, &doc, refused());
        apply_upload_result(&mut state, 0, &doc, pdf("ds.pdf"));
        assert!(state.datasheet_errors.is_empty());
    }

    #[test]
    fn a_refused_file_for_a_removed_document_records_nothing() {
        let mut state = fixture();
        add(&mut state);
        set_label(&mut state, 1, "Errata".into());
        let errata = state.row.documents()[1].clone();
        remove(&mut state, 1);
        apply_upload_result(&mut state, 1, &errata, refused());
        assert!(state.datasheet_errors.is_empty());
    }

    #[test]
    fn an_upload_for_a_removed_document_changes_nothing() {
        let mut state = fixture();
        add(&mut state);
        set_label(&mut state, 1, "Errata".into());
        let errata = state.row.documents()[1].clone();
        remove(&mut state, 1);
        state.dirty = false;
        let before = state.row.clone();
        apply_upload_result(&mut state, 1, &errata, pdf("x.pdf"));
        apply_upload_result(&mut state, 7, &errata, pdf("x.pdf"));
        move_to(&mut state, 0, 4);
        assert_eq!(state.row, before);
        assert!(!state.dirty);
    }

    /// [TI, Errata, App note]: the dialog opens for Errata, TI is removed
    /// while it is open. The PDF pins Errata (now at 0), not App note
    /// (now at 1) (#1239 review).
    #[test]
    fn an_upload_follows_its_document_when_the_list_shifts() {
        let mut state = fixture();
        add(&mut state);
        add(&mut state);
        set_label(&mut state, 1, "Errata".into());
        set_mode(&mut state, 1, DatasheetMode::PinnedPdf);
        set_label(&mut state, 2, "App note".into());
        set_url(&mut state, 2, "https://ti.example/an.pdf".into());
        let errata = state.row.documents()[1].clone();

        remove(&mut state, 0);
        apply_upload_result(&mut state, 1, &errata, pdf("errata.pdf"));

        let docs = state.row.documents();
        assert_eq!(docs[0].label, "Errata");
        assert!(matches!(
            &docs[0].source,
            DatasheetRef::HashPinned { filename, .. } if filename == "errata.pdf"
        ));
        assert_eq!(
            docs[1].source,
            DatasheetRef::url("https://ti.example/an.pdf")
        );
    }

    #[test]
    fn an_upload_pins_only_its_document() {
        let mut state = fixture();
        add(&mut state);
        set_mode(&mut state, 1, DatasheetMode::PinnedPdf);
        let picked_for = state.row.documents()[1].clone();
        apply_upload_result(&mut state, 1, &picked_for, pdf("errata.pdf"));
        assert_eq!(
            state.row.datasheet,
            DatasheetRef::url("https://ti.example/u1.pdf")
        );
        assert!(matches!(
            &state.row.more_datasheets[0].source,
            DatasheetRef::HashPinned { filename, hash } if filename == "errata.pdf" && hash.len() == 64
        ));
    }
}
