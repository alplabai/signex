//! The ERC and annotation section's view (Graphite mockup m34): an ERC rules
//! card with the rule source pill, the severity tiles and Open Electrical
//! Rules Check…, then an Annotation card. Pure: the project and its facts
//! in, widgets out.

use iced::font::Weight;
use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_erc::Severity;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::project::AnnotateUndo;
use signex_types::theme::ThemeTokens;
use signex_widgets::{radio, text};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{
    ButtonKind, CardHeader, Glyph, card, field_label, info_line, kv_row, mono_font, pill,
    text_button, warn_line,
};
use super::super::table::{SP, TS, section};
use super::{ErcFacts, ErcSource};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{ErcMsg, Message};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, ti};

/// The words and tone of the rule source pill in the ERC rules card header.
pub(super) fn source_pill(source: ErcSource) -> (MessageId, PillTone) {
    match source {
        ErcSource::Machine => (ids::PROJECT_ERC_SOURCE_MACHINE, PillTone::Neutral),
        ErcSource::Project => (ids::PROJECT_ERC_SOURCE_PROJECT, PillTone::Ok),
        ErcSource::Unreadable => (ids::PROJECT_ERC_SOURCE_UNREADABLE, PillTone::Warn),
    }
}

/// The words under a severity tile's count: a rule count, not ERC results.
pub(super) fn tile_words(severity: Severity) -> MessageId {
    match severity {
        Severity::Error => ids::PROJECT_ERC_TILE_ERROR,
        Severity::Warning => ids::PROJECT_ERC_TILE_WARNING,
        Severity::Info => ids::PROJECT_ERC_TILE_INFO,
        Severity::Off => ids::PROJECT_ERC_TILE_OFF,
    }
}

/// The colour of a severity tile's dot.
fn tile_dot(severity: Severity, tokens: &ThemeTokens) -> Color {
    let r = resolve(tokens);
    match severity {
        Severity::Error => ti(tokens.error),
        Severity::Warning => ti(tokens.warning),
        Severity::Info => r.info,
        Severity::Off => r.line_strong,
    }
}

/// Open Electrical Rules Check…: the ERC dialog. The Project tab makes its
/// project the active one, so the dialog edits this project's rules.
pub(super) fn open_erc_message() -> Message {
    Message::Erc(ErcMsg::OpenDialog)
}

fn annotate_words(choice: AnnotateUndo) -> MessageId {
    match choice {
        AnnotateUndo::Project => ids::PROJECT_ANNOTATE_UNDO_PROJECT,
        AnnotateUndo::PerSheet => ids::PROJECT_ANNOTATE_UNDO_PER_SHEET,
    }
}

pub(in crate::app::project_tab) fn view<'a>(
    project: &LoadedProject,
    facts: &ErcFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let body = vec![
        rules_card(project.id, facts, i18n, tokens),
        annotation_card(project, facts.machine_annotate_undo, i18n, tokens),
    ];
    section(
        ids::PROJECT_SECTION_ERC_ANNOTATION,
        ids::PROJECT_ERC_ANNOTATION_HELP,
        body,
        i18n,
        tokens,
    )
}

/// The ERC rules card: the source pill in the header, the source line, and
/// for a readable rule set the severity tiles and the edit hint.
fn rules_card<'a>(
    project_id: ProjectId,
    facts: &ErcFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (words, tone) = source_pill(facts.source);
    let header = CardHeader {
        title: t!(i18n, ids::PROJECT_ERC_TITLE),
        trailing: vec![pill(t!(i18n, words), tone, None, tokens)],
    };
    let mut body: Column<Element<'a, Message>> =
        column![source_line(project_id, facts.source, i18n, tokens)].spacing(SP.l);
    if facts.source != ErcSource::Unreadable {
        body = body
            .push(severity_block(facts, i18n, tokens))
            .push(edit_hint(i18n, tokens));
    }
    card(Some(header), body.boxed(), tokens)
}

/// Where the rules come from. Machine rules carry the Store button on the
/// same line, so the fix sits next to the problem.
fn source_line<'a>(
    project_id: ProjectId,
    source: ErcSource,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let words = |id: MessageId| text(t!(i18n, id)).size(TS.body).boxed();
    match source {
        ErcSource::Project => info_line(words(ids::PROJECT_ERC_STORED), tokens),
        ErcSource::Unreadable => warn_line(
            Glyph::Warn,
            words(ids::PROJECT_ERC_UNREADABLE),
            None,
            tokens,
        ),
        ErcSource::Machine => {
            let store = tab_message(ProjectTabMsg::ErcRulesStoreRequested { project_id });
            let button = text_button(
                None,
                t!(i18n, ids::PROJECT_ERC_STORE),
                Some(store),
                ButtonKind::Small,
                tokens,
            );
            // The mockup's Store button sits on the warn tint with the
            // raised fill.
            let raised = resolve(tokens).raised;
            let button = container(button)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(raised)),
                    border: Border {
                        radius: Radii::GRAPHITE.small.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed();
            warn_line(
                Glyph::Info,
                container(words(ids::PROJECT_ERC_MACHINE))
                    .width(Length::Fill)
                    .boxed(),
                Some(button),
                tokens,
            )
        }
    }
}

