//! `Command::UpdateTextNoteProps`: a text note's justification, turn, text
//! size and format (plain or rich, #1114), as its Properties rows set them.

use crate::*;

impl Engine {
    /// Set what `props` names on the note `note_id`: one history entry, none
    /// when the note is missing or nothing changes.
    pub(crate) fn update_text_note_props(
        &mut self,
        before: SchematicSheet,
        note_id: uuid::Uuid,
        props: TextNoteProps,
    ) -> CommandResult {
        let Some(note) = (self.document.text_notes.iter_mut()).find(|n| n.uuid == note_id) else {
            return CommandResult::unchanged();
        };
        let mut changed = false;
        if let Some(h) = props.justify_h
            && note.justify_h != h
        {
            note.justify_h = h;
            changed = true;
        }
        if let Some(v) = props.justify_v
            && note.justify_v != v
        {
            note.justify_v = v;
            changed = true;
        }
        if let Some(degrees) = props.rotation_degrees.filter(|d| d.is_finite()) {
            let degrees = degrees.rem_euclid(360.0);
            if (note.rotation - degrees).abs() > 1e-6 {
                note.rotation = degrees;
                changed = true;
            }
        }
        if let Some(size) = props.font_size_mm.filter(|s| s.is_finite() && *s > 0.0)
            && (note.font_size - size).abs() > 1e-6
        {
            note.font_size = size;
            changed = true;
        }
        if let Some(format) = props.format
            && note.format != format
        {
            note.format = format;
            changed = true;
        }
        if let Some(rich) = props.rich.filter(lengths_are_valid)
            && note.rich != rich
        {
            note.rich = rich;
            changed = true;
        }
        if !changed {
            return CommandResult::unchanged();
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::TextUpdated,
            document: DocumentPatch::TEXT_NOTES,
        };
        self.record_history(before, patch_pair);
        CommandResult::changed(patch_pair)
    }
}

/// Every length of `rich` is a finite number at or above zero.
fn lengths_are_valid(rich: &signex_types::schematic::RichNoteLayout) -> bool {
    let length = |mm: f64| mm.is_finite() && mm >= 0.0;
    rich.border_mm.is_none_or(length)
        && length(rich.padding_mm)
        && length(rich.width_mm)
        && length(rich.height_mm)
}

#[cfg(test)]
mod tests {
    use crate::{Command, Engine, TextNoteProps};
    use signex_types::schematic::{HAlign, Point, SchematicSheet, TextNote, VAlign};

    fn engine_with_note() -> (Engine, uuid::Uuid) {
        let note = TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: "n".into(),
            position: Point::new(1.0, 1.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
            format: Default::default(),
            rich: Default::default(),
        };
        let id = note.uuid;
        let sheet = SchematicSheet {
            text_notes: vec![note],
            ..Default::default()
        };
        (Engine::new(sheet).expect("engine"), id)
    }

    #[test]
    fn each_prop_is_one_undo_step_and_none_keeps_the_rest() {
        let (mut engine, id) = engine_with_note();
        let props = TextNoteProps {
            justify_h: Some(HAlign::Right),
            rotation_degrees: Some(-90.0),
            ..Default::default()
        };
        let done = engine
            .execute(Command::UpdateTextNoteProps { note_id: id, props })
            .expect("applied");
        assert!(done.changed);
        let note = &engine.document().text_notes[0];
        assert_eq!(note.justify_h, HAlign::Right);
        assert_eq!(note.justify_v, VAlign::Top, "None keeps it");
        assert!((note.rotation - 270.0).abs() < 1e-9);
        assert!((note.font_size - 1.27).abs() < 1e-9);
        engine.undo().expect("one step back");
        let note = &engine.document().text_notes[0];
        assert_eq!(note.justify_h, HAlign::Left);
        assert!(note.rotation.abs() < 1e-9);
    }

    /// #1114: Rich text is one undo step and keeps the text as it is; the
    /// format the note already has records nothing.
    #[test]
    fn the_format_is_one_undo_step_and_keeps_the_text() {
        use signex_types::schematic::TextNoteFormat;
        let (mut engine, id) = engine_with_note();
        let rich = TextNoteProps {
            format: Some(TextNoteFormat::Typst),
            ..Default::default()
        };
        let done = engine
            .execute(Command::UpdateTextNoteProps {
                note_id: id,
                props: rich,
            })
            .expect("applied");
        assert!(done.changed);
        let note = &engine.document().text_notes[0];
        assert_eq!(note.format, TextNoteFormat::Typst);
        assert_eq!(note.text, "n");
        let again = engine
            .execute(Command::UpdateTextNoteProps {
                note_id: id,
                props: rich,
            })
            .expect("ran");
        assert!(!again.changed);
        engine.undo().expect("one step back");
        assert_eq!(
            engine.document().text_notes[0].format,
            TextNoteFormat::Plain
        );
        assert!(!engine.can_undo(), "one step only");
    }

    /// #1114, mockup row 203: the Frame and Size sections are one undo step;
    /// a length that is not a finite length is refused whole.
    #[test]
    fn the_frame_and_size_are_one_undo_step_and_a_bad_length_is_refused() {
        use signex_types::schematic::{NoteFrame, NoteSize, RichNoteLayout};
        let (mut engine, id) = engine_with_note();
        let layout = RichNoteLayout {
            frame: NoteFrame::Border,
            size: NoteSize::FixedWidth,
            width_mm: 40.0,
            ..RichNoteLayout::default()
        };
        let props = TextNoteProps {
            rich: Some(layout),
            ..Default::default()
        };
        let done = engine
            .execute(Command::UpdateTextNoteProps { note_id: id, props })
            .expect("applied");
        assert!(done.changed);
        assert_eq!(engine.document().text_notes[0].rich, layout);
        assert_eq!(engine.document().text_notes[0].text, "n");
        let bad = TextNoteProps {
            rich: Some(RichNoteLayout {
                width_mm: f64::INFINITY,
                ..layout
            }),
            ..Default::default()
        };
        let refused = engine
            .execute(Command::UpdateTextNoteProps {
                note_id: id,
                props: bad,
            })
            .expect("ran");
        assert!(!refused.changed);
        assert_eq!(engine.document().text_notes[0].rich, layout);
        engine.undo().expect("one step back");
        assert!(engine.document().text_notes[0].rich.is_default());
        assert!(!engine.can_undo(), "one step only");
    }

    #[test]
    fn an_unchanged_or_bad_value_records_nothing() {
        let (mut engine, id) = engine_with_note();
        for props in [
            TextNoteProps {
                justify_h: Some(HAlign::Left),
                ..Default::default()
            },
            TextNoteProps {
                font_size_mm: Some(0.0),
                ..Default::default()
            },
            TextNoteProps {
                rotation_degrees: Some(f64::NAN),
                ..Default::default()
            },
        ] {
            let done = engine
                .execute(Command::UpdateTextNoteProps { note_id: id, props })
                .expect("ran");
            assert!(!done.changed, "{props:?}");
        }
        assert!(!engine.can_undo());
    }
}
