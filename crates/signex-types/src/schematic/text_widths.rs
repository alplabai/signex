//! Measured text widths, kept with the sheet (#2023).
//!
//! Shaping text is the expensive part of a text box, and a hit test or a box
//! select asks for every box on the sheet. A width is measured once and read
//! back by item id and field; an entry remembers what it was measured from
//! (text, face, size), so a changed text, font, size or sheet default font,
//! an undo and a redo all make it measure again, with no dependence on a
//! revision counter. The entries are plain data, so a later format can store
//! the measured width in the file (#2029).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

/// Which text of an item a width belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextField {
    Label,
    Note,
    SymbolReference,
    SymbolValue,
}

/// A width and the inputs it was measured from.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredText {
    pub text: String,
    /// The resolved face (`None` is the bundled default).
    pub family: Option<String>,
    /// The cap height the width was measured at, mm.
    pub size_mm: f64,
    pub width_mm: f64,
}

impl MeasuredText {
    fn matches(&self, text: &str, family: Option<&str>, size_mm: f64) -> bool {
        self.size_mm == size_mm && self.family.as_deref() == family && self.text == text
    }
}

/// More entries than any sheet has items; reaching it starts the table over
/// (stale entries of deleted items are the only way to get there).
const MAX_ENTRIES: usize = 200_000;

/// The widths of one sheet. Clones share ONE table (an `Arc<Mutex>`): an undo
/// snapshot, a render snapshot and the engine's document all read and write the
/// same entries, each checked against its own inputs.
#[derive(Clone, Default)]
pub struct TextWidths(Arc<Mutex<HashMap<(Uuid, TextField), MeasuredText>>>);

impl std::fmt::Debug for TextWidths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextWidths")
            .field("entries", &self.len())
            .finish()
    }
}

impl TextWidths {
    /// The width of `text` for item `id`'s `field`: the stored one when it
    /// was measured from these inputs, else `measure()`, stored. `measure`
    /// says whether its answer is final; a stand-in (the shaper was not ready)
    /// is returned but not stored, so it is measured again later.
    pub fn width_mm(
        &self,
        id: Uuid,
        field: TextField,
        text: &str,
        family: Option<&str>,
        size_mm: f64,
        measure: impl FnOnce() -> (f64, bool),
    ) -> f64 {
        let mut table = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(hit) = table
            .get(&(id, field))
            .filter(|hit| hit.matches(text, family, size_mm))
        {
            return hit.width_mm;
        }
        let (width_mm, final_width) = measure();
        if !final_width {
            return width_mm;
        }
        if table.len() >= MAX_ENTRIES {
            table.clear();
        }
        table.insert(
            (id, field),
            MeasuredText {
                text: text.to_string(),
                family: family.map(str::to_string),
                size_mm,
                width_mm,
            },
        );
        width_mm
    }

    pub fn len(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_width_is_measured_once_and_again_when_any_input_changes() {
        let widths = TextWidths::default();
        let id = Uuid::now_v7();
        let calls = Cell::new(0);
        let get = |text: &str, family: Option<&str>, size: f64| {
            widths.width_mm(id, TextField::Note, text, family, size, || {
                calls.set(calls.get() + 1);
                (text.len() as f64, true)
            })
        };
        assert_eq!(get("abc", None, 1.27), 3.0);
        assert_eq!(get("abc", None, 1.27), 3.0);
        assert_eq!(calls.get(), 1, "second read is stored");
        get("abcd", None, 1.27);
        get("abcd", Some("Roboto"), 1.27);
        get("abcd", Some("Roboto"), 2.0);
        assert_eq!(calls.get(), 4, "text, face and size each re-measure");
    }

    /// A width measured before the shaper exists is a stand-in: it is not
    /// stored, so the first read after the shaper is installed measures again.
    #[test]
    fn a_stand_in_width_is_not_stored() {
        let widths = TextWidths::default();
        let id = Uuid::now_v7();
        let before = widths.width_mm(id, TextField::Note, "abc", None, 1.0, || (1.0, false));
        assert_eq!(before, 1.0);
        assert!(widths.is_empty(), "nothing stored for a stand-in");
        let after = widths.width_mm(id, TextField::Note, "abc", None, 1.0, || (2.0, true));
        assert_eq!(after, 2.0, "measured again once final");
        assert_eq!(widths.len(), 1);
    }

    #[test]
    fn clones_share_entries_and_each_field_has_its_own() {
        let widths = TextWidths::default();
        let id = Uuid::now_v7();
        widths.width_mm(id, TextField::Label, "x", None, 1.0, || (5.0, true));
        let clone = widths.clone();
        assert_eq!(
            clone.width_mm(id, TextField::Label, "x", None, 1.0, || (9.0, true)),
            5.0
        );
        assert_eq!(
            clone.width_mm(id, TextField::Note, "x", None, 1.0, || (9.0, true)),
            9.0
        );
    }
}
