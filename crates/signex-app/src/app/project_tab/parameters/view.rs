//! Parameters section view. Pure: reads the project, emits `ProjectTabMsg`s.

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

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
        "",
        project.data.parameters.get(name).map_or("", String::as_str),
    )
    .on_input(move |value| {
        tab_message(ProjectTabMsg::ParameterEdited {
            project_id,
            name: key.clone(),
            value,
        })
    })
    .size(ts.label)
    .padding(sp.s)
    .width(Length::Fill)
}

pub(in crate::app::project_tab) fn view<'a>(
    project: &'a LoadedProject,
    new_name: &'a str,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let project_id = project.id;
    let mut body = column![
        text("Parameters")
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text)),
        text("Every sheet inherits these; a sheet's own Title Block value overrides one.")
            .size(ts.label)
            .color(crate::styles::ti(tokens.text_secondary)),
    ]
    .spacing(sp.l)
    .max_width(ts.body * SECTION_MAX_EMS);
    for field in TitleBlockField::ALL {
        body = body.push(labelled(
            field.label(),
            value_input(project, field.key()),
            tokens,
        ));
    }
    body = body.push(
        text("Custom parameters")
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text)),
    );
    for name in project
        .data
        .parameters
        .keys()
        .filter(|k| !TitleBlockField::ALL.iter().any(|f| f.key() == k.as_str()))
    {
        let remove = button(text("Remove").size(ts.body))
            .padding([sp.xs, sp.m])
            .style(crate::styles::outline_button(tokens))
            .on_press(tab_message(ProjectTabMsg::ParameterRemoved {
                project_id,
                name: name.clone(),
            }));
        body = body.push(labelled(
            name,
            row![value_input(project, name), remove]
                .spacing(sp.m)
                .align_y(iced::Alignment::Center),
            tokens,
        ));
    }
    let add = button(text("Add").size(ts.body))
        .padding([sp.xs, sp.m])
        .style(crate::styles::outline_button(tokens))
        .on_press(tab_message(ProjectTabMsg::ParameterAdded { project_id }));
    let new_input = text_input("Name", new_name)
        .on_input(move |text| {
            tab_message(ProjectTabMsg::NewParameterNameEdited { project_id, text })
        })
        .on_submit(tab_message(ProjectTabMsg::ParameterAdded { project_id }))
        .size(ts.label)
        .padding(sp.s)
        .width(Length::Fill);
    body = body.push(row![new_input, add].spacing(sp.m));
    container(body).padding([sp.xl, sp.xl]).into()
}
