//! Row updates a primitive's save still owes (#2004), kept per file for as
//! long as this app runs.
//!
//! A save that could not move every row bound to a primitive leaves the
//! obligation on the editor. An editor that is closed, or replaced while its
//! save ran, would take it along, so it is kept here and the file's next save
//! (or reopened editor) picks it up. Nothing is written to disk: a restart
//! drops it, because carrying it over would need a new on-disk record.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct OwedCascades {
    by_file: HashMap<PathBuf, Vec<Uuid>>,
}

impl OwedCascades {
    /// Replace what `file` owes; nothing owed forgets the file.
    pub fn set(&mut self, file: &Path, owed: &[Uuid]) {
        if owed.is_empty() {
            self.by_file.remove(file);
        } else {
            self.by_file.insert(file.to_path_buf(), owed.to_vec());
        }
    }

    /// What `file` owes.
    pub fn of(&self, file: &Path) -> &[Uuid] {
        self.by_file.get(file).map_or(&[], Vec::as_slice)
    }

    /// The files that owe something.
    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.by_file.keys().map(PathBuf::as_path)
    }

    /// What `file` owes, forgetting it.
    pub fn take(&mut self, file: &Path) -> Vec<Uuid> {
        self.by_file.remove(file).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_file_owes_is_kept_until_taken() {
        let mut owed = OwedCascades::default();
        let (file, id) = (Path::new("a.snxsym"), Uuid::now_v7());
        owed.set(file, &[id]);
        assert_eq!(owed.of(file), [id]);
        assert_eq!(owed.take(file), vec![id]);
        assert!(owed.of(file).is_empty());
    }

    #[test]
    fn setting_nothing_forgets_the_file() {
        let mut owed = OwedCascades::default();
        let file = Path::new("a.snxsym");
        owed.set(file, &[Uuid::now_v7()]);
        owed.set(file, &[]);
        assert!(owed.take(file).is_empty());
    }
}
