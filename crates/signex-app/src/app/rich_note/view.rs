//! The in-place editor's view (mockup row 203), drawn over the sheet above
//! the note in Graphite Dark in both themes: the title line, the toolbar,
//! the Typst source with line numbers and the notebook editor's colours on
//! the left, the note as it will draw on the right, the first error under
//! the source, and the key hint with Cancel and Apply. Pure: reads the
//! editor state and emits messages.

use iced::advanced::text::highlighter;
use iced::widget::text::LineHeight;
use iced::widget::text_editor::{Binding, KeyPress};
use iced::widget::{Column, Space, column, container, row, text_editor};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_notebook::Token;
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::{button, text};

use super::editor::editor_id;
use super::preview::NotePreview;
use super::{RichNoteEditor, RichNoteMsg, Wrap};
use crate::app::notebook_tab::highlighter::NotebookHighlighter;
use crate::app::{Message, Signex};
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The editor's size on screen, in logical pixels.
pub(super) const EDITOR_WIDTH: f32 = 720.0;
pub(super) const EDITOR_HEIGHT: f32 = 380.0;
/// The gap between the editor and the note it edits.
const GAP: f32 = 12.0;
/// Line height of the source and its gutter, in em of the text size.
const LINE_EM: f32 = 1.3;

/// The label of each toolbar button.
pub(super) fn wrap_label(wrap: Wrap) -> MessageId {
    match wrap {
        Wrap::Bold => ids::OVERLAY_RICH_NOTE_WRAP_BOLD,
        Wrap::Italic => ids::OVERLAY_RICH_NOTE_WRAP_ITALIC,
        Wrap::Heading => ids::OVERLAY_RICH_NOTE_WRAP_HEADING,
        Wrap::List => ids::OVERLAY_RICH_NOTE_WRAP_LIST,
        Wrap::Math => ids::OVERLAY_RICH_NOTE_WRAP_MATH,
        Wrap::Link => ids::OVERLAY_RICH_NOTE_WRAP_LINK,
        Wrap::Table => ids::OVERLAY_RICH_NOTE_WRAP_TABLE,
    }
}

/// Where the editor's top-left goes: above the note, else below it when the
/// canvas has no room above; kept inside the canvas.
#[must_use]
pub(super) fn editor_origin(anchor: (f32, f32), canvas: iced::Rectangle) -> (f32, f32) {
    let (x, y) = anchor;
    let above = y - GAP - EDITOR_HEIGHT;
    let top = if above >= canvas.y { above } else { y + GAP };
    let right = canvas.x + canvas.width - EDITOR_WIDTH;
    (x.min(right).max(canvas.x), top.max(canvas.y))
}

impl Signex {
    /// The rich note editor over the sheet in `window_id`, while it edits a
    /// note of that sheet.
    pub(in crate::app) fn rich_note_editor_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let editor = self.ui_state.rich_notes.editor.as_ref()?;
        let sheet = self
            .document_state
            .sheet_path_for_window(window_id, &self.ui_state)?;
        if editor.sheet.as_ref() != Some(sheet) {
            return None;
        }
        let engine = self
            .document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let note = engine
            .document()
            .text_notes
            .iter()
            .find(|n| n.uuid == editor.note)?;
        let camera = self
            .interaction_state
            .canvas_for_window(window_id)
            .live_camera();
        let anchor = crate::app::view::canvas_region::world_to_window(
            (note.position.x, note.position.y),
            camera,
            self.canvas_bounds(),
        );
        let (left, top) = editor_origin(anchor, self.canvas_bounds());
        let name = sheet
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let title = t!(
            self.ui_state.i18n,
            ids::OVERLAY_RICH_NOTE_TITLE,
            "sheet" => name,
            "x" => format!("{:.2}", note.position.x),
            "y" => format!("{:.2}", note.position.y)
        );
        let card = editor_card(
            &self.ui_state.i18n,
            editor,
            title,
            note.rich.text_width_mm(),
        );
        Some(
            column![
                Space::new().height(top.max(0.0)),
                row![Space::new().width(left.max(0.0)), card],
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        )
    }
}

