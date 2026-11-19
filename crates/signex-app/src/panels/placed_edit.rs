//! The editable Position, Rotation, Size and Parameters of one placed object
//! in Properties (#2418): its messages and the state the view reads. A box
//! keeps what is typed as a draft and commits on Enter, so one edit is one
//! undo step; the commit runs the same app method as the `set_object_*`
//! commands (`app/command/placed_operands.rs`).

use iced::widget::text_editor;
use signex_types::schematic::{HAlign, Point, SelectedItem, TextNote, VAlign};

/// One editable box of a placed object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlacedField {
    /// The position's X, in the panel's unit.
    X,
    /// The position's Y, in the panel's unit, counted from the page origin.
    Y,
    /// A wire's or bus's end X, in the panel's unit.
    EndX,
    /// A wire's or bus's end Y, in the panel's unit, from the page origin.
    EndY,
    /// A text note's text size, in the panel's unit.
    TextSize,
    /// A sheet symbol's width.
    Width,
    /// A sheet symbol's height.
    Height,
    /// The value of a placed part's parameter, by name.
    Parameter(String),
}

/// What a placed object's Properties rows send.
#[derive(Debug, Clone, PartialEq)]
pub enum PlacedEditMsg {
    /// A keystroke in a box: kept as the draft until Enter.
    Typed {
        item: SelectedItem,
        field: PlacedField,
        text: String,
    },
    /// Enter in a box: the draft becomes one edit.
    Submitted {
        item: SelectedItem,
        field: PlacedField,
    },
    /// A pick in the Rotation row, in stored degrees.
    RotationPicked { item: SelectedItem, degrees: f64 },
    /// An edit, a cursor move or a selection in a text note's Text box: kept
    /// in the note's draft.
    NoteActed {
        item: SelectedItem,
        action: text_editor::Action,
    },
    /// Ctrl+Enter in a text note's Text box, or the pointer left it: the
    /// draft becomes one edit.
    NoteSubmitted { item: SelectedItem },
    /// A pick in a text note's Rotation or Justify rows.
    NoteLayoutPicked {
        item: SelectedItem,
        layout: NoteLayout,
    },
}

/// What a text note's Rotation and Justify rows set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteLayout {
    JustifyH(HAlign),
    JustifyV(VAlign),
    /// Stored degrees, counter-clockwise.
    Rotation(f64),
}

impl NoteLayout {
    /// Set this on `note`.
    pub fn apply(self, note: &mut TextNote) {
        match self {
            Self::JustifyH(h) => note.justify_h = h,
            Self::JustifyV(v) => note.justify_v = v,
            Self::Rotation(degrees) => note.rotation = degrees.rem_euclid(360.0),
        }
    }
}

/// The box being typed in and its text.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedDraft {
    pub item: SelectedItem,
    pub field: PlacedField,
    pub text: String,
}

/// The multi-line Text box of the selected text note: its content and
/// whether it was typed in since the note's text was last applied.
#[derive(Debug, Clone)]
pub struct NoteDraft {
    pub item: SelectedItem,
    pub content: text_editor::Content,
    pub edited: bool,
}

impl NoteDraft {
    /// Lines the box grows to before it scrolls.
    pub const MAX_VISIBLE_LINES: usize = 6;

    pub fn new(item: SelectedItem, text: &str) -> Self {
        Self {
            item,
            content: text_editor::Content::with_text(text),
            edited: false,
        }
    }

    /// The lines the box shows: one per line of text, at most
    /// [`Self::MAX_VISIBLE_LINES`].
    pub fn visible_lines(&self) -> usize {
        self.content.line_count().clamp(1, Self::MAX_VISIBLE_LINES)
    }
}

impl PartialEq for NoteDraft {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
            && self.edited == other.edited
            && self.content.text() == other.content.text()
    }
}

