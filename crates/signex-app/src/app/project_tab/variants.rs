//! Variants section of the Project tab (#1690): Base and the project's
//! variant definitions in a table, the active one checked, each name editable
//! in place (Enter renames), Remove per row and Add under the table. Every
//! edit is one step of the General history; a refused one shows its reason
//! and changes nothing.

use iced::widget::row;
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::project::{MAX_VARIANT_NAME_CHARS, VariantProblem};
use signex_types::theme::ThemeTokens;
use signex_widgets::{radio, text_input};

use super::general::tab_message;
use super::table::{SP, TS, action, heading_row, problem, section, table_row, word_cell};
use super::{ProjectTabMsg, ProjectTabState, VariantRenameDraft};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, Signex};

/// Width of the Active column, in `body`-size characters.
const ACTIVE_EMS: f32 = 6.0;
/// Width of the Remove column, in `body`-size characters.
const ACTIONS_EMS: f32 = 8.0;

/// The words of a refused variant edit.
pub(super) fn problem_words(problem: &VariantProblem, i18n: &I18n) -> Localized {
    match problem {
        VariantProblem::Empty => t!(i18n, ids::PROJECT_VARIANT_EMPTY),
        VariantProblem::TooLong => {
            t!(i18n, ids::PROJECT_VARIANT_TOO_LONG, "max" => MAX_VARIANT_NAME_CHARS)
        }
        VariantProblem::Invalid => t!(i18n, ids::PROJECT_VARIANT_INVALID),
        VariantProblem::Reserved => t!(i18n, ids::PROJECT_VARIANT_RESERVED),
        VariantProblem::Taken(name) => {
            t!(i18n, ids::PROJECT_VARIANT_TAKEN, "name" => name.as_str())
        }
        VariantProblem::Unknown(name) => {
            t!(i18n, ids::PROJECT_VARIANT_UNKNOWN, "name" => name.as_str())
        }
    }
}

pub(super) fn view<'a>(
    project: &LoadedProject,
    tab: Option<&ProjectTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let data = &project.data;
    let active_w = Length::Fixed(TS.body * ACTIVE_EMS);
    let actions = Length::Fixed(TS.body * ACTIONS_EMS);
    let columns = [
        (ids::PROJECT_VARIANTS_COL_ACTIVE, active_w),
        (ids::PROJECT_VARIANTS_COL_NAME, Length::Fill),
    ];
    // `None` is Base; a radio needs one value type for every row.
    let active: Option<usize> = data.active_variant.as_deref().and_then(|name| {
        data.variant_definitions
            .iter()
            .position(|defined| defined.eq_ignore_ascii_case(name))
    });
    // Row 0 is Base, row `i + 1` the definition `i`.
    let selected = Some(active.map_or(0, |index| index + 1));
    let names = data.variant_definitions.clone();
    let on_pick = move |row: usize| {
        tab_message(ProjectTabMsg::ActiveVariantPicked {
            project_id,
            name: row.checked_sub(1).and_then(|i| names.get(i).cloned()),
        })
    };
    let mut body: Vec<Element<'a, Message>> =
        vec![heading_row(&columns, Some(actions), i18n, tokens)];
    body.push(table_row(vec![
        (
            radio(
                UserText::new(String::new()),
                0usize,
                selected,
                on_pick.clone(),
            )
            .size(TS.body)
            .boxed(),
            active_w,
        ),
        (
            word_cell(t!(i18n, ids::PROJECT_VARIANTS_BASE), tokens),
            Length::Fill,
        ),
        (
            iced::widget::Row::<Element<'_, Message>>::new().boxed(),
            actions,
        ),
    ]));
    let draft = tab.and_then(|tab| tab.variant_rename.as_ref());
    for (index, name) in data.variant_definitions.iter().enumerate() {
        let shown = match draft {
            Some(VariantRenameDraft { from, text }) if from == name => text.clone(),
            _ => name.clone(),
        };
        let from = name.clone();
        let field = text_input(t!(i18n, ids::PROJECT_VARIANTS_COL_NAME), shown)
            .on_input(move |text| {
                tab_message(ProjectTabMsg::VariantRenameEdited {
                    project_id,
                    from: from.clone(),
                    text,
                })
            })
            .on_submit(tab_message(ProjectTabMsg::VariantRenameSubmitted {
                project_id,
            }))
            .style(crate::panels::form_look::text_input_style)
            .size(TS.body)
            .padding(SP.xs)
            .width(Length::Fill);
        let remove = tab_message(ProjectTabMsg::VariantRemoved {
            project_id,
            name: name.clone(),
        });
        body.push(table_row(vec![
            (
                radio(
                    UserText::new(String::new()),
                    index + 1,
                    selected,
                    on_pick.clone(),
                )
                .size(TS.body)
                .boxed(),
                active_w,
            ),
            (field.boxed(), Length::Fill),
            (
                action(t!(i18n, ids::PROJECT_VARIANTS_REMOVE), Some(remove), tokens),
                actions,
            ),
        ]));
    }
    body.push(add_row(project_id, tab, i18n, tokens));
    if let Some(refused) = tab.and_then(|tab| tab.variant_problem.as_ref()) {
        body.push(problem(problem_words(refused, i18n), tokens));
    }
    section(
        ids::PROJECT_SECTION_VARIANTS,
        ids::PROJECT_VARIANTS_HELP,
        body,
        i18n,
        tokens,
    )
}