/// The editor card itself: what the layout probe measures.
pub(super) fn editor_card<'a>(
    i18n: &'a I18n,
    editor: &'a RichNoteEditor,
    title: signex_i18n::Localized,
    width_mm: Option<f64>,
) -> Element<'a, Message> {
    // Mockup row 203: the editor is Graphite Dark whatever the page theme.
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let r = resolve(&tokens);
    let header = row![
        text(title).size(TS.dense).color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        text(compile_note(i18n, editor))
            .size(TS.caption)
            .color(r.ink3),
    ]
    .align_y(Alignment::Center);
    let body = row![
        container(source(editor, &tokens))
            .width(Length::FillPortion(1))
            .height(Length::Fill),
        container(preview(i18n, editor, &tokens, width_mm))
            .width(Length::FillPortion(1))
            .height(Length::Fill),
    ]
    .spacing(SP.s)
    .height(Length::Fill);
    let mut content: Column<Element<'a, Message>> =
        column![header, toolbar(i18n, &tokens), body].spacing(SP.s);
    if let Some(error) = editor.preview.as_ref().and_then(|p| p.first_error()) {
        let line = t!(
            i18n,
            ids::OVERLAY_RICH_NOTE_ERROR,
            "line" => error.line,
            "message" => error.message.clone()
        );
        content = content.push(text(line).size(TS.caption).color(ti(tokens.error)));
    }
    content = content.push(footer(i18n, &tokens));
    let (raised, line) = (r.raised, ti(tokens.border));
    container(content)
        .padding(SP.m)
        .width(EDITOR_WIDTH)
        .height(EDITOR_HEIGHT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(raised)),
            border: Border {
                width: 1.0,
                color: line,
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// `4 ms`, or Compiling… before the first compile of the typed text.
fn compile_note(i18n: &I18n, editor: &RichNoteEditor) -> signex_i18n::Localized {
    match &editor.preview {
        Some(outcome) => t!(i18n, ids::OVERLAY_RICH_NOTE_COMPILE_MS, "ms" => outcome.elapsed_ms),
        None => t!(i18n, ids::OVERLAY_RICH_NOTE_COMPILING),
    }
}

/// Bold, Italic, Heading, List, Maths, Link, Table.
fn toolbar<'a>(i18n: &'a I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut bar = row![].spacing(SP.xs);
    for wrap in Wrap::ALL {
        bar = bar.push(
            button(text(t!(i18n, wrap_label(wrap))).size(TS.dense))
                .padding([SP.xs, SP.s])
                .style(crate::styles::ghost_button(tokens))
                .on_press(Message::from(RichNoteMsg::WrapPressed(wrap))),
        );
    }
    bar.boxed()
}

/// The source: line numbers, then the highlighted editor. Ctrl+Enter
/// applies, Esc cancels; every other key keeps the editor's own binding.
fn source<'a>(editor: &'a RichNoteEditor, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = resolve(tokens);
    let colours = Colours::of(tokens);
    let (value, selection, placeholder) = (ti(tokens.text), ti(tokens.selection), r.ink4);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let error_line = editor
        .preview
        .as_ref()
        .and_then(|p| p.first_error())
        .map(|e| e.line as usize);
    let mut gutter: Column<Element<'a, Message>> = column![].padding([0.0, SP.s]);
    for line in 1..=editor.content.line_count().max(1) {
        let ink = if Some(line) == error_line {
            ti(tokens.error)
        } else {
            r.ink4
        };
        gutter = gutter.push(
            text(UserText::new(line.to_string()))
                .size(TS.body)
                .line_height(LineHeight::Relative(LINE_EM))
                .font(mono)
                .color(ink),
        );
    }
    let input = text_editor(&editor.content)
        .id(editor_id())
        .on_action(|action| Message::from(RichNoteMsg::EditorActed(action)))
        .key_binding(editor_binding)
        .font(mono)
        .size(TS.body)
        .line_height(LineHeight::Relative(LINE_EM))
        .height(Length::Fill)
        .style(move |_: &Theme, _| text_editor::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder,
            value,
            selection,
        })
        .highlight_with::<NotebookHighlighter>(
            editor.highlights.clone(),
            move |token: Token, _: &Theme| colours.style(token),
        );
    let field = r.field;
    container(row![gutter, input].height(Length::Fill))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(field)),
            ..container::Style::default()
        })
        .height(Length::Fill)
        .boxed()
}

