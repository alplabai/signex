//! The editable Position, Rotation, Size and Parameters of one placed object
//! in Properties (#2418): its messages and the state the view reads. A box
//! keeps what is typed as a draft and commits on Enter, so one edit is one
//! undo step; the commit runs the same app method as the `set_object_*`
//! commands (`app/command/placed_operands.rs`).

use std::path::{Path, PathBuf};

use iced::widget::text_editor;
use signex_types::schematic::{
    HAlign, Point, RichNoteLayout, SelectedItem, TextNoteFormat, VAlign,
};

/// One editable box of a placed object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlacedField {
    /// The position's X, in the panel's unit; a wire's or bus's start X.
    X,
    /// The position's Y, in the panel's unit, counted from the page origin;
    /// a wire's or bus's start Y.
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
    /// A length of a rich text note's Frame or Size section (#1114).
    NoteLength(NoteLength),
}

/// The lengths of a rich note's Frame and Size sections (mockup row 203).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteLength {
    /// The outline width; empty or `auto` is Auto.
    Border,
    Padding,
    /// The box width of Fixed width and Fixed box.
    Width,
    /// The box height of Fixed box.
    Height,
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
    /// The Text box lost focus or Ctrl+Enter was pressed in it: Escape, or
    /// a press outside the box. The draft becomes one edit.
    NoteSubmitted { item: SelectedItem },
    /// A pick in a text note's Rotation or Justify rows.
    NoteLayoutPicked {
        item: SelectedItem,
        layout: NoteLayout,
    },
    /// A keystroke in a power port's Name box: kept until Enter or until the
    /// port is left, so a rename is one undo step (mockup row 45).
    PortNameTyped { item: SelectedItem, text: String },
    /// Enter in a power port's Name box.
    PortNameSubmitted { item: SelectedItem },
    /// Edit… beside a rich note's Text: its in-place editor (mockup row 203).
    RichNoteEditRequested { item: SelectedItem },
}

/// What a text note's Rotation, Format and Justify rows set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteLayout {
    JustifyH(HAlign),
    JustifyV(VAlign),
    /// Stored degrees, counter-clockwise.
    Rotation(f64),
    /// Plain or Rich text (#1114, mockup row 203).
    Format(TextNoteFormat),
    /// A pick in a rich note's Frame or Size section: the sections as they
    /// become.
    Rich(RichNoteLayout),
}

impl NoteLayout {
    /// The engine's note properties this pick sets.
    pub fn props(self) -> signex_engine::TextNoteProps {
        let none = signex_engine::TextNoteProps::default();
        match self {
            Self::JustifyH(h) => signex_engine::TextNoteProps {
                justify_h: Some(h),
                ..none
            },
            Self::JustifyV(v) => signex_engine::TextNoteProps {
                justify_v: Some(v),
                ..none
            },
            Self::Rotation(degrees) => signex_engine::TextNoteProps {
                rotation_degrees: Some(degrees),
                ..none
            },
            Self::Format(format) => signex_engine::TextNoteProps {
                format: Some(format),
                ..none
            },
            Self::Rich(layout) => signex_engine::TextNoteProps {
                rich: Some(layout),
                ..none
            },
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

/// A power port's Name box while typed: the sheet and port it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct PortNameDraft {
    pub sheet: PathBuf,
    pub item: SelectedItem,
    pub text: String,
}

/// The multi-line Text box of a text note: the sheet and note it belongs to,
/// its content, and whether it was typed in since the note's text was last
/// applied. A copied sheet can carry the same note id, so the sheet is part
/// of what the draft belongs to.
#[derive(Debug, Clone)]
pub struct NoteDraft {
    pub sheet: PathBuf,
    pub item: SelectedItem,
    pub content: text_editor::Content,
    pub edited: bool,
}

impl NoteDraft {
    /// Lines the box grows to before it scrolls.
    pub const MAX_VISIBLE_LINES: usize = 6;

    pub fn new(sheet: &Path, item: SelectedItem, text: &str) -> Self {
        Self {
            sheet: sheet.to_path_buf(),
            item,
            content: text_editor::Content::with_text(text),
            edited: false,
        }
    }

    /// Whether this is the box of the note `item` on `sheet`.
    pub fn is_of(&self, sheet: &Path, item: SelectedItem) -> bool {
        self.item == item && self.sheet == sheet
    }

    /// The lines the box shows: one per line of text, at most
    /// [`Self::MAX_VISIBLE_LINES`].
    pub fn visible_lines(&self) -> usize {
        self.content.line_count().clamp(1, Self::MAX_VISIBLE_LINES)
    }
}

impl PartialEq for NoteDraft {
    fn eq(&self, other: &Self) -> bool {
        self.sheet == other.sheet
            && self.item == other.item
            && self.edited == other.edited
            && self.content.text() == other.content.text()
    }
}

/// The selected text note as the box is synced to it: its sheet, the note
/// and its stored text.
pub type SelectedNote<'a> = (&'a Path, SelectedItem, &'a str);

/// The net a selected wire or bus is on, as the project's net index names it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunNet {
    pub name: String,
    /// The label or power port that names the net; `None` for a net with an
    /// automatic name.
    pub named_by: Option<signex_net::NetAnchor>,
}

/// What the Properties rows of the single selected object read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlacedPanel {
    /// The object the rows edit, when it has an editable position.
    pub item: Option<SelectedItem>,
    /// The net of a selected wire or bus, once the net index has answered.
    pub run_net: Option<RunNet>,
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
    /// Typed note boxes whose note is no longer selected: owed to their own
    /// sheets, applied by the update loop (`Signex::settle_owed_note_drafts`).
    pub notes_owed: Vec<NoteDraft>,
    /// The text size of the selected text note, in stored mm.
    pub note_size_mm: Option<f64>,
    /// The Frame and Size sections of the selected text note (#1114).
    pub note_rich: Option<RichNoteLayout>,
    /// The selected power port's Name box, while typed.
    pub port_name: Option<PortNameDraft>,
    /// Typed names whose port is no longer selected: owed to their sheets,
    /// applied by the update loop like [`Self::notes_owed`].
    pub port_names_owed: Vec<PortNameDraft>,
}

