//! A copied selection as text for the OS clipboard.
//!
//! The text is a marker line followed by the fragment written as a `.snxsch`
//! document, so a second Signex process can paste it and the library bodies
//! travel with the parts. Anything that does not start with the marker is
//! somebody else's text and is none of our business.

use signex_types::format::SnxSchematic;

use crate::{CopiedSelection, EngineError};

/// Largest marked clipboard text that is parsed; anything bigger is refused
/// before the parser sees it.
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// First line of every clipboard fragment; the trailing `/1` is the fragment
/// version.
pub const CLIPBOARD_MARKER: &str = "signex-clipboard-fragment/1";

impl CopiedSelection {
    /// The copy as clipboard text: the marker line, then the fragment at its
    /// original sheet positions.
    pub fn to_clipboard_text(&self) -> Result<String, EngineError> {
        let absolute = self.payload.translated(self.origin.x, self.origin.y);
        let body = SnxSchematic::new(absolute.fragment().clone())
            .write_string()
            .map_err(|e| EngineError::ClipboardFragment(e.to_string()))?;
        Ok(format!("{CLIPBOARD_MARKER}\n{body}"))
    }

    /// The copy `text` carries. `Ok(None)` when `text` is not a Signex
    /// fragment at all; `Err` when it claims to be one but cannot be read.
    /// `grid_mm` snaps the anchor as in [`crate::Engine::copy_selection`].
    pub fn from_clipboard_text(text: &str, grid_mm: f64) -> Result<Option<Self>, EngineError> {
        if !text.starts_with(CLIPBOARD_MARKER) {
            return Ok(None);
        }
        if text.len() > MAX_CLIPBOARD_TEXT_BYTES {
            return Err(EngineError::ClipboardFragment("too large".to_owned()));
        }
        let (first, body) = text.split_once('\n').unwrap_or((text, ""));
        if first.trim_end() != CLIPBOARD_MARKER {
            return Ok(None);
        }
        let parsed =
            SnxSchematic::parse(body).map_err(|e| EngineError::ClipboardFragment(e.to_string()))?;
        Self::from_fragment(parsed.sheet, grid_mm)?
            .map(Some)
            .ok_or_else(|| EngineError::ClipboardFragment("it holds nothing to paste".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{
        Label, LabelType, LibSymbol, Point, SelectedItem, SelectedKind, Symbol, Wire,
    };
    use uuid::Uuid;

    fn copied() -> CopiedSelection {
        let mut sheet = test_sheet();
        let lib: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
        sheet.lib_symbols.insert(lib.id.clone(), lib);
        let part = Symbol {
            uuid: Uuid::now_v7(),
            lib_id: "lib:r".to_owned(),
            reference: "R7".to_owned(),
            position: Point::new(25.4, 25.4),
            unit: 1,
            ..Default::default()
        };
        let wire = Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(25.4, 25.4),
            end: Point::new(38.1, 25.4),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
        };
        let label = Label {
            uuid: Uuid::now_v7(),
            text: "D0".to_owned(),
            position: Point::new(38.1, 25.4),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            font_family: None,
            justify: Default::default(),
            justify_v: Default::default(),
            unknown_columns: Default::default(),
        };
        let items = vec![
            SelectedItem::new(part.uuid, SelectedKind::Symbol),
            SelectedItem::new(wire.uuid, SelectedKind::Wire),
            SelectedItem::new(label.uuid, SelectedKind::Label),
        ];
        sheet.symbols.push(part);
        sheet.wires.push(wire);
        sheet.labels.push(label);
        let engine = Engine::new(sheet).unwrap();
        engine.copy_selection(&items, 1.27).unwrap().unwrap()
    }

    #[test]
    fn a_copy_round_trips_through_text_with_its_bodies() {
        let original = copied();
        let text = original.to_clipboard_text().unwrap();
        assert!(text.starts_with("signex-clipboard-fragment/1\n"));

        let back = CopiedSelection::from_clipboard_text(&text, 1.27)
            .unwrap()
            .unwrap();
        let (a, b) = (original.payload.fragment(), back.payload.fragment());
        assert_eq!(b.symbols.len(), 1);
        assert_eq!(b.symbols[0].reference, "R7");
        assert_eq!(b.wires.len(), 1);
        assert_eq!(b.labels[0].text, "D0");
        assert!(b.lib_symbols.contains_key("lib:r"));
        assert_eq!(a.symbols[0].position, b.symbols[0].position);
        assert_eq!(original.origin, back.origin);
    }

    #[test]
    fn text_without_the_marker_is_not_ours() {
        for text in ["", "hello", "R1\nR2", "signex-clipboard-fragment/2\nx"] {
            assert!(
                CopiedSelection::from_clipboard_text(text, 1.27)
                    .unwrap()
                    .is_none(),
                "{text:?}"
            );
        }
    }

    #[test]
    fn marked_text_over_the_cap_is_refused_before_parsing() {
        let huge = format!(
            "{CLIPBOARD_MARKER}\n{}",
            "x".repeat(MAX_CLIPBOARD_TEXT_BYTES)
        );
        let error = CopiedSelection::from_clipboard_text(&huge, 1.27).unwrap_err();
        assert!(error.to_string().contains("too large"), "{error}");
        // Unmarked text of any size is simply not ours.
        let other = "y".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert!(
            CopiedSelection::from_clipboard_text(&other, 1.27)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn marked_text_that_cannot_be_read_is_an_error() {
        let broken = format!("{CLIPBOARD_MARKER}\nthis is not a schematic");
        assert!(CopiedSelection::from_clipboard_text(&broken, 1.27).is_err());
        let marker_only = format!("{CLIPBOARD_MARKER}\n");
        assert!(CopiedSelection::from_clipboard_text(&marker_only, 1.27).is_err());
    }
}
