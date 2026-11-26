//! General section view (mockup row 29): the shared title and help line, the
//! card of editable values, then the Files card of read-only facts. Pure:
//! reads the project, emits `ProjectTabMsg`s. Sizes come from the shared type
//! and spacing scales.

use iced::widget::{column, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input};

use super::ProjectGeneralFacts;
use crate::app::project_tab::look::{
    ButtonKind, CardHeader, Glyph, card, kv_row, mono_font, text_button, ui_font,
};
use crate::app::project_tab::table::{SP, TS, section};
use crate::app::project_tab::{ProjectSection, ProjectTabMsg};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, ProjectMsg};
use crate::styles::ti;
use crate::styles::tokens::resolve;

pub(in crate::app::project_tab) fn tab_message(msg: ProjectTabMsg) -> Message {
    Message::Project(ProjectMsg::Tab(msg))
}

/// A read-only value: a path, a file name or a count, in mono.
fn mono_value<'a>(value: String, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(UserText::new(value))
        .size(TS.body)
        .font(mono_font())
        .color(ti(tokens.text))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .boxed()
}

fn read_only<'a>(caption: Localized, value: String, tokens: &ThemeTokens) -> Element<'a, Message> {
    kv_row(caption, mono_value(value, tokens), tokens)
}

fn or_dash(value: Option<String>) -> String {
    value.unwrap_or_else(|| "—".to_string())
}

/// The name (the file stem, read-only here) and the Rename… action that
/// runs the project-tree Rename flow.
fn name_row<'a>(
    project: &'a LoadedProject,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let rename = text_button(
        None,
        t!(i18n, ids::PROJECT_GENERAL_RENAME),
        Some(tab_message(ProjectTabMsg::RenameRequested {
            project_id: project.id,
        })),
        ButtonKind::Outline,
        tokens,
    );
    let name = row![
        text(UserText::new(project.data.name.as_str()))
            .size(TS.label)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text))
            .width(Length::Fill),
        rename,
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    kv_row(t!(i18n, ids::PROJECT_GENERAL_NAME), name.boxed(), tokens)
}

/// The library count, and the link to the Libraries section.
fn libraries_row<'a>(
    project_id: ProjectId,
    count: usize,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let show = button(
        text(t!(i18n, ids::PROJECT_GENERAL_SHOW_IN_LIBRARIES))
            .size(TS.body)
            .color(ti(tokens.accent)),
    )
    .padding([0.0, SP.xs])
    .style(crate::styles::ghost_button(tokens))
    .on_press(tab_message(ProjectTabMsg::SectionSelected {
        project_id,
        section: ProjectSection::Libraries,
    }));
    let value = row![mono_value(i18n.count(count as i64), tokens), show]
        .spacing(SP.l)
        .align_y(Alignment::Center);
    kv_row(
        t!(i18n, ids::PROJECT_GENERAL_LIBRARIES),
        value.boxed(),
        tokens,
    )
}

/// The Files card: `read-only` and Explore in its header, then the facts.
fn files_card<'a>(
    project: &LoadedProject,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let facts = ProjectGeneralFacts::of(project);
    let read_only_mark = text(t!(i18n, ids::PANELS_TAB_READ_ONLY))
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .boxed();
    let explore = text_button(
        Some(Glyph::Folder),
        t!(i18n, signex_i18n::menu_ids::CTX_TREE_EXPLORE),
        Some(tab_message(ProjectTabMsg::ExploreRequested { project_id })),
        ButtonKind::Small,
        tokens,
    );
    let rows = column![
        read_only(
            t!(i18n, ids::PROJECT_GENERAL_DIRECTORY),
            facts.directory,
            tokens
        ),
        read_only(
            t!(i18n, ids::PROJECT_GENERAL_ROOT_SCHEMATIC),
            or_dash(facts.schematic_root),
            tokens
        ),
        read_only(
            t!(i18n, ids::PROJECT_GENERAL_PCB_FILE),
            or_dash(facts.pcb_file),
            tokens
        ),
        libraries_row(project_id, facts.library_count, i18n, tokens),
    ]
    .spacing(SP.m);
    card(
        Some(CardHeader {
            title: t!(i18n, ids::PROJECT_GENERAL_FILES),
            trailing: vec![read_only_mark, explore],
        }),
        rows.boxed(),
        tokens,
    )
}

pub(in crate::app::project_tab) fn view<'a>(
    i18n: &I18n,
    project: &'a LoadedProject,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let description = text_input(
        t!(i18n, ids::PLACEHOLDER_PROJECT_DESCRIPTION),
        project.data.description.as_deref().unwrap_or(""),
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(move |text| tab_message(ProjectTabMsg::DescriptionEdited { project_id, text }))
    .size(TS.body)
    .padding(SP.s)
    .width(Length::Fill);
    let fields = column![
        name_row(project, i18n, tokens),
        kv_row(
            t!(i18n, ids::PROJECT_GENERAL_DESCRIPTION),
            description.boxed(),
            tokens
        ),
        super::designator_standard::row(project, i18n, tokens),
    ]
    .spacing(SP.m);
    section(
        ids::PROJECT_SECTION_GENERAL,
        ids::PROJECT_GENERAL_HELP,
        vec![
            card(None, fields.boxed(), tokens),
            files_card(project, i18n, tokens),
        ],
        i18n,
        tokens,
    )
}