/// Ctrl+Enter applies and Esc cancels.
fn editor_binding(key: KeyPress) -> Option<Binding<Message>> {
    use iced::keyboard::{Key, key::Named};
    let named = |name: Named| matches!(key.key.as_ref(), Key::Named(n) if n == name);
    if key.is_focused && named(Named::Enter) && key.modifiers.command() {
        return Some(Binding::Custom(Message::from(RichNoteMsg::ApplyPressed)));
    }
    if key.is_focused && named(Named::Escape) {
        return Some(Binding::Custom(Message::from(RichNoteMsg::CancelPressed)));
    }
    Binding::from_key_press(key)
}

/// The note as it will draw, at the preview's width, with its width named.
fn preview<'a>(
    i18n: &'a I18n,
    editor: &'a RichNoteEditor,
    tokens: &ThemeTokens,
    width_mm: Option<f64>,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let mut col: Column<Element<'a, Message>> = column![].spacing(SP.xs);
    if let Some(drawing) = editor.shown_drawing() {
        let shown_mm = width_mm.unwrap_or(drawing.width_mm);
        let caption = t!(i18n, ids::OVERLAY_RICH_NOTE_PREVIEW, "width" => format!("{shown_mm:.2}"));
        col = col.push(text(caption).size(TS.caption).color(r.ink3));
        let px_per_mm = preview_scale(drawing.width_mm.max(shown_mm));
        let size = (
            (drawing.width_mm as f32 * px_per_mm).max(1.0),
            (drawing.height_mm as f32 * px_per_mm).max(1.0),
        );
        col = col.push(
            iced::widget::canvas(NotePreview {
                drawing,
                ink: ti(tokens.text),
                px_per_mm,
            })
            .width(size.0)
            .height(size.1),
        );
    }
    iced::widget::scrollable(col).height(Length::Fill).boxed()
}

/// Pixels per mm so a note `width_mm` wide fills the preview half.
fn preview_scale(width_mm: f64) -> f32 {
    let half = EDITOR_WIDTH / 2.0 - 2.0 * SP.m;
    if width_mm > 0.0 {
        (half / width_mm as f32).clamp(1.0, 12.0)
    } else {
        4.0
    }
}

/// The key hint, then Cancel and Apply.
fn footer<'a>(i18n: &'a I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = resolve(tokens);
    let hint = t!(
        i18n,
        ids::OVERLAY_RICH_NOTE_HINT,
        "apply" => crate::keymap::shown_in("Mod+Enter", i18n),
        "click" => crate::keymap::shown_in("Mod+Click", i18n)
    );
    row![
        text(hint).size(TS.caption).color(r.ink3),
        Space::new().width(Length::Fill),
        button(text(t!(i18n, ids::OVERLAY_RICH_NOTE_CANCEL)).size(TS.dense))
            .padding([SP.xs, SP.m])
            .style(crate::styles::ghost_button(tokens))
            .on_press(Message::from(RichNoteMsg::CancelPressed)),
        button(text(t!(i18n, ids::OVERLAY_RICH_NOTE_APPLY)).size(TS.dense))
            .padding([SP.xs, SP.m])
            .style(crate::styles::primary_button(tokens))
            .on_press(Message::from(RichNoteMsg::ApplyPressed)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// The source colours, as the notebook editor picks them.
#[derive(Clone, Copy)]
struct Colours {
    text: Color,
    accent: Color,
    string: Color,
    number: Color,
    math: Color,
    comment: Color,
}

impl Colours {
    fn of(tokens: &ThemeTokens) -> Self {
        let r = resolve(tokens);
        Self {
            text: ti(tokens.text),
            accent: r.accent_text,
            string: ti(tokens.success),
            number: r.info,
            math: ti(tokens.warning),
            comment: r.ink4,
        }
    }

    fn style(self, token: Token) -> highlighter::Style {
        let (color, font) = match token {
            Token::Heading => (self.text, None),
            Token::Function | Token::Reference | Token::Keyword => (self.accent, None),
            Token::String | Token::Raw => (self.string, None),
            Token::Number | Token::Label => (self.number, None),
            Token::Math => (self.math, None),
            Token::Comment => (self.comment, Some(iced::font::Style::Italic)),
            Token::Emphasis => (self.text, Some(iced::font::Style::Italic)),
        };
        highlighter::Style {
            color: Some(color),
            style: font,
        }
    }
}