impl PlacedPanel {
    /// The Name box of the port `item`: what is typed, else `stored`.
    pub fn port_name_text(&self, item: SelectedItem, stored: &str) -> String {
        match &self.port_name {
            Some(draft) if draft.item == item => draft.text.clone(),
            _ => stored.to_string(),
        }
    }

    /// Owe a typed name once its port is no longer the selected object on
    /// `sheet`. Nothing is written here.
    pub fn sync_port_name(&mut self, sheet: Option<&Path>, selected: Option<SelectedItem>) {
        let is_kept = self
            .port_name
            .as_ref()
            .is_none_or(|d| Some(d.item) == selected && Some(d.sheet.as_path()) == sheet);
        if !is_kept {
            self.port_names_owed.extend(self.port_name.take());
        }
    }

    /// Take the typed names of the sheets `matches` picks: the owed ones and,
    /// with `selected`, the one still in the box. Each comes back once.
    pub fn take_typed_port_names(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        selected: bool,
    ) -> Vec<PortNameDraft> {
        let owed = std::mem::take(&mut self.port_names_owed);
        let (mut taken, kept): (Vec<_>, Vec<_>) = owed.into_iter().partition(|d| matches(&d.sheet));
        self.port_names_owed = kept;
        if selected && self.port_name.as_ref().is_some_and(|d| matches(&d.sheet)) {
            taken.extend(self.port_name.take());
        }
        taken
    }

    /// The text of `field` of `item` while it is being typed, else `shown`.
    pub fn text(&self, item: SelectedItem, field: &PlacedField, shown: String) -> String {
        match &self.draft {
            Some(d) if d.item == item && &d.field == field => d.text.clone(),
            _ => shown,
        }
    }

    /// Keep the note box in step with `selected`. The box is seeded again
    /// when another note (or the same id on another sheet) is selected, or
    /// when the stored text changed (an undo) while nothing was typed. A
    /// typed box whose note is left is kept in [`Self::notes_owed`], never
    /// dropped and never written here.
    pub fn sync_note(&mut self, selected: Option<SelectedNote<'_>>) {
        let keeps = match (&self.note, selected) {
            (Some(draft), Some((sheet, item, text))) if draft.is_of(sheet, item) => {
                draft.edited || draft.content.text() == text
            }
            _ => false,
        };
        if keeps {
            return;
        }
        let previous = self.note.take();
        self.note = selected.map(|(sheet, item, text)| NoteDraft::new(sheet, item, text));
        let left = previous.filter(|draft| {
            draft.edited && selected.is_none_or(|(sheet, item, _)| !draft.is_of(sheet, item))
        });
        self.notes_owed.extend(left);
    }

