//! The order of a project's sheets (#1689) and libraries (#1691).
//!
//! The sheet list's order is the page order a sheet's title block shows
//! (Sheet N of M) and the project tree lists; the PDF export numbers by the
//! sheet hierarchy, not by this list. The library list's
//! order is the order the project tree shows and mounts them in. Both lists
//! have other writers (Add, Remove), so an undo puts back an order, never a
//! list: [`ProjectData::restore_sheet_order`] reorders the entries it names
//! and leaves the rest where they are.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ProjectData;

/// One step up or down a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListMove {
    Up,
    Down,
}

impl ListMove {
    /// The operand token: `up` or `down`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }

    /// Parse an operand token, ignoring ASCII case.
    pub fn parse(token: &str) -> Option<Self> {
        let token = token.trim();
        [Self::Up, Self::Down]
            .into_iter()
            .find(|step| step.token().eq_ignore_ascii_case(token))
    }
}

/// Move the item at `index` one step; `false` at the end it moves towards.
fn step_item<T>(items: &mut [T], index: usize, step: ListMove) -> bool {
    let target = match step {
        ListMove::Up => index.checked_sub(1),
        ListMove::Down => Some(index + 1).filter(|next| *next < items.len()),
    };
    match target {
        Some(target) => {
            items.swap(index, target);
            true
        }
        None => false,
    }
}

/// Put the items `order` names in that order, in the places those items hold
/// now; an item it does not name keeps its place. Each name takes at most
/// one item. So a sheet renamed or added since keeps its page.
fn reorder<T, K: PartialEq + ?Sized>(items: &mut Vec<T>, order: &[&K], key: impl Fn(&T) -> &K) {
    let mut taken = vec![false; items.len()];
    let mut picks: Vec<usize> = Vec::new();
    for wanted in order {
        let found = (0..items.len()).find(|&i| !taken[i] && key(&items[i]) == *wanted);
        if let Some(index) = found {
            taken[index] = true;
            picks.push(index);
        }
    }
    let mut slots = picks.clone();
    slots.sort_unstable();
    let mut old: Vec<Option<T>> = std::mem::take(items).into_iter().map(Some).collect();
    let mut placed: Vec<Option<T>> = std::iter::repeat_with(|| None).take(old.len()).collect();
    for (slot, pick) in slots.iter().zip(&picks) {
        placed[*slot] = old[*pick].take();
    }
    for (index, item) in old.into_iter().enumerate() {
        if item.is_some() {
            placed[index] = item;
        }
    }
    *items = placed.into_iter().flatten().collect();
}

impl ProjectData {
    /// The sheet files in page order.
    pub fn sheet_order(&self) -> Vec<String> {
        self.sheets.iter().map(|s| s.filename.clone()).collect()
    }

    /// Move the sheet `filename` one page up or down. `false` when there is
    /// no such sheet or it is already at that end.
    pub fn move_sheet(&mut self, filename: &str, step: ListMove) -> bool {
        self.sheets
            .iter()
            .position(|s| s.filename == filename)
            .is_some_and(|index| step_item(&mut self.sheets, index, step))
    }

    /// Put the sheets back in `order` (see the module docs).
    pub fn restore_sheet_order(&mut self, order: &[String]) {
        let order: Vec<&str> = order.iter().map(String::as_str).collect();
        reorder(&mut self.sheets, &order, |s| s.filename.as_str());
    }

    /// The library paths, as stored, in list order.
    pub fn library_order(&self) -> Vec<PathBuf> {
        self.libraries.iter().map(|l| l.path.clone()).collect()
    }

    /// Move the library stored at `path` one step up or down.
    pub fn move_library(&mut self, path: &Path, step: ListMove) -> bool {
        self.libraries
            .iter()
            .position(|l| l.path == path)
            .is_some_and(|index| step_item(&mut self.libraries, index, step))
    }

    /// Put the libraries back in `order` (see the module docs).
    pub fn restore_library_order(&mut self, order: &[PathBuf]) {
        let order: Vec<&Path> = order.iter().map(PathBuf::as_path).collect();
        reorder(&mut self.libraries, &order, |l| l.path.as_path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{LibraryEntry, LibraryEntryKind, SheetEntry};

    const EMPTY_PROJECT: &str = r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#;

    fn project(sheets: &[&str], libraries: &[&str]) -> ProjectData {
        let mut data =
            serde_json::from_str::<ProjectData>(EMPTY_PROJECT).expect("an empty project");
        data.sheets = sheets
            .iter()
            .map(|name| SheetEntry {
                name: (*name).into(),
                filename: format!("{name}.snxsch"),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            })
            .collect();
        data.libraries = libraries
            .iter()
            .map(|name| LibraryEntry {
                path: PathBuf::from(format!("{name}.snxlib")),
                kind: LibraryEntryKind::ProjectLocal,
                library_id: None,
                unknown: Default::default(),
            })
            .collect();
        data
    }

    fn names(data: &ProjectData) -> Vec<String> {
        data.sheet_order()
    }

    #[test]
    fn a_sheet_moves_one_page_and_stops_at_the_ends() {
        let mut data = project(&["a", "b", "c"], &[]);
        assert!(data.move_sheet("c.snxsch", ListMove::Up));
        assert_eq!(names(&data), ["a.snxsch", "c.snxsch", "b.snxsch"]);
        assert!(!data.move_sheet("a.snxsch", ListMove::Up));
        assert!(data.move_sheet("b.snxsch", ListMove::Up));
        assert!(!data.move_sheet("a.snxsch", ListMove::Up));
        assert!(!data.move_sheet("missing.snxsch", ListMove::Down));
        assert_eq!(names(&data), ["a.snxsch", "b.snxsch", "c.snxsch"]);
    }

    #[test]
    fn restoring_an_order_keeps_a_sheet_added_since_and_drops_none() {
        let mut data = project(&["a", "b", "c"], &[]);
        let before = data.sheet_order();
        assert!(data.move_sheet("a.snxsch", ListMove::Down));
        let mut added = project(&["d"], &[]);
        data.sheets.push(added.sheets.remove(0));
        data.sheets.retain(|s| s.filename != "c.snxsch");
        data.restore_sheet_order(&before);
        assert_eq!(names(&data), ["a.snxsch", "b.snxsch", "d.snxsch"]);
    }

    #[test]
    fn a_sheet_renamed_since_keeps_its_page_when_an_order_is_restored() {
        let mut data = project(&["a", "b", "c"], &[]);
        let before = data.sheet_order();
        data.sheets[0].filename = "z.snxsch".into();
        data.restore_sheet_order(&before);
        assert_eq!(names(&data), ["z.snxsch", "b.snxsch", "c.snxsch"]);
    }

    #[test]
    fn a_library_moves_and_its_order_restores() {
        let mut data = project(&[], &["x", "y"]);
        let before = data.library_order();
        assert!(data.move_library(Path::new("x.snxlib"), ListMove::Down));
        assert_eq!(
            data.library_order(),
            [PathBuf::from("y.snxlib"), PathBuf::from("x.snxlib")]
        );
        assert!(!data.move_library(Path::new("x.snxlib"), ListMove::Down));
        data.restore_library_order(&before);
        assert_eq!(data.library_order(), before);
    }

    #[test]
    fn a_step_token_parses_either_case() {
        assert_eq!(ListMove::parse(" UP "), Some(ListMove::Up));
        assert_eq!(ListMove::parse("down"), Some(ListMove::Down));
        assert_eq!(ListMove::parse("left"), None);
    }
}
