//! The Sheets section view (mockup row 31): the page table with its root and
//! missing pills and the move buttons on row hover, or the empty-state line
//! with Add New ▸ Schematic, then the New sheets card (the defaults column
//! beside the preview).

use iced::widget::{Row, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::project::{ListMove, UserProfile};
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::text;

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{
    ButtonKind, CardHeader, Glyph, card, info_line, mono_font, pill, text_button,
};
use super::super::table::{SP, TS, TableRow, grid_table, icon_move_buttons, mono_cell, section};
use super::SheetRow;
use super::preview;
use crate::app::new_project::NewProjectState;
use crate::app::state::LoadedProject;
use crate::app::view::dialogs::new_project::customize;
use crate::app::{Message, ProjectMsg};
use crate::styles::{PillTone, ti, tokens::resolve};

/// Width of the Page column (56 px).
const PAGE_W: f32 = TS.body * 4.7;
/// Width of the Move up / Move down column (90 px).
const ACTIONS_W: f32 = TS.body * 7.5;
/// How much taller the sheet is in the larger preview window.
const POPUP_HEIGHT_FACTOR: f32 = 2.5;

pub(in crate::app::project_tab) fn view<'a>(
    project: &LoadedProject,
    rows: &[SheetRow],
    setup_draft: Option<&NewProjectState>,
    users: &[UserProfile],
    canvas: &CanvasColors,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let pages = if rows.is_empty() {
        empty_state(project, i18n, tokens)
    } else {
        page_table(project, rows, i18n, tokens)
    };
    section(
        ids::PROJECT_SECTION_SHEETS,
        ids::PROJECT_SHEETS_HELP,
        vec![
            pages,
            new_sheets_card(project, setup_draft, users, canvas, i18n, tokens),
        ],
        i18n,
        tokens,
    )
}

/// The pages in list order: number, name (and `root`), file (and
/// `missing`), and the move buttons.
fn page_table<'a>(
    project: &LoadedProject,
    rows: &[SheetRow],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let columns = [
        (Some(ids::PROJECT_SHEETS_COL_NUMBER), Length::Fixed(PAGE_W)),
        (Some(ids::PROJECT_SHEETS_COL_NAME), Length::FillPortion(2)),
        (Some(ids::PROJECT_SHEETS_COL_FILE), Length::FillPortion(3)),
    ];
    let lines = rows
        .iter()
        .enumerate()
        .map(|(index, sheet)| {
            let filename = sheet.filename.clone();
            let moved = move |step: ListMove| {
                tab_message(ProjectTabMsg::SheetMoved {
                    project_id,
                    filename: filename.clone(),
                    step,
                })
            };
            TableRow {
                cells: vec![
                    text(t!(i18n, ids::PROJECT_SHEETS_PAGE, "page" => index + 1))
                        .size(TS.body)
                        .font(mono_font())
                        .color(ti(tokens.text))
                        .boxed(),
                    name_cell(sheet, i18n, tokens),
                    file_cell(sheet, i18n, tokens),
                ],
                actions: Some(icon_move_buttons(index, rows.len(), moved, i18n, tokens)),
                is_error: false,
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

/// A cell's text, then a pill when `mark` is set.
fn marked<'a>(
    words: Element<'a, Message>,
    mark: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s + SP.xs * 0.5)
        .align_y(Alignment::Center)
        .push(words);
    if let Some(mark) = mark {
        line = line.push(mark);
    }
    line.boxed()
}

fn name_cell<'a>(sheet: &SheetRow, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let name = text(UserText::new(sheet.name.clone()))
        .size(TS.body)
        .color(ti(tokens.text))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .boxed();
    let root = sheet.is_root.then(|| {
        pill(
            t!(i18n, ids::PROJECT_SHEETS_ROOT),
            PillTone::Neutral,
            None,
            tokens,
        )
    });
    marked(name, root)
}

fn file_cell<'a>(sheet: &SheetRow, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let missing = sheet.is_missing.then(|| {
        pill(
            t!(i18n, ids::PROJECT_SHEETS_MISSING),
            PillTone::Err,
            None,
            tokens,
        )
    });
    marked(mono_cell(sheet.filename.clone(), tokens), missing)
}

/// A project that lists no sheets: the note, and Add New ▸ Schematic, which
/// opens what the project tree's Add New ▸ Schematic opens.
fn empty_state<'a>(
    project: &LoadedProject,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let add = text_button(
        Some(Glyph::Plus),
        t!(i18n, ids::PROJECT_SHEETS_ADD_SCHEMATIC),
        Some(tab_message(ProjectTabMsg::AddSchematicRequested {
            project_id: project.id,
        })),
        ButtonKind::Small,
        tokens,
    );
    let line = row![
        text(t!(i18n, ids::PROJECT_SHEETS_EMPTY))
            .size(TS.body)
            .width(Length::Fill),
        add,
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed();
    info_line(line, tokens)
}

/// The sheet of the New sheets card, as shown (the half-edited draft or the
/// project's stored setup), for the larger preview window.
pub(in crate::app::project_tab) fn popup_drawing<'a>(
    project: &LoadedProject,
    draft: Option<&NewProjectState>,
    users: &[UserProfile],
    canvas: &CanvasColors,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let stored;
    let state = match draft {
        Some(draft) => draft,
        None => {
            stored = NewProjectState::of_project(&project.data, users);
            &stored
        }
    };
    preview::large(
        &state.setup.sheet,
        preview::PREVIEW_H * POPUP_HEIGHT_FACTOR,
        canvas,
        i18n,
        tokens,
    )
}

/// The New sheets card: the New Project dialog's sheet column, fed from the
/// project's stored setup (or the half-edited draft) with its edits sent as
/// Project setup edits, beside the preview of the same size and
/// orientation.
fn new_sheets_card<'a>(
    project: &LoadedProject,
    draft: Option<&NewProjectState>,
    users: &[UserProfile],
    canvas: &CanvasColors,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let stored;
    let state = match draft {
        Some(draft) => draft,
        None => {
            stored = NewProjectState::of_project(&project.data, users);
            &stored
        }
    };
    let project_id = project.id;
    let defaults = container(
        customize::schematic_fields(state, i18n, tokens).map(move |msg| {
            Message::Project(ProjectMsg::Tab(ProjectTabMsg::SetupEdited {
                project_id,
                msg,
            }))
        }),
    )
    .width(Length::FillPortion(1));
    let preview = container(preview::view(
        project_id,
        &state.setup.sheet,
        canvas,
        i18n,
        tokens,
    ))
    .width(Length::FillPortion(1));
    let body = row![defaults, preview]
        .spacing(SP.l)
        .align_y(Alignment::Start)
        .width(Length::Fill)
        .boxed();
    let help = text(t!(i18n, ids::PROJECT_SHEETS_DEFAULTS_HELP))
        .size(TS.caption)
        .color(resolve(tokens).ink4)
        .boxed();
    card(
        Some(CardHeader {
            title: t!(i18n, ids::PROJECT_SHEETS_DEFAULTS_TITLE),
            trailing: vec![help],
        }),
        body,
        tokens,
    )
}
