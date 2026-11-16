//! Sheets section of the Project tab (#1689): the project's sheets in page
//! order (Sheet N of M, the project tree, PDF pages) with Move up / Move
//! down, then what a new sheet starts from (size, orientation, title block),
//! edited with the New Project dialog's own fields through the Project setup
//! flow. A move is one step of the General history.

use iced::Length;
use iced::widget::container;
use iced::{Element, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::project::{ListMove, UserProfile};
use signex_types::theme::ThemeTokens;

use super::ProjectTabMsg;
use super::general::tab_message;
use super::table::{
    TS, cell, heading, heading_row, move_buttons, note, section, table_row, word_cell,
};
use crate::app::new_project::NewProjectState;
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::view::dialogs::new_project::customize;
use crate::app::{Message, ProjectMsg, Signex};

/// Width of the page column, in `body`-size characters.
const PAGE_EMS: f32 = 5.0;
/// Width of the Move up / Move down column, in `body`-size characters.
pub(super) const ACTIONS_EMS: f32 = 16.0;

pub(super) fn view<'a>(
    project: &LoadedProject,
    setup_draft: Option<&NewProjectState>,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let columns = [
        (
            ids::PROJECT_SHEETS_COL_NUMBER,
            Length::Fixed(TS.body * PAGE_EMS),
        ),
        (ids::PROJECT_SHEETS_COL_NAME, Length::FillPortion(2)),
        (ids::PROJECT_SHEETS_COL_FILE, Length::FillPortion(3)),
    ];
    let actions = Length::Fixed(TS.body * ACTIONS_EMS);
    let mut body: Vec<Element<'a, Message>> =
        vec![heading_row(&columns, Some(actions), i18n, tokens)];
    let sheets = &project.data.sheets;
    if sheets.is_empty() {
        body.push(note(t!(i18n, ids::PROJECT_SHEETS_EMPTY), tokens));
    }
    for (index, sheet) in sheets.iter().enumerate() {
        let filename = sheet.filename.clone();
        let moved = move |step: ListMove| {
            tab_message(ProjectTabMsg::SheetMoved {
                project_id,
                filename: filename.clone(),
                step,
            })
        };
        body.push(table_row(vec![
            (
                word_cell(
                    t!(i18n, ids::PROJECT_SHEETS_PAGE, "page" => index + 1),
                    tokens,
                ),
                columns[0].1,
            ),
            (cell(sheet.name.clone(), tokens), columns[1].1),
            (cell(sheet.filename.clone(), tokens), columns[2].1),
            (
                move_buttons(index, sheets.len(), moved, i18n, tokens),
                actions,
            ),
        ]));
    }
    body.push(heading(
        t!(i18n, ids::PROJECT_SHEETS_DEFAULTS_TITLE),
        tokens,
    ));
    body.push(note(t!(i18n, ids::PROJECT_SHEETS_DEFAULTS_HELP), tokens));
    body.push(new_sheet_defaults(
        project,
        setup_draft,
        users,
        i18n,
        tokens,
    ));
    section(
        ids::PROJECT_SECTION_SHEETS,
        ids::PROJECT_SHEETS_HELP,
        body,
        i18n,
        tokens,
    )
}

/// The New Project dialog's sheet column, fed from the project's stored
/// setup (or the half-edited draft), its edits sent as Project setup edits.
fn new_sheet_defaults<'a>(
    project: &LoadedProject,
    draft: Option<&NewProjectState>,
    users: &[UserProfile],
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
    container(customize::schematic(state, i18n, tokens).map(move |msg| {
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::SetupEdited {
            project_id,
            msg,
        }))
    }))
    .width(Length::Fill)
    .boxed()
}

impl Signex {
    /// Move up / Move down in the Sheets table: one step of its own.
    pub(super) fn move_project_sheet(
        &mut self,
        project_id: ProjectId,
        filename: String,
        step: ListMove,
    ) {
        let command = signex_engine::ProjectSectionCommand::MoveSheet { filename, step };
        // A move never refuses; at the end of the list it changes nothing.
        if let Err(problem) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }
}