/// "Rules by severity" with its caption, then one tile per severity.
fn severity_block<'a>(facts: &ErcFacts, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let caption = text(t!(
        i18n,
        ids::PROJECT_ERC_BY_SEVERITY_CAPTION,
        "total" => facts.total()
    ))
    .size(TS.caption)
    .color(resolve(tokens).ink4);
    let label = row![
        field_label(t!(i18n, ids::PROJECT_ERC_BY_SEVERITY), tokens),
        caption,
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let mut tiles: Row<Element<'a, Message>> = Row::new().spacing(SP.m);
    for (severity, count) in facts.counts {
        tiles = tiles.push(tile(severity, count, i18n, tokens));
    }
    column![label, tiles].spacing(SP.s + SP.xs).boxed()
}

/// One severity tile: the rule count in mono, then a dot and its words.
fn tile<'a>(
    severity: Severity,
    count: usize,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let figure = text(t!(i18n, ids::PROJECT_ERC_RULE_COUNT, "count" => count))
        .size(TS.display)
        .font(Font {
            weight: Weight::Medium,
            ..mono_font()
        })
        .color(ti(tokens.text));
    let words = row![
        dot(tile_dot(severity, tokens), SP.m),
        text(t!(i18n, tile_words(severity)))
            .size(TS.body)
            .color(resolve(tokens).ink3),
    ]
    .spacing(SP.s + SP.xs)
    .align_y(Alignment::Center);
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(column![figure, words])
        .width(Length::Fill)
        .padding([SP.m + SP.xs, SP.l])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A round dot of `size` px in `colour`.
fn dot<'a>(colour: Color, size: f32) -> Element<'a, Message> {
    container(Space::new().width(size).height(size))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Where severities are changed, and the button that opens it.
fn edit_hint<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    row![
        text(t!(i18n, ids::PROJECT_ERC_EDIT_HINT))
            .size(TS.body)
            .color(resolve(tokens).ink4)
            .width(Length::Fill),
        text_button(
            None,
            t!(i18n, ids::PROJECT_ERC_OPEN_DIALOG),
            Some(open_erc_message()),
            ButtonKind::Ghost,
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The Annotation card: Annotate undo as a label and its radios, with the
/// help under them.
fn annotation_card<'a>(
    project: &LoadedProject,
    machine: AnnotateUndo,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let header = CardHeader {
        title: t!(i18n, ids::PROJECT_ANNOTATE_TITLE),
        trailing: Vec::new(),
    };
    let field = column![
        annotate_choice(project, machine, i18n),
        text(t!(i18n, ids::PROJECT_ANNOTATE_UNDO_HELP))
            .size(TS.caption)
            .color(resolve(tokens).ink4),
    ]
    .spacing(SP.m);
    let body = kv_row(t!(i18n, ids::PROJECT_ANNOTATE_UNDO), field.boxed(), tokens);
    card(Some(header), body, tokens)
}

/// The three Annotate undo choices: the Preferences default (named with its
/// current value), or one of the two values stored in the project.
fn annotate_choice<'a>(
    project: &LoadedProject,
    machine: AnnotateUndo,
    i18n: &I18n,
) -> Element<'a, Message> {
    let project_id = project.id;
    let selected = Some(project.data.annotate_undo());
    let pick = move |choice: Option<AnnotateUndo>| {
        tab_message(ProjectTabMsg::AnnotateUndoPicked { project_id, choice })
    };
    let machine_words = t!(i18n, annotate_words(machine)).into_string();
    let default = t!(i18n, ids::PROJECT_ANNOTATE_UNDO_DEFAULT, "value" => machine_words);
    let mut choices = column![
        radio(default, None, selected, pick)
            .size(TS.body)
            .text_size(TS.body)
    ]
    .spacing(SP.m);
    for choice in AnnotateUndo::ALL.iter().copied() {
        choices = choices.push(
            radio(
                t!(i18n, annotate_words(choice)),
                Some(choice),
                selected,
                pick,
            )
            .size(TS.body)
            .text_size(TS.body)
            .boxed(),
        );
    }
    choices.boxed()
}
