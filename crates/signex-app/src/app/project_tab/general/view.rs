//! General section view: the name with Rename…, the description input, then
//! the read-only file facts. Pure: reads the project, emits `ProjectTabMsg`s.
//! Sizes come from the shared type and spacing scales.

use iced::widget::{Widget, column, container, row};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text, text_input};

use super::ProjectGeneralFacts;
use crate::app::project_tab::ProjectTabMsg;
use crate::app::state::LoadedProject;
use crate::app::{Message, ProjectMsg};
use crate::styles::{Spacing, TypeScale};

/// Width of the label column, in `body`-size characters.
const LABEL_COLUMN_EMS: f32 = 12.0;
/// Widest the section's rows grow, in `body`-size characters.
const SECTION_MAX_EMS: f32 = 48.0;

pub(in crate::app::project_tab) fn tab_message(msg: ProjectTabMsg) -> Message {
    Message::Project(ProjectMsg::Tab(msg))
}

pub(in crate::app::project_tab) fn labelled<'a>(
    label: &'a str,
    field: impl Widget<Message> + 'a,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    row![
        container(
            text(raw(label))
                .size(ts.body)
                .color(crate::styles::ti(tokens.text_secondary))
        )
        .width(Length::Fixed(ts.body * LABEL_COLUMN_EMS)),
        field.boxed(),
    ]
    .spacing(sp.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

fn read_only<'a>(label: &'a str, value: String, tokens: &ThemeTokens) -> Element<'a, Message> {
    let value = text(raw(value))
        .size(TypeScale::default().label)
        .color(crate::styles::ti(tokens.text));
    labelled(label, value, tokens)
}

fn or_dash(value: Option<String>) -> String {
    value.unwrap_or_else(|| "—".to_string())
}

/// The name (the file stem, read-only here) and the Rename… action that
/// runs the project-tree Rename flow.
fn name_row<'a>(project: &'a LoadedProject, tokens: &ThemeTokens) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let rename = button(text(raw("Rename…")).size(ts.body))
        .padding([sp.xs, sp.m])
        .style(crate::styles::outline_button(tokens))
        .on_press(tab_message(ProjectTabMsg::RenameRequested {
            project_id: project.id,
        }));
    let name = row![
        text(signex_i18n::UserText::new(project.data.name.as_str()))
            .size(ts.label)
            .color(crate::styles::ti(tokens.text))
            .width(Length::Fill),
        rename,
    ]
    .spacing(sp.m)
    .align_y(iced::Alignment::Center);
    labelled("Name", name, tokens)
}

pub(in crate::app::project_tab) fn view<'a>(
    i18n: &signex_i18n::I18n,
    project: &'a LoadedProject,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let project_id = project.id;
    let description = text_input(
        raw("Description"),
        project.data.description.as_deref().unwrap_or(""),
    )
    .on_input(move |text| tab_message(ProjectTabMsg::DescriptionEdited { project_id, text }))
    .size(ts.label)
    .padding(sp.s)
    .width(Length::Fill);

    let facts = ProjectGeneralFacts::of(project);
    let body = column![
        text(raw("General"))
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text)),
        name_row(project, tokens),
        labelled("Description", description, tokens),
        super::designator_standard::row(project, tokens),
        read_only("Directory", facts.directory, tokens),
        read_only("Root schematic", or_dash(facts.schematic_root), tokens),
        read_only("PCB file", or_dash(facts.pcb_file), tokens),
        read_only("Libraries", i18n.count(facts.library_count as i64), tokens),
    ]
    .spacing(sp.l)
    .width(Length::Fill.max(ts.body * SECTION_MAX_EMS));
    container(body).padding([sp.xl, sp.xl]).boxed()
}
