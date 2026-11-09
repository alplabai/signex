//! Schematic defaults section view. Pure: reads the project, emits
//! `ProjectTabMsg`s.

use signex_widgets::dropdown_style::GraphiteDropdown as _;
use std::collections::HashMap;

use iced::widget::{column, container, pick_list};
use iced::{Element, Length, Widget};
use signex_types::net_class::NetLineStyle;
use signex_types::project::SchematicDefaultField;
use signex_types::schematic::FillType;
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text, text_input};

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
        text(raw("Schematic defaults"))
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text))
            .boxed(),
        text(raw("Unset widths and sizes on every sheet of this project follow these; leave a field empty for the built-in value."))
            .size(ts.label)
            .color(crate::styles::ti(tokens.text_secondary))
            .boxed(),
    ]
    .spacing(sp.l)
    .width(Length::Fixed(ts.body * SECTION_MAX_EMS));
    for field in SchematicDefaultField::ALL {
        let draft = drafts.and_then(|drafts| drafts.get(&field));
        let shown = super::shown_text(draft, overrides.get(field));
        let input = text_input(raw(format!("{}", field.resolved(&resolved))), shown)
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
        if let Some(note) = super::clamp_note(field, draft) {
            body = body.push(
                text(raw(note))
                    .size(ts.caption)
                    .color(crate::styles::ti(tokens.text_secondary))
                    .boxed(),
            );
        }
    }
    let font = text_input(
        raw(overrides.font_family()),
        overrides.font_family.as_deref().unwrap_or_default(),
    )
    .on_input(move |text| {
        tab_message(ProjectTabMsg::SchematicDefaultFontEdited { project_id, text })
    })
    .size(ts.label)
    .padding(sp.s)
    .width(Length::Fill);
    body = body.push(labelled("Font", font, tokens));
    let line = pick_list(
        Some(LineChoice(overrides.wire_line_style)),
        LineChoice::ALL,
        LineChoice::to_string,
    )
    .graphite()
    .on_select(move |choice: LineChoice| {
        tab_message(ProjectTabMsg::SchematicDefaultLineStylePicked {
            project_id,
            line: choice.0,
        })
    })
    .width(Length::Fill);
    body = body.push(labelled("Wire line style", line, tokens));
    let fill = pick_list(
        Some(FillChoice(overrides.shape_fill)),
        FillChoice::ALL,
        FillChoice::to_string,
    )
    .graphite()
    .on_select(move |choice: FillChoice| {
        tab_message(ProjectTabMsg::SchematicDefaultShapeFillPicked {
            project_id,
            fill: choice.0,
        })
    })
    .width(Length::Fill);
    body = body.push(labelled("New shape fill", fill, tokens));
    container(body).padding([sp.xl, sp.xl]).boxed()
}

/// A wire line style choice; `None` is the built-in Solid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LineChoice(Option<NetLineStyle>);

impl LineChoice {
    const ALL: [Self; 4] = [
        Self(None),
        Self(Some(NetLineStyle::Solid)),
        Self(Some(NetLineStyle::Dashed)),
        Self(Some(NetLineStyle::Dotted)),
    ];
}

impl std::fmt::Display for LineChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(line) => f.write_str(line.label()),
            None => f.write_str("Built-in (Solid)"),
        }
    }
}

/// A new-shape fill choice; `None` is the built-in no fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FillChoice(Option<FillType>);

impl FillChoice {
    const ALL: [Self; 4] = [
        Self(None),
        Self(Some(FillType::None)),
        Self(Some(FillType::Outline)),
        Self(Some(FillType::Background)),
    ];
}

impl std::fmt::Display for FillChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            None => "Built-in (No fill)",
            Some(FillType::None) => "No fill",
            Some(FillType::Outline) => "Outline colour",
            Some(FillType::Background) => "Background colour",
        })
    }
}
