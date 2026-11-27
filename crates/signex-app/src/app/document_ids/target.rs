//! Resolving a `Document` operand ([`DocumentTarget`], in `signex-commands`)
//! to the document it names.

use std::path::Path;

pub use signex_commands::DocumentTarget;

use super::entries::DocumentEntry;
use crate::app::Signex;
use crate::app::document::DocumentKey;
use crate::app::state::scope::path_key_with;

impl Signex {
    /// The document `target` names, among every document listed (loaded or
    /// only referenced by an open project). Never the active document: a
    /// target that names nothing is refused with the reason.
    pub(crate) fn resolve_document_target(
        &self,
        target: &DocumentTarget,
    ) -> Result<DocumentEntry, String> {
        let entries = self.document_entries();
        match target {
            DocumentTarget::Id(id) => entries
                .into_iter()
                .find(|entry| entry.id == Some(*id))
                .ok_or_else(|| format!("no document has the id {id}")),
            DocumentTarget::Path(text) => {
                let given = Path::new(text);
                let key = path_key_with(given, self.identity_fold_for(given));
                entries
                    .into_iter()
                    .find(|entry| {
                        entry.path.as_deref().is_some_and(|path| {
                            path_key_with(path, self.identity_fold_for(path)) == key
                        })
                    })
                    // A block diagram is not listed in its project: one in
                    // a project's folder is named by path, opened on demand.
                    .or_else(|| self.unopened_block_diagram(given))
                    .ok_or_else(|| format!("not an open or referenced document: {text}"))
            }
        }
    }

    /// The revision `read_document` reports for `entry`: a loaded sheet's
    /// engine revision or an open board's. `None` for every other document,
    /// which has no revision to compare.
    pub(crate) fn document_revision(&self, entry: &DocumentEntry) -> Option<u64> {
        let docs = &self.document_state;
        match &entry.key {
            Some(DocumentKey::Schematic(path)) => docs
                .engines
                .get(path)
                .filter(|_| entry.loaded)
                .map(signex_engine::Engine::revision),
            Some(DocumentKey::Pcb(path)) => docs.pcb_revisions.get(path).copied(),
            _ => None,
        }
    }
}
