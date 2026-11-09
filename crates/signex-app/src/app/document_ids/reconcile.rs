//! The one place a document is given an id it does not have (#2172).
//!
//! Every loaded document reads its id from its own file. A file can hold none
//! (a hand-made or old one: the nil uuid) or the id of another document (a
//! file copied outside Signex). Both are repaired here and nowhere else: the
//! first document seen keeps its id, a later one gets a new id and Messages
//! says so. The new id lives in the document, so its next save writes it;
//! the repair itself is not an edit and does not mark the document dirty.

use std::collections::HashMap;

use signex_types::document_id::DocumentId;

use super::entries::DocumentEntry;
use crate::app::document::DocumentKey;
use crate::app::state::scope::path_key_with;
use crate::app::{Signex, TabDocument};

impl Signex {
    /// Make every loaded document's id present and unique. Returns the
    /// documents that were given a new id.
    pub(crate) fn reconcile_document_ids(&mut self) -> Vec<DocumentKey> {
        let mut held: Vec<DocumentEntry> = self
            .all_document_entries()
            .into_iter()
            .filter(|entry| entry.key.is_some() && entry.loaded)
            .collect();
        // Documents are first seen in tab order, then whatever is held with
        // no tab (a parked draft, a project), so the order a copy was opened
        // in decides who keeps the id.
        let tab_keys: Vec<DocumentKey> = self
            .document_state
            .tabs
            .iter()
            .filter_map(DocumentKey::of)
            .collect();
        let ids = &mut self.document_state.document_ids;
        ids.forget_unheld_orders(
            tab_keys
                .iter()
                .chain(held.iter().filter_map(|e| e.key.as_ref())),
        );
        for key in tab_keys
            .iter()
            .chain(held.iter().filter_map(|e| e.key.as_ref()))
        {
            ids.seen_order(key);
        }
        held.sort_by_key(|entry| {
            entry
                .key
                .as_ref()
                .map_or(u64::MAX, |key| ids.seen_order(key))
        });
        let mut first: HashMap<DocumentId, (String, Option<std::path::PathBuf>)> = HashMap::new();
        let mut repaired = Vec::new();
        let mut minted: Vec<(std::path::PathBuf, DocumentId)> = Vec::new();
        for entry in held {
            let Some(key) = entry.key.clone() else {
                continue;
            };
            let label = entry
                .path
                .as_deref()
                .and_then(|path| path.file_name())
                .map_or_else(|| entry.name.clone(), |n| n.to_string_lossy().into_owned());
            match entry.id {
                Some(id) if !first.contains_key(&id) => {
                    first.insert(id, (label, entry.path.clone()));
                }
                Some(id) => {
                    let (owner, owner_path) = first.get(&id).cloned().unwrap_or_default();
                    // The same file held under another spelling of its path
                    // (a case variant) is one document, not a copy.
                    let same_file = entry
                        .path
                        .as_deref()
                        .zip(owner_path.as_deref())
                        .is_some_and(|(a, b)| {
                            let fold = self.identity_fold_for(a);
                            path_key_with(a, fold) == path_key_with(b, fold)
                        });
                    if same_file {
                        continue;
                    }
                    crate::diagnostics::log_warning(format!(
                        "{label} has the same document id as {owner}; it was given a new id, \
                         which its next save writes"
                    ));
                    self.give_new_id(&key, DocumentId::new());
                    repaired.push(key);
                }
                None => {
                    // Two spellings of one file are one document: they get
                    // the one id, so both encode alike.
                    let same_file = |path: &std::path::Path, other: &std::path::Path| {
                        let fold = self.identity_fold_for(path);
                        path_key_with(path, fold) == path_key_with(other, fold)
                    };
                    let shared = entry.path.as_deref().and_then(|path| {
                        let held = first.iter().filter_map(|(id, (_, other))| {
                            other.as_deref().map(|other| (other, *id))
                        });
                        held.chain(minted.iter().map(|(other, id)| (other.as_path(), *id)))
                            .find(|(other, _)| same_file(path, other))
                            .map(|(_, id)| id)
                    });
                    let id = shared.unwrap_or_else(DocumentId::new);
                    if let Some(path) = &entry.path {
                        minted.push((path.clone(), id));
                    }
                    self.give_new_id(&key, id);
                    repaired.push(key);
                }
            }
        }
        repaired
    }

    /// Store `id` in the document `key` holds.
    fn give_new_id(&mut self, key: &DocumentKey, id: DocumentId) {
        let docs = &mut self.document_state;
        match key {
            DocumentKey::Schematic(path) => {
                let old = docs.engines.get_mut(path).map(|engine| {
                    let old = engine.document().uuid;
                    engine.reassign_document_id(id.as_uuid());
                    old
                });
                // Net waivers name their sheet by id: they move with it.
                if let Some(old) = old.filter(|old| !old.is_nil()) {
                    let owner = self.waiver_owner_of(path);
                    self.follow_waivers(owner, old, id.as_uuid(), super::waivers::Follow::Copy);
                }
            }
            DocumentKey::Pcb(path) => {
                for tab in docs.tabs.iter_mut().filter(|tab| &tab.path == path) {
                    if let Some(TabDocument::Pcb(board)) = &mut tab.cached_document {
                        board.uuid = id.as_uuid();
                    }
                }
                if let Some(board) = docs.parked_pcb_boards.get_mut(path) {
                    board.uuid = id.as_uuid();
                }
            }
            DocumentKey::Symbol(path) => {
                if let Some(editor) = docs.symbol_editors.get_mut(path) {
                    editor.file.file_uuid = id.as_uuid();
                    editor.baseline.file_uuid = id.as_uuid();
                }
            }
            DocumentKey::Footprint(path) => {
                if let Some(editor) = docs.footprint_editors.get_mut(path) {
                    editor.file.file_uuid = id.as_uuid();
                    editor.baseline.file_uuid = id.as_uuid();
                }
            }
            DocumentKey::Project(project) => {
                if let Some(project) = docs.project_by_id_mut(*project) {
                    project.data.document_id = Some(id);
                }
            }
            // A row's id is its library identity; the library never repeats one.
            DocumentKey::Row(_) => {}
        }
    }
}
