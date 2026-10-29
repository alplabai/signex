//! The "font is not installed" notice for an open document (#510).
//!
//! A document keeps naming the family its author chose; this only *tells*
//! the user that the canvas and the exports are drawing something else. It
//! reports a family once per document, so reopening the Properties panel or
//! editing a label does not repeat it.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use signex_types::font::{BUNDLED_DEFAULT_FAMILY, requested_family};
use signex_types::schematic::SchematicSheet;

use super::catalog::FontCatalog;

/// A family the document names that this machine does not have, and how many
/// text items asked for it (their own family or the document default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingFont {
    pub family: String,
    pub item_count: usize,
}

impl MissingFont {
    /// The Messages-panel text. Exports can only embed the bundled faces, so
    /// the one substitution named here is also what the PDF and SVG use.
    pub fn message(&self, document: &str) -> String {
        let noun = if self.item_count == 1 {
            "text item"
        } else {
            "text items"
        };
        format!(
            "Font '{}' is not installed; using {} for {} {} in {} (the file keeps the name; \
             PDF and SVG export use {} too)",
            self.family,
            BUNDLED_DEFAULT_FAMILY,
            self.item_count,
            noun,
            document,
            BUNDLED_DEFAULT_FAMILY,
        )
    }
}

/// Every missing family in `sheet`, most-used first, then by name.
pub fn missing_fonts(sheet: &SchematicSheet, catalog: &FontCatalog) -> Vec<MissingFont> {
    let document = sheet.font_family.as_deref();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut count = |item: Option<&str>| {
        let Some(requested) = requested_family(item, document) else {
            return;
        };
        if requested.eq_ignore_ascii_case(BUNDLED_DEFAULT_FAMILY) || catalog.has(requested) {
            return;
        }
        *counts.entry(requested.to_string()).or_default() += 1;
    };

    for label in &sheet.labels {
        count(label.font_family.as_deref());
    }
    for note in &sheet.text_notes {
        count(note.font_family.as_deref());
    }
    for symbol in &sheet.symbols {
        // A symbol with no stored text property still draws the field, with
        // the document default.
        for (prop, text) in [
            (&symbol.ref_text, &symbol.reference),
            (&symbol.val_text, &symbol.value),
        ] {
            match prop {
                Some(prop) if prop.hidden => {}
                Some(prop) => count(prop.font_family.as_deref()),
                None if !text.is_empty() => count(None),
                None => {}
            }
        }
    }
    // Sheet symbols name themselves in the document default.
    for _ in &sheet.child_sheets {
        count(None);
    }

    let mut missing: Vec<MissingFont> = counts
        .into_iter()
        .map(|(family, item_count)| MissingFont { family, item_count })
        .collect();
    missing.sort_by(|a, b| {
        b.item_count
            .cmp(&a.item_count)
            .then(a.family.cmp(&b.family))
    });
    missing
}

/// Remembers which `(document, family)` pairs were already reported.
#[derive(Debug, Default)]
pub struct MissingFontReporter {
    reported: HashSet<(PathBuf, String)>,
}

impl MissingFontReporter {
    /// Messages for families of `sheet` not yet reported for `path`.
    pub fn new_messages(
        &mut self,
        path: &Path,
        sheet: &SchematicSheet,
        catalog: &FontCatalog,
    ) -> Vec<String> {
        let document = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        missing_fonts(sheet, catalog)
            .into_iter()
            .filter(|missing| {
                self.reported
                    .insert((path.to_path_buf(), missing.family.to_lowercase()))
            })
            .map(|missing| missing.message(&document))
            .collect()
    }

    /// Forget `path`, so reopening the document reports again.
    pub fn forget(&mut self, path: &Path) {
        self.reported.retain(|(reported, _)| reported != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        HAlign, Label, LabelType, Point, SchematicSheet, TextNote, VAlign,
    };

    fn label(family: Option<&str>) -> Label {
        Label {
            uuid: uuid::Uuid::now_v7(),
            text: "N".into(),
            position: Point { x: 0.0, y: 0.0 },
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            font_family: family.map(str::to_string),
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
            unknown_columns: Default::default(),
            locked: false,
        }
    }

    fn note(family: Option<&str>) -> TextNote {
        TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: "n".into(),
            position: Point { x: 0.0, y: 0.0 },
            rotation: 0.0,
            font_size: 1.27,
            font_family: family.map(str::to_string),
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked: false,
        }
    }

    fn catalog() -> FontCatalog {
        FontCatalog::from_families(["Fira Mono"])
    }

    #[test]
    fn counts_items_that_ask_for_a_missing_family_directly_or_by_inheritance() {
        let sheet = SchematicSheet {
            font_family: Some("Ghost Sans".into()),
            labels: vec![
                label(None),
                label(Some("Ghost Sans")),
                label(Some("Fira Mono")),
            ],
            text_notes: vec![note(Some("Other Ghost"))],
            ..Default::default()
        };
        assert_eq!(
            missing_fonts(&sheet, &catalog()),
            vec![
                MissingFont {
                    family: "Ghost Sans".into(),
                    item_count: 2
                },
                MissingFont {
                    family: "Other Ghost".into(),
                    item_count: 1
                },
            ]
        );
    }

    #[test]
    fn nothing_is_missing_when_no_family_is_named_or_all_are_installed() {
        let sheet = SchematicSheet {
            labels: vec![
                label(None),
                label(Some("fira mono")),
                label(Some("Iosevka")),
            ],
            ..Default::default()
        };
        assert!(missing_fonts(&sheet, &catalog()).is_empty());
    }

    #[test]
    fn the_message_names_the_family_the_fallback_the_count_and_the_document() {
        let missing = MissingFont {
            family: "Ghost Sans".into(),
            item_count: 3,
        };
        let text = missing.message("power.snxsch");
        assert!(text.starts_with(
            "Font 'Ghost Sans' is not installed; using Iosevka for 3 text items in power.snxsch"
        ));
    }

    #[test]
    fn a_family_is_reported_once_per_document_and_again_after_reopening() {
        let sheet = SchematicSheet {
            labels: vec![label(Some("Ghost Sans"))],
            ..Default::default()
        };
        let mut reporter = MissingFontReporter::default();
        let a = Path::new("/p/a.snxsch");
        let b = Path::new("/p/b.snxsch");

        assert_eq!(reporter.new_messages(a, &sheet, &catalog()).len(), 1);
        assert!(reporter.new_messages(a, &sheet, &catalog()).is_empty());
        assert_eq!(
            reporter.new_messages(b, &sheet, &catalog()).len(),
            1,
            "another document is reported on its own"
        );

        reporter.forget(a);
        assert_eq!(reporter.new_messages(a, &sheet, &catalog()).len(), 1);
    }

    #[test]
    fn a_second_family_added_later_is_reported_without_repeating_the_first() {
        let mut sheet = SchematicSheet {
            labels: vec![label(Some("Ghost Sans"))],
            ..Default::default()
        };
        let mut reporter = MissingFontReporter::default();
        let path = Path::new("/p/a.snxsch");
        assert_eq!(reporter.new_messages(path, &sheet, &catalog()).len(), 1);

        sheet.labels.push(label(Some("Other Ghost")));
        let again = reporter.new_messages(path, &sheet, &catalog());
        assert_eq!(again.len(), 1);
        assert!(again[0].contains("Other Ghost"));
    }
}
