//! The Libraries section view (mockup row 33): the libraries in list order with
//! their location, kind and state pill, the move buttons on row hover, the
//! line that says where libraries are added and removed, and Import library…,
//! which opens the Import Library dialog for this project (#3122).

use iced::widget::{Space, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::project::ListMove;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{ButtonKind, Glyph, ICON, glyph, info_line, mono_font, pill, text_button};
use super::super::row_drag::{RowDrag, RowKey, grip_column, grip_of};
use super::super::table::{SP, TS, TableRow, grid_table, icon_move_buttons, section, word_cell};
use super::{LibraryMount, LibraryRow, kind_words, path_tail};
use crate::app::Message;
use crate::app::state::ProjectId;
use crate::styles::{ti, tokens::resolve};

/// Width of the Kind column (110 px).
const KIND_W: f32 = TS.body * 9.2;
/// Width of the State column (150 px).
const STATE_W: f32 = TS.body * 12.5;
/// Width of the Move up / Move down column (80 px).
const ACTIONS_W: f32 = TS.body * 6.7;
/// Most characters a location shows before it is cut from the start.
pub(super) const LOCATION_CHARS: usize = 48;

pub(in crate::app::project_tab) fn view<'a>(
    project_id: ProjectId,
    rows: &[LibraryRow],
    drag: Option<&RowDrag>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut body = if rows.is_empty() {
        vec![where_line(ids::PROJECT_LIBRARIES_EMPTY, i18n, tokens)]
    } else {
        vec![
            library_table(project_id, rows, drag, i18n, tokens),
            where_line(ids::PROJECT_LIBRARIES_WHERE, i18n, tokens),
        ]
    };
    body.push(import_button(project_id, i18n, tokens));
    section(
        ids::PROJECT_SECTION_LIBRARIES,
        ids::PROJECT_LIBRARIES_HELP,
        body,
        i18n,
        tokens,
    )
}

/// Import library…: the Import Library dialog, mounting for this project.
fn import_button<'a>(
    project_id: ProjectId,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let open = crate::library::import_dialog::ImportDialogMsg::RequestedForProject(project_id);
    let message = Message::Library(crate::library::import_dialog::msg(open));
    text_button(
        None,
        t!(i18n, ids::PROJECT_LIBRARIES_IMPORT),
        Some(message),
        ButtonKind::Small,
        tokens,
    )
}

/// The info line that sends adding, removing and renaming to the tree.
fn where_line<'a>(
    words: signex_i18n::MessageId,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    info_line(text(t!(i18n, words)).size(TS.body).boxed(), tokens)
}

fn library_table<'a>(
    project_id: ProjectId,
    rows: &[LibraryRow],
    drag: Option<&RowDrag>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let columns = [
        grip_column(),
        (
            Some(ids::PROJECT_LIBRARIES_COL_NAME),
            Length::FillPortion(2),
        ),
        (
            Some(ids::PROJECT_LIBRARIES_COL_LOCATION),
            Length::FillPortion(3),
        ),
        (Some(ids::PROJECT_LIBRARIES_COL_KIND), Length::Fixed(KIND_W)),
        (
            Some(ids::PROJECT_LIBRARIES_COL_STATE),
            Length::Fixed(STATE_W),
        ),
    ];
    let lines = rows
        .iter()
        .enumerate()
        .map(|(index, library)| {
            let path = library.stored.clone();
            let key = RowKey::Library(path.clone());
            let moved = move |step: ListMove| {
                tab_message(ProjectTabMsg::LibraryMoved {
                    project_id,
                    path: path.clone(),
                    step,
                })
            };
            TableRow {
                cells: vec![
                    Space::new().boxed(),
                    name_cell(library, tokens),
                    location_cell(library, tokens),
                    word_cell(t!(i18n, kind_words(library.kind)), tokens),
                    state_cell(project_id, library, i18n, tokens),
                ],
                actions: Some(icon_move_buttons(index, rows.len(), moved, i18n, tokens)),
                is_error: false,
                grip: Some(grip_of(project_id, key, index, drag)),
            }
        })
        .collect();
    grid_table(
        &columns,
        Some(Length::Fixed(ACTIONS_W)),
        lines,
        i18n,
        tokens,
    )
}

/// The library glyph and the name; a missing library shows the warning
/// glyph in the error ink instead.
fn name_cell<'a>(library: &LibraryRow, tokens: &ThemeTokens) -> Element<'a, Message> {
    let mark = match library.mount {
        LibraryMount::Missing => glyph(Glyph::Warn, ICON, ti(tokens.error)),
        _ => glyph(Glyph::Library, ICON, resolve(tokens).ink3),
    };
    row![
        mark,
        text(UserText::new(library.name()))
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
    ]
    .spacing(SP.s + SP.xs * 0.5)
    .align_y(Alignment::Center)
    .boxed()
}

/// The path as stored, mono, cut from the start so the file name stays.
fn location_cell<'a>(library: &LibraryRow, tokens: &ThemeTokens) -> Element<'a, Message> {
    let stored = library.stored.display().to_string();
    text(UserText::new(path_tail(&stored, LOCATION_CHARS)))
        .size(TS.body)
        .font(mono_font())
        .color(ti(tokens.text_secondary))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::Start)
        .boxed()
}

/// The State cell: the pill, and Locate… on a Missing row (mockup row 33).
fn state_cell<'a>(
    project_id: ProjectId,
    library: &LibraryRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let state = state_pill(library.mount, i18n, tokens);
    if library.mount != LibraryMount::Missing {
        return state;
    }
    let locate = text_button(
        None,
        t!(i18n, ids::PROJECT_LIBRARIES_LOCATE),
        Some(tab_message(ProjectTabMsg::LibraryLocateRequested {
            project_id,
            missing: library.resolved.clone(),
        })),
        ButtonKind::Ghost,
        tokens,
    );
    row![state, locate]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
}

/// The mount state as a pill: Mounted ok, read-only neutral with a lock,
/// Missing error, Not mounted neutral.
fn state_pill<'a>(mount: LibraryMount, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let lock = (mount == LibraryMount::ReadOnly).then_some(Glyph::Lock);
    pill(t!(i18n, mount.words()), mount.tone(), lock, tokens)
}