/// What the Properties rows of the single selected object read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlacedPanel {
    /// The object the rows edit, when it has an editable position.
    pub item: Option<SelectedItem>,
    /// Its geometry in stored mm.
    pub geometry: Option<signex_engine::PlacedGeometry>,
    /// The end of a selected wire or bus, in stored mm.
    pub end: Option<Point>,
    /// Why its fields are disabled: the lock that holds it.
    pub locked: Option<String>,
    /// The box being typed in.
    pub draft: Option<PlacedDraft>,
    /// The Text box of the selected text note.
    pub note: Option<NoteDraft>,
    /// The text size of the selected text note, in stored mm.
    pub note_size_mm: Option<f64>,
}

impl PlacedPanel {
    /// The text of `field` of `item` while it is being typed, else `shown`.
    pub fn text(&self, item: SelectedItem, field: &PlacedField, shown: String) -> String {
        match &self.draft {
            Some(d) if d.item == item && &d.field == field => d.text.clone(),
            _ => shown,
        }
    }

    /// Keep the note box in step with `selected`, the selected text note and
    /// its stored text. The box is seeded again when another note is
    /// selected, or when the stored text changed (an undo) while nothing was
    /// typed. Returns a draft typed into a note that is no longer selected,
    /// for the caller to apply, so leaving the box never drops typed text.
    pub fn sync_note(&mut self, selected: Option<(SelectedItem, &str)>) -> Option<NoteDraft> {
        let keeps = match (&self.note, selected) {
            (Some(draft), Some((item, text))) if draft.item == item => {
                draft.edited || draft.content.text() == text
            }
            _ => false,
        };
        if keeps {
            return None;
        }
        let previous = self.note.take();
        self.note = selected.map(|(item, text)| NoteDraft::new(item, text));
        previous.filter(|draft| draft.edited && selected.is_none_or(|(item, _)| item != draft.item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::SelectedKind;

    fn note(n: u128) -> SelectedItem {
        SelectedItem::new(uuid::Uuid::from_u128(n), SelectedKind::TextNote)
    }

    fn typed(panel: &mut PlacedPanel, c: char) {
        let draft = panel.note.as_mut().expect("a note box");
        draft
            .content
            .perform(text_editor::Action::Edit(text_editor::Edit::Insert(c)));
        draft.edited = true;
    }

    #[test]
    fn selecting_a_note_seeds_its_box_with_the_stored_text() {
        let mut panel = PlacedPanel::default();
        assert!(panel.sync_note(Some((note(1), "a\nb"))).is_none());
        let draft = panel.note.as_ref().expect("a note box");
        assert_eq!(draft.content.text(), "a\nb");
        assert!(!draft.edited);
    }

    #[test]
    fn a_typed_box_survives_a_refresh_of_the_same_note() {
        let mut panel = PlacedPanel::default();
        let _ = panel.sync_note(Some((note(1), "a")));
        typed(&mut panel, 'x');
        assert!(panel.sync_note(Some((note(1), "a"))).is_none());
        assert!(panel.note.as_ref().is_some_and(|d| d.edited));
    }

    #[test]
    fn leaving_a_typed_note_hands_its_draft_back_once() {
        let mut panel = PlacedPanel::default();
        let _ = panel.sync_note(Some((note(1), "a")));
        typed(&mut panel, 'x');
        let left = panel.sync_note(Some((note(2), "other")));
        assert_eq!(left.map(|d| d.item), Some(note(1)));
        assert_eq!(panel.note.as_ref().map(|d| d.item), Some(note(2)));
        assert!(panel.sync_note(None).is_none(), "note 2 was not typed in");
        assert!(panel.note.is_none());
    }

    #[test]
    fn an_undo_reseeds_an_untouched_box() {
        let mut panel = PlacedPanel::default();
        let _ = panel.sync_note(Some((note(1), "new")));
        assert!(panel.sync_note(Some((note(1), "old"))).is_none());
        let shown = panel.note.as_ref().map(|d| d.content.text());
        assert_eq!(shown.as_deref(), Some("old"));
    }

    #[test]
    fn the_box_grows_to_six_lines_then_scrolls() {
        let one = NoteDraft::new(note(1), "a");
        assert_eq!(one.visible_lines(), 1);
        let many = NoteDraft::new(note(1), "1\n2\n3\n4\n5\n6\n7\n8");
        assert_eq!(many.visible_lines(), NoteDraft::MAX_VISIBLE_LINES);
    }
}
