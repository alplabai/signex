//! General section view: name and description inputs, then the read-only
//! file facts. Pure: reads the project, emits `ProjectTabMsg`s.

use iced::widget::{column, container, row, text, text_input};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use super::{ProjectGeneralFacts, project_stem_problem};
use crate::app::project_tab::ProjectTabMsg;
use crate::app::state::LoadedProject;
use crate::app::{Message, ProjectMsg};

const LABEL_WIDTH: f32 = 140.0;
const TEXT_SIZE: f32 = 12.0;
const LABEL_SIZE: f32 = 11.0;
const ROW_SPACING: f32 = 10.0;
const FIELD_MAX_WIDTH: f32 = 520.0;

fn tab_message(msg: ProjectTabMsg) -> Message {
    Message::Project(ProjectMsg::Tab(msg))
}

fn labelled<'a>(
    label: &'a str,
    field: impl Into<Element<'a, Message>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        container(
            text(label)
                .size(LABEL_SIZE)
                .color(crate::styles::ti(tokens.text_secondary))
        )
        .width(Length::Fixed(LABEL_WIDTH)),
        field.into(),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn read_only<'a>(label: &'a str, value: String, tokens: &ThemeTokens) -> Element<'a, Message> {
    let value = text(value)
        .size(TEXT_SIZE)
        .color(crate::styles::ti(tokens.text));
    labelled(label, value, tokens)
}

fn or_dash(value: Option<String>) -> String {
    value.unwrap_or_else(|| "—".to_string())
}

pub(in crate::app::project_tab) fn view<'a>(
    project: &'a LoadedProject,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let name = text_input("Project name", &project.data.name)
        .on_input(move |text| tab_message(ProjectTabMsg::NameEdited { project_id, text }))
        .size(TEXT_SIZE)
        .padding(6)
        .width(Length::Fill);
    let description = text_input(
        "Description",
        project.data.description.as_deref().unwrap_or(""),
    )
    .on_input(move |text| tab_message(ProjectTabMsg::DescriptionEdited { project_id, text }))
    .size(TEXT_SIZE)
    .padding(6)
    .width(Length::Fill);

    let mut body = column![
        text("General")
            .size(14)
            .color(crate::styles::ti(tokens.text)),
        labelled("Name", name, tokens),
    ]
    .spacing(ROW_SPACING)
    .max_width(FIELD_MAX_WIDTH);
    if let Some(problem) = project_stem_problem(&project.data.name) {
        let note = text(format!(
            "{problem} The project is not saved until it is fixed."
        ))
        .size(LABEL_SIZE)
        .color(crate::styles::ti(tokens.error));
        body = body.push(labelled("", note, tokens));
    }
    let facts = ProjectGeneralFacts::of(project);
    body = body
        .push(labelled("Description", description, tokens))
        .push(read_only("Directory", facts.directory, tokens))
        .push(read_only(
            "Root schematic",
            or_dash(facts.schematic_root),
            tokens,
        ))
        .push(read_only("PCB file", or_dash(facts.pcb_file), tokens))
        .push(read_only(
            "Libraries",
            facts.library_count.to_string(),
            tokens,
        ));
    container(body).padding([16, 20]).into()
}