/// The new variant's name box and Add.
fn add_row<'a>(
    project_id: ProjectId,
    tab: Option<&ProjectTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let typed = tab.map_or_else(String::new, |tab| tab.new_variant_name.clone());
    let add =
        (!typed.trim().is_empty()).then(|| tab_message(ProjectTabMsg::VariantAdded { project_id }));
    let field = text_input(t!(i18n, ids::PROJECT_VARIANTS_NEW), typed)
        .on_input(move |text| tab_message(ProjectTabMsg::NewVariantNameEdited { project_id, text }))
        .on_submit_maybe(add.clone())
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fill);
    row![
        field,
        action(t!(i18n, ids::PROJECT_VARIANTS_ADD), add, tokens),
    ]
    .spacing(SP.s)
    .padding([0.0, SP.m])
    .align_y(iced::Alignment::Center)
    .boxed()
}

impl Signex {
    pub(super) fn edit_new_variant_name(&mut self, project_id: ProjectId, text: String) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.new_variant_name = text;
            tab.variant_problem = None;
        }
    }

    /// Add: the typed name as a new variant, or the reason it is refused.
    pub(super) fn add_typed_variant(&mut self, project_id: ProjectId) {
        let Some(name) = self
            .document_state
            .project_tabs
            .get(&project_id)
            .map(|tab| tab.new_variant_name.clone())
        else {
            return;
        };
        let command = signex_engine::ProjectSectionCommand::AddVariant { name };
        let outcome = self.apply_project_section(project_id, &command);
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            match outcome {
                Ok(_) => {
                    tab.new_variant_name.clear();
                    tab.variant_problem = None;
                }
                Err(refused) => tab.variant_problem = Some(refused),
            }
        }
    }

    pub(super) fn edit_variant_rename(
        &mut self,
        project_id: ProjectId,
        from: String,
        text: String,
    ) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_rename = Some(VariantRenameDraft { from, text });
            tab.variant_problem = None;
        }
    }

    /// Enter in a name box: rename the variant to what was typed. A refused
    /// name stays in the box with its reason.
    pub(super) fn submit_variant_rename(&mut self, project_id: ProjectId) {
        let Some(draft) = self
            .document_state
            .project_tabs
            .get_mut(&project_id)
            .and_then(|tab| tab.variant_rename.take())
        else {
            return;
        };
        if draft.text == draft.from {
            return;
        }
        let command = signex_engine::ProjectSectionCommand::RenameVariant {
            from: draft.from.clone(),
            to: draft.text.clone(),
        };
        let outcome = self.apply_project_section(project_id, &command);
        if let (Err(refused), Some(tab)) = (
            outcome,
            self.document_state.project_tabs.get_mut(&project_id),
        ) {
            tab.variant_problem = Some(refused);
            tab.variant_rename = Some(draft);
        }
    }

    pub(super) fn remove_project_variant(&mut self, project_id: ProjectId, name: String) {
        let command = signex_engine::ProjectSectionCommand::RemoveVariant { name: name.clone() };
        let outcome = self.apply_project_section(project_id, &command);
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            // A half-typed rename of the removed variant goes with it.
            if tab.variant_rename.as_ref().is_some_and(|d| d.from == name) {
                tab.variant_rename = None;
            }
            tab.variant_problem = outcome.err();
        }
    }

    /// The Active radio of a row; `None` is Base.
    pub(super) fn pick_active_variant(&mut self, project_id: ProjectId, name: Option<String>) {
        let command = signex_engine::ProjectSectionCommand::SetActiveVariant { name };
        let outcome = self.apply_project_section(project_id, &command);
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_problem = outcome.err();
        }
    }
}
