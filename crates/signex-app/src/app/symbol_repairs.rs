//! Opening a `.snxsym` the read had to repair (#3129).
//!
//! A build before #3129 could write a control character into a pin table,
//! which TOML forbids there, so the file saved and then never opened.
//! `SymbolFile::from_bytes_repairing` removes those characters; the editor
//! that opens the result says in Messages what was removed and stays
//! read-only until the user saves, so the file is never rewritten unasked.

use std::path::Path;

use signex_i18n::{I18n, ids, t};
use signex_library::SymbolLoadRepair;

use super::Signex;

/// Repairs named one by one before the rest are counted.
const SHOWN: usize = 20;

impl Signex {
    /// Mark the editor just opened at `path` repaired: dirty, read-only until
    /// saved, and each repair told in Messages. Nothing when `repairs` is empty.
    pub(crate) fn open_repaired_symbol(&mut self, path: &Path, repairs: &[SymbolLoadRepair]) {
        if repairs.is_empty() {
            return;
        }
        let Some(editor) = self.document_state.symbol_editors.get_mut(path) else {
            return;
        };
        editor
            .draft
            .open_repaired(crate::app::SymbolDocument::HISTORY_DEPTH);
        self.document_state.mark_document_dirty(path);
        for line in repair_lines(path, repairs, &self.ui_state.i18n) {
            crate::diagnostics::log_warning_at(line, path.to_path_buf(), None);
        }
    }
}

impl Signex {
    /// Why a write of the `.snxsym` at `path` other than the person's Save is
    /// refused: its editor opened it repaired and the repair is not on disk
    /// yet. `None` when no such editor holds it.
    pub(crate) fn repaired_symbol_write_refusal(&self, path: &Path) -> Option<String> {
        let editor = self.document_state.symbol_editors.get(path)?;
        if !editor.draft.is_read_only() {
            return None;
        }
        let file = path.file_name().unwrap_or_default().to_string_lossy();
        let line = t!(
            &self.ui_state.i18n,
            ids::EDITORS_SYMBOL_REPAIRED_WRITE_REFUSED,
            "file" => file.as_ref()
        );
        Some(line.into_string())
    }
}

/// The Messages lines for `repairs` of the file at `path`.
pub(crate) fn repair_lines(path: &Path, repairs: &[SymbolLoadRepair], i18n: &I18n) -> Vec<String> {
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    let mut lines: Vec<String> = repairs
        .iter()
        .take(SHOWN)
        .map(|repair| {
            t!(
                i18n,
                ids::EDITORS_SYMBOL_REPAIRED_ON_OPEN,
                "file" => file.as_ref(),
                "character" => signex_library::toml_literal::code_point(repair.character),
                "column" => repair.column,
                "pin" => repair.pin.as_str(),
                "symbol" => repair.symbol.as_str()
            )
            .into_string()
        })
        .collect();
    if repairs.len() > SHOWN {
        lines.push(
            t!(
                i18n,
                ids::EDITORS_SYMBOL_REPAIRED_MORE,
                "file" => file.as_ref(),
                "count" => repairs.len() - SHOWN
            )
            .into_string(),
        );
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered line without Fluent's bidi isolation marks.
    fn plain(line: &str) -> String {
        line.replace(['\u{2068}', '\u{2069}'], "")
    }

    fn repair(pin: &str) -> SymbolLoadRepair {
        SymbolLoadRepair {
            symbol: "OPAMP".into(),
            symbol_uuid: None,
            pin: pin.into(),
            column: "name",
            character: '\r',
        }
    }

    #[test]
    fn each_repair_names_the_file_symbol_pin_column_and_character() {
        let path = std::path::PathBuf::from("lib").join("opamps.snxsym");
        let lines = repair_lines(&path, &[repair("1")], &I18n::en_us());
        assert_eq!(lines.len(), 1);
        let line = plain(&lines[0]);
        for part in [
            "opamps.snxsym",
            "OPAMP",
            "pin 1",
            "name",
            "U+000D",
            "read-only",
        ] {
            assert!(line.contains(part), "{part} missing from {line}");
        }
        assert!(!line.contains("lib"), "the file name, not the path");
    }

    #[test]
    fn repairs_past_the_shown_ones_are_counted() {
        let many: Vec<_> = (0..SHOWN + 3).map(|i| repair(&i.to_string())).collect();
        let lines = repair_lines(std::path::Path::new("a.snxsym"), &many, &I18n::en_us());
        assert_eq!(lines.len(), SHOWN + 1);
        assert!(plain(&lines[SHOWN]).contains(" 3 more"), "{}", lines[SHOWN]);
    }
}
