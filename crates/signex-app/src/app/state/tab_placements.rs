//! Which sheet symbol the user descended through to reach each open child
//! tab (#1133), so "Current page" prints that placement's designators.
//!
//! The placement type and its designator rule live in `signex-types`; this is
//! only the per-tab memory. A tab opened straight from disk has no entry, and
//! the root chain is never stored: a root page is not a placement of anything.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::InstancePath;

#[derive(Debug, Default)]
pub struct TabPlacements(HashMap<PathBuf, InstancePath>);

impl TabPlacements {
    pub fn get(&self, path: &Path) -> Option<&InstancePath> {
        self.0.get(path)
    }

    /// Remember `placement` for the tab at `path`, replacing what was there.
    /// `None` and the root chain both mean "no known placement" and clear it.
    pub fn record(&mut self, path: &Path, placement: Option<InstancePath>) {
        match placement.filter(|p| !p.is_root()) {
            Some(placement) => {
                self.0.insert(path.to_path_buf(), placement);
            }
            None => self.forget(path),
        }
    }

    /// The tab at `path` closed or became a different file.
    pub fn forget(&mut self, path: &Path) {
        self.0.remove(path);
    }

    /// The tab's file was renamed; the sheet symbol still names it.
    pub fn rekey(&mut self, old: &Path, new: &Path) {
        if let Some(placement) = self.0.remove(old) {
            self.0.insert(new.to_path_buf(), placement);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn placement(n: u128) -> InstancePath {
        InstancePath::from_chain(vec![Uuid::from_u128(n)])
    }

    #[test]
    fn recording_replaces_and_the_root_or_none_clears() {
        let mut map = TabPlacements::default();
        let path = Path::new("/w/child.snxsch");
        map.record(path, Some(placement(1)));
        map.record(path, Some(placement(2)));
        assert_eq!(map.get(path), Some(&placement(2)));
        map.record(path, Some(InstancePath::root()));
        assert_eq!(map.get(path), None);
        map.record(path, Some(placement(3)));
        map.record(path, None);
        assert_eq!(map.get(path), None);
    }

    #[test]
    fn rekey_moves_the_entry_and_forget_drops_it() {
        let mut map = TabPlacements::default();
        let (old, new) = (Path::new("/w/a.snxsch"), Path::new("/w/b.snxsch"));
        map.record(old, Some(placement(1)));
        map.rekey(old, new);
        assert_eq!(map.get(old), None);
        assert_eq!(map.get(new), Some(&placement(1)));
        map.forget(new);
        assert_eq!(map.get(new), None);
    }
}
