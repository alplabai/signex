//! Memo of every open sheet's child-sheet references.
//!
//! Project ownership ([`DocumentState::document_project`] and friends) walks
//! the child-sheet references of all open engines, and used to rebuild that map
//! — cloning every child filename — on each call. The answer only moves when a
//! sheet's document does, and an engine's [`revision`](signex_engine::Engine::revision)
//! is unique per instance and moves with every edit, so the memo is checked
//! against `(path, revision)` pairs rather than invalidated by hooks at each
//! mutation site.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use signex_engine::Engine;

/// `sheet path → the child `filename` strings it references`.
pub(crate) type ChildSheetRefs = HashMap<PathBuf, Vec<String>>;

struct Cached {
    /// The engines the refs were read from, with the revision each had.
    revisions: HashMap<PathBuf, u64>,
    refs: Arc<ChildSheetRefs>,
}

impl Cached {
    fn is_current(&self, engines: &HashMap<PathBuf, Engine>) -> bool {
        self.revisions.len() == engines.len()
            && engines
                .iter()
                .all(|(path, engine)| self.revisions.get(path) == Some(&engine.revision()))
    }
}

/// Interior-mutable only because the lookups it serves take `&self`; it holds
/// nothing the engines do not, so a stale entry is never trusted.
#[derive(Default)]
pub struct ChildRefsCache(Mutex<Option<Cached>>);

impl ChildRefsCache {
    /// The refs for `engines`, rebuilt only if an engine was added, removed or
    /// edited since the last call.
    pub(crate) fn get(&self, engines: &HashMap<PathBuf, Engine>) -> Arc<ChildSheetRefs> {
        let mut slot = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cached) = slot.as_ref()
            && cached.is_current(engines)
        {
            return Arc::clone(&cached.refs);
        }
        let refs: Arc<ChildSheetRefs> = Arc::new(
            engines
                .iter()
                .map(|(path, engine)| {
                    let refs = engine
                        .document()
                        .child_sheets
                        .iter()
                        .map(|cs| cs.filename.clone())
                        .collect();
                    (path.clone(), refs)
                })
                .collect(),
        );
        *slot = Some(Cached {
            revisions: engines
                .iter()
                .map(|(path, engine)| (path.clone(), engine.revision()))
                .collect(),
            refs: Arc::clone(&refs),
        });
        refs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};

    fn engine_referencing(path: &str, child: &str) -> (PathBuf, Engine) {
        let sheet = SchematicSheet {
            child_sheets: vec![ChildSheet {
                uuid: uuid::Uuid::nil(),
                name: String::new(),
                filename: child.to_string(),
                position: Point::ZERO,
                size: (0.0, 0.0),
                stroke_width: 0.0,
                fill: FillType::None,
                stroke_color: None,
                fill_color: None,
                fields_autoplaced: false,
                pins: Vec::new(),
                instances: Vec::new(),
            }],
            ..Default::default()
        };
        let path = PathBuf::from(path);
        let engine = Engine::new_with_path(sheet, Some(path.clone())).expect("engine");
        (path, engine)
    }

    #[test]
    fn refs_are_reused_until_a_sheet_changes() {
        let cache = ChildRefsCache::default();
        let mut engines = HashMap::new();
        let (path, engine) = engine_referencing("/w/top.snxsch", "mid.snxsch");
        engines.insert(path.clone(), engine);

        let first = cache.get(&engines);
        assert!(
            Arc::ptr_eq(&first, &cache.get(&engines)),
            "an unchanged project must not rebuild the refs"
        );

        // An edit to a sheet's references is seen by the next lookup.
        engines
            .get_mut(&path)
            .unwrap()
            .execute(signex_engine::Command::RenameChildSheetFile {
                from: "mid.snxsch".to_string(),
                to: "renamed.snxsch".to_string(),
            })
            .expect("rename");
        let edited = cache.get(&engines);
        assert_eq!(edited[&path], vec!["renamed.snxsch".to_string()]);

        // So is a sheet opening and a sheet closing.
        let (other, engine) = engine_referencing("/w/other.snxsch", "leaf.snxsch");
        engines.insert(other.clone(), engine);
        assert!(cache.get(&engines).contains_key(&other));
        engines.remove(&other);
        assert!(!cache.get(&engines).contains_key(&other));
    }
}