    /// Take the typed note boxes of the sheets `matches` picks: the owed
    /// ones, and the selected note's box when it was typed in (it stays,
    /// marked applied). Each comes back once.
    pub fn take_typed_notes(&mut self, matches: &dyn Fn(&Path) -> bool) -> Vec<NoteDraft> {
        let owed = std::mem::take(&mut self.notes_owed);
        let (mut taken, kept): (Vec<_>, Vec<_>) = owed.into_iter().partition(|d| matches(&d.sheet));
        self.notes_owed = kept;
        if let Some(draft) = self.note.as_mut().filter(|d| d.edited && matches(&d.sheet)) {
            draft.edited = false;
            taken.push(draft.clone());
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::SelectedKind;

    fn note(n: u128) -> SelectedItem {
        SelectedItem::new(uuid::Uuid::from_u128(n), SelectedKind::TextNote)
    }

    fn a() -> &'static Path {
        Path::new("a.snxsch")
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
        panel.sync_note(Some((a(), note(1), "a\nb")));
        assert!(panel.notes_owed.is_empty());
        let draft = panel.note.as_ref().expect("a note box");
        assert_eq!(draft.content.text(), "a\nb");
        assert!(!draft.edited);
    }

    #[test]
    fn a_typed_box_survives_a_refresh_of_the_same_note() {
        let mut panel = PlacedPanel::default();
        panel.sync_note(Some((a(), note(1), "a")));
        typed(&mut panel, 'x');
        panel.sync_note(Some((a(), note(1), "a")));
        assert!(panel.notes_owed.is_empty());
        assert!(panel.note.as_ref().is_some_and(|d| d.edited));
    }

    #[test]
    fn leaving_a_typed_note_owes_its_draft_once() {
        let mut panel = PlacedPanel::default();
        panel.sync_note(Some((a(), note(1), "a")));
        typed(&mut panel, 'x');
        panel.sync_note(Some((a(), note(2), "other")));
        let owed: Vec<_> = panel.notes_owed.iter().map(|d| d.item).collect();
        assert_eq!(owed, vec![note(1)]);
        assert_eq!(panel.note.as_ref().map(|d| d.item), Some(note(2)));
        panel.sync_note(None);
        assert_eq!(panel.notes_owed.len(), 1, "note 2 was not typed in");
        assert!(panel.note.is_none());
    }

    /// The same note id on another sheet (a copied sheet) is another note.
    #[test]
    fn the_same_id_on_another_sheet_is_another_box() {
        let mut panel = PlacedPanel::default();
        panel.sync_note(Some((a(), note(1), "a")));
        typed(&mut panel, 'x');
        let b = Path::new("b.snxsch");
        panel.sync_note(Some((b, note(1), "a")));
        let owed = panel.notes_owed.first().expect("owed to a");
        assert_eq!(owed.sheet, a());
        let shown = panel.note.as_ref().expect("b's box");
        assert_eq!((shown.sheet.as_path(), shown.edited), (b, false));
    }

    #[test]
    fn taking_the_typed_notes_marks_the_selected_box_applied() {
        let mut panel = PlacedPanel::default();
        panel.sync_note(Some((a(), note(1), "a")));
        typed(&mut panel, 'x');
        let other = Path::new("b.snxsch");
        assert!(panel.take_typed_notes(&|s| s == other).is_empty());
        let taken = panel.take_typed_notes(&|s| s == a());
        assert_eq!(taken.len(), 1);
        assert!(panel.note.as_ref().is_some_and(|d| !d.edited));
        assert!(panel.take_typed_notes(&|_| true).is_empty(), "once");
    }

    #[test]
    fn an_undo_reseeds_an_untouched_box() {
        let mut panel = PlacedPanel::default();
        panel.sync_note(Some((a(), note(1), "new")));
        panel.sync_note(Some((a(), note(1), "old")));
        let shown = panel.note.as_ref().map(|d| d.content.text());
        assert_eq!(shown.as_deref(), Some("old"));
    }

    fn port(n: u128) -> SelectedItem {
        SelectedItem::new(uuid::Uuid::from_u128(n), SelectedKind::Symbol)
    }

    #[test]
    fn a_typed_port_name_is_owed_once_its_port_is_left() {
        let mut panel = PlacedPanel {
            port_name: Some(PortNameDraft {
                sheet: a().to_path_buf(),
                item: port(1),
                text: "+5V".into(),
            }),
            ..PlacedPanel::default()
        };
        panel.sync_port_name(Some(a()), Some(port(1)));
        assert!(panel.port_names_owed.is_empty(), "still selected");
        assert_eq!(panel.port_name_text(port(1), "+3V3"), "+5V");
        panel.sync_port_name(Some(a()), Some(port(2)));
        assert_eq!(panel.port_name, None);
        assert_eq!(panel.port_name_text(port(1), "+3V3"), "+3V3");
        assert_eq!(panel.take_typed_port_names(&|_| true, false).len(), 1);
        assert!(panel.take_typed_port_names(&|_| true, true).is_empty());
    }

    #[test]
    fn the_box_grows_to_six_lines_then_scrolls() {
        let one = NoteDraft::new(a(), note(1), "a");
        assert_eq!(one.visible_lines(), 1);
        let many = NoteDraft::new(a(), note(1), "1\n2\n3\n4\n5\n6\n7\n8");
        assert_eq!(many.visible_lines(), NoteDraft::MAX_VISIBLE_LINES);
    }
}
