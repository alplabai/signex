//! Schematic defaults section view. Pure: reads the project, emits
//! `ProjectTabMsg`s.

use std::collections::HashMap;

use iced::widget::{column, container, text, text_input};
use iced::{Element, Length, Widget};
use signex_types::project::SchematicDefaultField;
use signex_types::theme::ThemeTokens;

use crate::app::Message;
use crate::app::project_tab::ProjectTabMsg;
use crate::app::project_tab::general::{labelled, tab_message};
use crate::app::state::LoadedProject;
use crate::styles::{Spacing, TypeScale};

/// Widest the section's rows grow, in `body`-size characters.
const SECTION_MAX_EMS: f32 = 48.0;

pub(in crate::app::project_tab) fn view<'a>(
    project: &'a LoadedProject,
    drafts: Option<&'a HashMap<SchematicDefaultField, String>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let project_id = project.id;
    let overrides = &project.data.schematic_defaults;
    let resolved = overrides.resolve();
    let mut body = column![
        text("Schematic defaults")
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text))
            .boxed(),
        text("Unset widths and sizes on every sheet of this project follow these; leave a field empty for the built-in value.")
            .size(ts.label)
            .color(crate::styles::ti(tokens.text_secondary))
            .boxed(),
    ]
    .spacing(sp.l)
    .width(Length::Fixed(ts.body * SECTION_MAX_EMS));
    for field in SchematicDefaultField::ALL {
        let draft = drafts.and_then(|drafts| drafts.get(&field));
        let shown = super::shown_text(draft, overrides.get(field));
        let input = text_input(format!("{}", field.resolved(&resolved)), shown)
            .on_input(move |text| {
                tab_message(ProjectTabMsg::SchematicDefaultEdited {
                    project_id,
                    field,
                    text,
                })
            })
            .size(ts.label)
            .padding(sp.s)
            .width(Length::Fill);
        body = body.push(labelled(field.label(), input, tokens));
    }
    let font = text_input(
        overrides.font_family(),
        overrides.font_family.as_deref().unwrap_or_default(),
    )
    .on_input(move |text| {
        tab_message(ProjectTabMsg::SchematicDefaultFontEdited { project_id, text })
    })
    .size(ts.label)
    .padding(sp.s)
    .width(Length::Fill);
    body = body.push(labelled("Font", font, tokens));
    container(body).padding([sp.xl, sp.xl]).boxed()
}
