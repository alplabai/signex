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
            locked: false,
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
            locked: false,
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
    /// A fragment with every kind of item the writer and parser know, on an
    /// exact binary grid so positions compare equal after the nm round trip.
    fn rich_fragment() -> signex_types::schematic::SchematicSheet {
        use signex_types::schematic::{Bus, NoConnect, SchDrawing, TextNote};
        let at = |x: f64, y: f64| Point::new(x, y);
        let foreign = r#"[["x_vendor","v1"]]"#;
        let mut sheet = test_sheet();
        sheet.wires.push(
            serde_json::from_str(&format!(
                r#"{{"uuid":"{}","start":{{"x":5.0,"y":5.0}},"end":{{"x":15.0,"y":5.0}},"stroke_width":0.25,"unknown_columns":{foreign}}}"#,
                Uuid::new_v4()
            ))
            .unwrap(),
        );
        sheet.buses.push(Bus {
            uuid: Uuid::new_v4(),
            start: at(5.0, 10.0),
            end: at(15.0, 10.0),
        });
        sheet.junctions.push(
            serde_json::from_str(&format!(
                r#"{{"uuid":"{}","position":{{"x":10.0,"y":5.0}},"diameter":0.5,"minted":true,"unknown_columns":{foreign}}}"#,
                Uuid::new_v4()
            ))
            .unwrap(),
        );
        sheet.no_connects.push(NoConnect {
            uuid: Uuid::new_v4(),
            position: at(20.0, 5.0),
        });
        sheet.text_notes.push(TextNote {
            uuid: Uuid::new_v4(),
            text: "note\twith tab".to_owned(),
            position: at(5.0, 15.0),
            rotation: 90.0,
            font_size: 2.5,
            font_family: Some("Iosevka".to_owned()),
            justify_h: Default::default(),
            justify_v: Default::default(),
            locked: true,
        });
        sheet.labels.push(
            serde_json::from_str(&format!(
                r#"{{"uuid":"{}","text":"D0","position":{{"x":15.0,"y":5.0}},"rotation":0.0,"label_type":"net","shape":"","font_size":1.27,"unknown_columns":{foreign}}}"#,
                Uuid::new_v4()
            ))
            .unwrap(),
        );
        let line = SchDrawing::Line {
            uuid: Uuid::new_v4(),
            start: at(0.0, 0.0),
            end: at(25.0, 0.0),
            width: 0.5,
            stroke_color: None,
            locked: true,
        };
        let poly = SchDrawing::Polyline {
            uuid: Uuid::new_v4(),
            points: vec![at(0.0, 20.0), at(10.0, 25.0), at(20.0, 20.0)],
            width: 0.25,
            fill: Default::default(),
            stroke_color: None,
            locked: false,
        };
        sheet.drawing_placements.insert(
            poly_uuid(&poly),
            signex_types::schematic::DrawingPlacement::Above,
        );
        sheet.drawings.extend([line, poly]);
        sheet
    }

    fn poly_uuid(drawing: &signex_types::schematic::SchDrawing) -> Uuid {
        match drawing {
            signex_types::schematic::SchDrawing::Polyline { uuid, .. } => *uuid,
            _ => unreachable!("a polyline"),
        }
    }

    #[test]
    fn every_kind_of_item_survives_the_clipboard_text() {
        let original = CopiedSelection::from_fragment(rich_fragment(), 2.5)
            .unwrap()
            .unwrap();
        let text = original.to_clipboard_text().unwrap();
        let back = CopiedSelection::from_clipboard_text(&text, 2.5)
            .unwrap()
            .unwrap();
        let (a, b) = (original.payload.fragment(), back.payload.fragment());
        assert_eq!(b.wires[0].unknown_columns.get("x_vendor"), Some("v1"));
        assert_eq!(a.wires, b.wires, "wires with their foreign columns");
        assert_eq!(a.buses, b.buses);
        assert_eq!(
            a.junctions, b.junctions,
            "diameter, minted, foreign columns"
        );
        assert_eq!(a.no_connects, b.no_connects);
        assert_eq!(a.text_notes, b.text_notes);
        assert_eq!(a.labels, b.labels);
        assert_eq!(a.drawings, b.drawings);
        assert_eq!(a.drawing_placements, b.drawing_placements);
        assert_eq!(original.origin, back.origin);
    }
}
