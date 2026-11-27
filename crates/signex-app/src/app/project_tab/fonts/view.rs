//! The Fonts section view (mockup row 205): Add font file... in the header,
//! the summary, then one row per family with its file count, styles,
//! embedding permission and license box, and Remove on row hover. Pure: the
//! project and the section's state in, widgets out.

use iced::widget::row;
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, RenderTranslatable, UserText, ids, t};
use signex_types::project::{FontEmbedding, ProjectFontFamily};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::super::look::{
    ButtonKind, Glyph, err_box, icon_button, info_line, text_button, warn_line,
};
use super::super::table::{
    SP, TS, TableRow, grid_table, note, problem, section_with_action, word_cell,
};
use super::{ProjectFontsMsg, draft_key, embedding_word, message};
use crate::app::Message;
use crate::app::core::project_fonts::FontsTabState;
use crate::app::state::{LoadedProject, ProjectId};
use crate::styles::{ti, tokens::resolve};

/// Width of the Embedding column (130 px).
const EMBEDDING_W: f32 = TS.body * 10.8;
/// Width of the Remove column (40 px).
const ACTIONS_W: f32 = TS.body * 3.4;

pub(in crate::app::project_tab) fn view<'a>(
    project: &LoadedProject,
    state: Option<&FontsTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let adding = state.is_some_and(|state| state.adding);
    let read_only = project.data.unknown.newer_minor_source.is_some();
    let fonts = project.data.project_fonts();
    let can_add = !adding && !read_only && fonts.is_ok();
    let words = if adding {
        t!(i18n, ids::PROJECT_FONTS_ADDING)
    } else {
        t!(i18n, ids::PROJECT_FONTS_ADD)
    };
    let add = text_button(
        Some(Glyph::Plus),
        words,
        can_add.then(|| message(project_id, ProjectFontsMsg::AddRequested)),
        ButtonKind::Outline,
        tokens,
    );
    let mut body: Vec<Element<'a, Message>> = Vec::new();
    match fonts {
        Err(_) => body.push(err_box(
            text(t!(i18n, ids::PROJECT_FONTS_LIST_UNREADABLE))
                .size(TS.body)
                .boxed(),
            tokens,
        )),
        Ok(fonts) if fonts.is_empty() => body.push(info_line(
            text(t!(i18n, ids::PROJECT_FONTS_EMPTY))
                .size(TS.body)
                .boxed(),
            tokens,
        )),
        Ok(fonts) => {
            let files: usize = fonts.iter().map(|family| family.files.len()).sum();
            body.push(note(
                t!(
                    i18n,
                    ids::PROJECT_FONTS_SUMMARY,
                    "families" => fonts.len(),
                    "files" => files
                ),
                tokens,
            ));
            body.push(font_table(project_id, &fonts, state, i18n, tokens));
            if fonts.iter().any(|family| family.license.is_none()) {
                body.push(warn_line(
                    Glyph::Warn,
                    text(t!(i18n, ids::PROJECT_FONTS_LICENSE_MISSING))
                        .size(TS.body)
                        .boxed(),
                    None,
                    tokens,
                ));
            }
        }
    }
    if let Some(refused) = state.and_then(|state| state.problem.as_ref()) {
        body.push(problem(refused.localized(i18n), tokens));
    }
    section_with_action(
        ids::PROJECT_FONTS_TITLE,
        ids::PROJECT_FONTS_HELP,
        add,
        body,
        i18n,
        tokens,
    )
}

fn font_table<'a>(
    project_id: ProjectId,
    fonts: &[ProjectFontFamily],
    state: Option<&FontsTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let columns = [
        (Some(ids::PROJECT_FONTS_COL_FAMILY), Length::FillPortion(3)),
        (Some(ids::PROJECT_FONTS_COL_STYLES), Length::FillPortion(3)),
        (
            Some(ids::PROJECT_FONTS_COL_EMBEDDING),
            Length::Fixed(EMBEDDING_W),
        ),
        (Some(ids::PROJECT_FONTS_COL_LICENSE), Length::FillPortion(3)),
    ];
    let lines = fonts
        .iter()
        .map(|family| {
            let remove = icon_button(
                Glyph::Trash,
                t!(i18n, ids::PROJECT_FONTS_REMOVE),
                Some(message(
                    project_id,
                    ProjectFontsMsg::Removed {
                        family: family.family.clone(),
                    },
                )),
                tokens,
            );
            TableRow {
                cells: vec![
                    family_cell(family, i18n, tokens),
                    styles_cell(family, i18n, tokens),
                    word_cell(t!(i18n, embedding_word(strictest(family))), tokens),
                    license_cell(project_id, family, state, i18n),
                ],
                actions: Some(remove),
                is_error: false,
                grip: None,
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

/// The family name, then how many files hold it, muted.
fn family_cell<'a>(
    family: &ProjectFontFamily,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        text(UserText::new(family.family.clone()))
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
        text(t!(i18n, ids::PROJECT_FONTS_FILES, "count" => family.files.len()))
            .size(TS.caption)
            .color(resolve(tokens).ink3)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// The styles of the family's files, each once, in file order.
fn styles_cell<'a>(
    family: &ProjectFontFamily,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let regular = t!(i18n, ids::PROJECT_FONTS_NO_STYLE).into_string();
    let mut styles: Vec<String> = Vec::new();
    for file in &family.files {
        let style = if file.style.is_empty() {
            regular.clone()
        } else {
            file.style.clone()
        };
        if !styles.contains(&style) {
            styles.push(style);
        }
    }
    text(UserText::new(styles.join(" · ")))
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .boxed()
}

/// The family's license box: the draft while typing, else what is stored.
fn license_cell<'a>(
    project_id: ProjectId,
    family: &ProjectFontFamily,
    state: Option<&FontsTabState>,
    i18n: &I18n,
) -> Element<'a, Message> {
    let typed = state.and_then(|state| state.license_drafts.get(&draft_key(&family.family)));
    let value = typed
        .cloned()
        .or_else(|| family.license.clone())
        .unwrap_or_default();
    let name = family.family.clone();
    let submitted = message(
        project_id,
        ProjectFontsMsg::LicenseSubmitted {
            family: name.clone(),
        },
    );
    text_input(t!(i18n, ids::PROJECT_FONTS_LICENSE_PLACEHOLDER), value)
        .on_input(move |text| {
            message(
                project_id,
                ProjectFontsMsg::LicenseEdited {
                    family: name.clone(),
                    text,
                },
            )
        })
        .on_submit(submitted)
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .boxed()
}

/// The strictest embedding permission of the family's files.
fn strictest(family: &ProjectFontFamily) -> FontEmbedding {
    family
        .files
        .iter()
        .map(|file| file.embedding)
        .reduce(FontEmbedding::strictest)
        .unwrap_or(FontEmbedding::Installable)
}
