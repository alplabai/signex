//! Parameters section view. Pure: reads the project, emits `ProjectTabMsg`s.

use iced::widget::{column, container, row};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};
use signex_types::theme::ThemeTokens;
use signex_types::variables::{FieldText, TextTable, classify, variable_text};
use signex_widgets::{button, text, text_input};

use crate::app::Message;
use crate::app::project_tab::ProjectTabMsg;
use crate::app::project_tab::general::{labelled, tab_message};
use crate::app::state::LoadedProject;
use crate::panels::TitleBlockField;
use crate::styles::{Spacing, TypeScale};

/// Widest the section's rows grow, in `body`-size characters.
const SECTION_MAX_EMS: f32 = 48.0;

fn value_input<'a>(project: &'a LoadedProject, name: &str) -> iced::widget::TextInput<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let project_id = project.id;
    let key = name.to_string();
    text_input(
        signex_i18n::UserText::new(String::new()),
        project.data.parameters.get(name).map_or("", String::as_str),
    )
    .on_input(move |value| {
        tab_message(ProjectTabMsg::ParameterEdited {
            project_id,
            name: key.clone(),
            value,
        })
    })
    .on_submit(tab_message(ProjectTabMsg::ParameterSubmitted { project_id }))
    .size(ts.label)
    .padding(sp.s)
    .width(Length::Fill)
}

/// What the variable `name` evaluates to, or why it does not (#1987); `None`
/// for a literal value, which shows as typed.
fn evaluation_note<'a>(
    i18n: &signex_i18n::I18n,
    project: &LoadedProject,
    name: &str,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    let ts = TypeScale::default();
    let stored = project.data.parameters.get(name)?;
    if let FieldText::Literal(_) = classify(stored) {
        return None;
    }
    let (note, color) = match variable_text(&TextTable(&project.data.parameters), name)? {
        Ok(value) => (
            t!(i18n, ids::PROJECT_PARAMETER_SHOWS, "value" => value),
            tokens.text_secondary,
        ),
        Err(error) => {
            let error = error.to_string();
            (t!(i18n, ids::PROJECT_PARAMETER_ERROR, "error" => error), tokens.error)
        }
    };
    Some(
        text(note)
            .size(ts.label)
            .color(crate::styles::ti(color))
            .boxed(),
    )
}

pub(in crate::app::project_tab) fn view<'a>(
    i18n: &signex_i18n::I18n,
    project: &'a LoadedProject,
    new_name: &'a str,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let project_id = project.id;
    let mut body = column![
        text(t!(i18n, ids::PROJECT_SECTION_PARAMETERS))
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text))
            .boxed(),
        text(t!(i18n, ids::PROJECT_PARAMETERS_HINT))
            .size(ts.label)
            .color(crate::styles::ti(tokens.text_secondary))
            .boxed(),
    ]
    .spacing(sp.l)
    .width(Length::Fixed(ts.body * SECTION_MAX_EMS));
    for field in TitleBlockField::ALL {
        body = body.push(labelled(
            field.label(),
            value_input(project, field.key()),
            tokens,
        ));
        if let Some(note) = evaluation_note(i18n, project, field.key(), tokens) {
            body = body.push(note);
        }
    }
    body = body.push(
        text(t!(i18n, ids::PROJECT_PARAMETERS_VARIABLES))
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text))
            .boxed(),
    );
    for name in project
        .data
        .parameters
        .keys()
        .filter(|k| !TitleBlockField::ALL.iter().any(|f| f.key() == k.as_str()))
    {
        let remove = button(text(t!(i18n, ids::PROJECT_PARAMETERS_REMOVE)).size(ts.body))
            .padding([sp.xs, sp.m])
            .style(crate::styles::outline_button(tokens))
            .on_press(tab_message(ProjectTabMsg::ParameterRemoved {
                project_id,
                name: name.clone(),
            }));
        body = body.push(labelled(
            name,
            row![value_input(project, name).boxed(), remove.boxed()]
                .spacing(sp.m)
                .align_y(iced::Alignment::Center),
            tokens,
        ));
        if let Some(note) = evaluation_note(i18n, project, name, tokens) {
            body = body.push(note);
        }
    }
    let add = button(text(t!(i18n, ids::PROJECT_PARAMETERS_ADD)).size(ts.body))
        .padding([sp.xs, sp.m])
        .style(crate::styles::outline_button(tokens))
        .on_press(tab_message(ProjectTabMsg::ParameterAdded { project_id }));
    let new_input = text_input(
        signex_i18n::t!(i18n, signex_i18n::ids::PLACEHOLDER_PROJECT_PARAMETER_NAME),
        new_name,
    )
    .on_input(move |text| tab_message(ProjectTabMsg::NewParameterNameEdited { project_id, text }))
    .on_submit(tab_message(ProjectTabMsg::ParameterAdded { project_id }))
    .size(ts.label)
    .padding(sp.s)
    .width(Length::Fill);
    body = body.push(row![new_input.boxed(), add.boxed()].spacing(sp.m).boxed());
    container(body).padding([sp.xl, sp.xl]).boxed()
}
