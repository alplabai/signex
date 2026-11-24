//! Parameters section view (mockup row 30): a filter, then one
//! bordered list with a Title block and a Variables group. A value box is
//! outlined only on hover or focus; a variable row shows its remove button
//! on hover; an `=` expression value shows its result in a green pill beside
//! the value, or its error in an error-filled row (#1987). Pure: reads the
//! project and the tab state, emits `ProjectTabMsg`s.

use std::collections::BTreeMap;

use iced::widget::{Column, Row, Space, column, container, hover, row, text_input as input};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _,
};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_types::variables::{FieldText, TextTable, classify, variable_text};
use signex_widgets::{button, text, text_input};

use super::rename::{RenameNote, problem_words, rename_note};
use super::{ParameterGroup, VisibleParameters, field_words, variable_count, visible_parameters};
use crate::app::Message;
use crate::app::project_tab::general::tab_message;
use crate::app::project_tab::look::{
    ButtonKind, Glyph, glyph, icon_button, panel_pill, pill, text_button,
};
use crate::app::project_tab::table::{SP, TS, section};
use crate::app::project_tab::{ProjectTabMsg, ProjectTabState};
use crate::app::state::{LoadedProject, ProjectId};
use crate::panels::TitleBlockField;
use crate::styles::tokens::{danger, hover_overlay, resolve};
use crate::styles::{ChromeMetrics, HAIRLINE, PillTone, Radii, ti};

/// Width of the filter field, in `body`-size characters (260 px).
const FILTER_EMS: f32 = 21.7;
/// Width of the name column and of the new name field (180 px).
const NAME_EMS: f32 = 15.0;
/// Width of the remove column (28 px).
const REMOVE_EMS: f32 = 2.33;
/// Width of the bar at the left of a refused row, in hairlines.
const ERROR_BAR_HAIRLINES: f32 = 2.0;

const METRICS: ChromeMetrics = ChromeMetrics::DEFAULT;
const RADII: Radii = Radii::GRAPHITE;

/// The colours the list draws with, resolved once per view.
#[derive(Debug, Clone, Copy)]
struct Look {
    ink: Color,
    ink3: Color,
    ink4: Color,
    line: Color,
    line_soft: Color,
    sunken: Color,
    field: Color,
    accent: Color,
    hover: Color,
    err: Color,
    err_soft: Color,
    warn: Color,
    warn_soft: Color,
}

impl Look {
    fn of(tokens: &ThemeTokens) -> Self {
        let r = resolve(tokens);
        Self {
            ink: ti(tokens.text),
            ink3: r.ink3,
            ink4: r.ink4,
            line: ti(tokens.border),
            line_soft: r.line_soft,
            sunken: r.sunken,
            field: r.field,
            accent: ti(tokens.accent),
            hover: hover_overlay(tokens),
            err: danger(tokens),
            err_soft: r.err_soft,
            warn: ti(tokens.warning),
            warn_soft: r.warn_soft,
        }
    }
}

/// What every row of one view shares.
struct Rows<'s> {
    project_id: ProjectId,
    i18n: &'s I18n,
    tokens: &'s ThemeTokens,
    look: Look,
}

/// What an `=` expression value shows on its row (mockup row 30). A literal
/// value shows as typed and has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Evaluation {
    /// The result, in a green pill beside the value.
    Shows(String),
    /// Why it does not evaluate, under the value in an error-filled row
    /// with a "?" pill beside the value.
    Error(String),
}

/// What the parameter `name` evaluates to or why it does not (#1987).
pub(super) fn evaluation(parameters: &BTreeMap<String, String>, name: &str) -> Option<Evaluation> {
    let stored = parameters.get(name)?;
    if let FieldText::Literal(_) = classify(stored) {
        return None;
    }
    Some(match variable_text(&TextTable(parameters), name)? {
        Ok(value) => Evaluation::Shows(value),
        Err(error) => Evaluation::Error(error.to_string()),
    })
}

/// The words of an expression's result pill and whether it is the error
/// "?" of one that does not evaluate (mockup row 30).
pub(super) fn result_words(
    i18n: &I18n,
    evaluated: Option<&Evaluation>,
) -> Option<(Localized, bool)> {
    Some(match evaluated? {
        Evaluation::Shows(value) => (
            t!(i18n, ids::PROJECT_PARAMETER_SHOWS, "value" => value.clone()),
            false,
        ),
        Evaluation::Error(_) => (t!(i18n, ids::PROJECT_PARAMETER_UNRESOLVED), true),
    })
}

pub(in crate::app::project_tab) fn view<'a>(
    project: &'a LoadedProject,
    tab: Option<&'a ProjectTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let rows = Rows {
        project_id: project.id,
        i18n,
        tokens,
        look: Look::of(tokens),
    };
    let parameters = &project.data.parameters;
    let filter = tab.map_or("", |tab| tab.parameter_filter.as_str());
    let shown = visible_parameters(parameters, filter, |field| {
        t!(i18n, field_words(field)).into_string()
    });
    let totals = t!(
        i18n,
        ids::PROJECT_PARAMETERS_TOTALS,
        "title_block" => TitleBlockField::ALL.len(),
        "variables" => variable_count(parameters)
    );
    let body = vec![
        rows.filter_row(filter, totals),
        rows.list(project, tab, shown),
    ];
    section(
        ids::PROJECT_SECTION_PARAMETERS,
        ids::PROJECT_PARAMETERS_HINT,
        body,
        i18n,
        tokens,
    )
}

impl Rows<'_> {
    /// The filter field and, at the right, the totals.
    fn filter_row<'a>(&self, filter: &str, totals: Localized) -> Element<'a, Message> {
        let (project_id, look) = (self.project_id, self.look);
        let field = text_input(
            t!(self.i18n, ids::PROJECT_PARAMETERS_FILTER),
            filter.to_string(),
        )
        .on_input(move |text| {
            tab_message(ProjectTabMsg::ParameterFilterEdited { project_id, text })
        })
        .style(move |_: &Theme, _| bare_input(look))
        .size(TS.body)
        .padding(0)
        .width(Length::Fill);
        let framed = container(
            row![glyph(Glyph::Search, TS.body, look.ink4), field]
                .spacing(SP.s)
                .align_y(Alignment::Center),
        )
        .padding([0.0, SP.m])
        .height(METRICS.field)
        .width(TS.body * FILTER_EMS)
        .align_y(Alignment::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(look.field)),
            border: Border {
                width: HAIRLINE,
                color: look.line,
                radius: RADII.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
        row![
            framed,
            Space::new().width(Length::Fill),
            text(totals).size(TS.caption).color(look.ink4),
        ]
        .align_y(Alignment::Center)
        .boxed()
    }

    /// The bordered list: each group's header, then its rows when open.
    fn list<'a>(
        &self,
        project: &'a LoadedProject,
        tab: Option<&'a ProjectTabState>,
        shown: VisibleParameters<'a>,
    ) -> Element<'a, Message> {
        let look = self.look;
        let open = |group| tab.is_none_or(|tab| tab.is_parameter_group_open(group));
        let parameters = &project.data.parameters;
        let mut list: Column<Element<'a, Message>> = Column::new().width(Length::Fill);

        let title_block_open = open(ParameterGroup::TitleBlock);
        list = list.push(self.group_header(
            ParameterGroup::TitleBlock,
            title_block_open,
            shown.title_block.len(),
        ));
        if title_block_open {
            let cells = shown.title_block.iter().map(|field| {
                let value = parameters.get(field.key()).map_or("", String::as_str);
                let evaluated = evaluation(parameters, field.key());
                let shows = self.result_pill(evaluated.as_ref());
                let line = self.title_block_row(*field, value, shows);
                self.noted(
                    line,
                    self.error_note(evaluated.as_ref()).into_iter().collect(),
                )
            });
            list = push_separated(list, cells, look);
        }

        let variables_open = open(ParameterGroup::Variables);
        list = list.push(self.group_header(
            ParameterGroup::Variables,
            variables_open,
            shown.variables.len(),
        ));
        if variables_open {
            list = self.push_variables(list, parameters, tab, &shown.variables);
        }

        container(list)
            .width(Length::Fill)
            .clip(true)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: HAIRLINE,
                    color: look.line,
                    radius: RADII.button.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    }

    /// The Variables group's rows, each with its rename draft and note,
    /// then the Name field and Add at its end.
    fn push_variables<'a>(
        &self,
        mut list: Column<Element<'a, Message>>,
        parameters: &BTreeMap<String, String>,
        tab: Option<&ProjectTabState>,
        variables: &[&str],
    ) -> Column<Element<'a, Message>> {
        let draft = tab.and_then(|tab| tab.parameter_rename.as_ref());
        let refused = tab.is_some_and(|tab| tab.parameter_rename_refused);
        let cells = variables.iter().map(|&name| {
            let value = parameters.get(name).map_or("", String::as_str);
            let typed = draft
                .filter(|(from, _)| from.as_str() == name)
                .map(|(_, text)| text.as_str());
            let rename = typed.and_then(|text| {
                rename_note(parameters, name, text, refused)
                    .map(|note| self.note_words(note, name, text))
            });
            let evaluated = evaluation(parameters, name);
            let notes = rename
                .into_iter()
                .chain(self.error_note(evaluated.as_ref()))
                .collect();
            let shows = self.result_pill(evaluated.as_ref());
            let line = self.variable_line(name, typed, value, shows);
            self.noted(line, notes)
        });
        list = push_separated(list, cells, self.look);
        list = list.push(hairline(self.look.line_soft));
        let typed = tab.map_or("", |tab| tab.new_parameter_name.as_str());
        list.push(self.add_row(typed))
    }

    /// A group header: chevron, name, count and, for the title block, the
    /// note that sheets override it. Clicking it folds the group.
    fn group_header<'a>(
        &self,
        group: ParameterGroup,
        open: bool,
        count: usize,
    ) -> Element<'a, Message> {
        let (project_id, look) = (self.project_id, self.look);
        let (name, note) = match group {
            ParameterGroup::TitleBlock => (
                ids::PROJECT_PARAMETERS_GROUP_TITLE_BLOCK,
                Some(ids::PROJECT_PARAMETERS_SHEETS_OVERRIDE),
            ),
            ParameterGroup::Variables => (ids::PROJECT_PARAMETERS_GROUP_VARIABLES, None),
        };
        let chevron = if open {
            Glyph::ChevronDown
        } else {
            Glyph::ChevronRight
        };
        let mut line = row![
            glyph(chevron, TS.body, look.ink4),
            text(t!(self.i18n, name)).size(TS.section).color(look.ink3),
            text(t!(self.i18n, ids::PROJECT_PARAMETERS_COUNT, "count" => count))
                .size(TS.caption)
                .font(mono())
                .color(look.ink3),
            Space::new().width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .height(METRICS.row);
        if let Some(note) = note {
            line = line.push(
                text(t!(self.i18n, note))
                    .size(TS.caption)
                    .color(look.ink4)
                    .boxed(),
            );
        }
        let header = button(line)
            .width(Length::Fill)
            .padding([0.0, SP.m])
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(look.sunken)),
                text_color: look.ink3,
                ..iced::widget::button::Style::default()
            })
            .on_press(tab_message(ProjectTabMsg::ParameterGroupToggled {
                project_id,
                group,
            }));
        column![header, hairline(look.line_soft)].boxed()
    }

    /// A title-block field: its name, then its value box and, for an
    /// expression, its result pill.
    fn title_block_row<'a>(
        &self,
        field: TitleBlockField,
        value: &str,
        shows: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let project_id = self.project_id;
        let key = field.key();
        let value_box = self
            .inline_input(value, false)
            .on_input(move |value| {
                tab_message(ProjectTabMsg::ParameterEdited {
                    project_id,
                    name: key.to_string(),
                    value,
                })
            })
            .on_submit(tab_message(ProjectTabMsg::ParameterSubmitted {
                project_id,
            }));
        let name = container(
            text(t!(self.i18n, field_words(field)))
                .size(TS.body)
                .color(self.look.ink3)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([0.0, SP.m])
        .width(TS.body * NAME_EMS)
        .clip(true);
        let line = value_line(name.boxed(), value_box, shows);
        hover(line, wash(self.look, None)).boxed()
    }

    /// The result pill of an expression: the green "Shows …" pill, or the
    /// error "?" on the panel fill when it does not evaluate.
    fn result_pill<'a>(&self, evaluated: Option<&Evaluation>) -> Option<Element<'a, Message>> {
        let (words, is_error) = result_words(self.i18n, evaluated)?;
        Some(if is_error {
            panel_pill(words, PillTone::Err, self.tokens)
        } else {
            pill(words, PillTone::Ok, None, self.tokens)
        })
    }

    /// The "Error: …" note of an expression that does not evaluate, drawn
    /// as a refusal (error fill and ink).
    fn error_note(&self, evaluated: Option<&Evaluation>) -> Option<(Localized, bool)> {
        let Some(Evaluation::Error(error)) = evaluated else {
            return None;
        };
        let words = t!(self.i18n, ids::PROJECT_PARAMETER_ERROR, "error" => error.clone());
        Some((words, true))
    }

    /// The words of a rename note and whether it is a refusal (error ink)
    /// rather than the references warning (warning ink).
    fn note_words(&self, note: RenameNote, from: &str, typed: &str) -> (Localized, bool) {
        match note {
            RenameNote::Refused(problem) => (problem_words(problem, typed, self.i18n), true),
            RenameNote::ReferencesKeepOldName => (
                t!(self.i18n, ids::PROJECT_PARAMETER_RENAME_REFERENCES, "name" => from.to_string()),
                false,
            ),
        }
    }

    /// The name box (Enter renames), value box, result pill of an
    /// expression and hover remove button of a variable.
    fn variable_line<'a>(
        &self,
        name: &str,
        typed: Option<&str>,
        value: &str,
        shows: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let (project_id, look) = (self.project_id, self.look);
        let from = name.to_string();
        let name_box = self
            .inline_input(typed.unwrap_or(name), true)
            .on_input(move |text| {
                tab_message(ProjectTabMsg::ParameterRenameEdited {
                    project_id,
                    from: from.clone(),
                    text,
                })
            })
            .on_submit(tab_message(ProjectTabMsg::ParameterRenameSubmitted {
                project_id,
            }));
        let key = name.to_string();
        let value_box = self
            .inline_input(value, false)
            .on_input(move |value| {
                tab_message(ProjectTabMsg::ParameterEdited {
                    project_id,
                    name: key.clone(),
                    value,
                })
            })
            .on_submit(tab_message(ProjectTabMsg::ParameterSubmitted {
                project_id,
            }));
        let name_cell = container(name_box)
            .padding(Padding::ZERO.left(SP.s))
            .width(TS.body * NAME_EMS)
            .boxed();
        let line = value_line(name_cell, value_box, shows);
        let remove = icon_button(
            Glyph::Close,
            t!(self.i18n, ids::PROJECT_PARAMETERS_REMOVE),
            Some(tab_message(ProjectTabMsg::ParameterRemoved {
                project_id,
                name: name.to_string(),
            })),
            self.tokens,
        );
        hover(line, wash(look, Some(remove))).boxed()
    }

    /// `line` with each note under it, over the soft fill with a bar at the
    /// left: error colours when a note is a refusal or an expression error,
    /// warning colours otherwise. No notes leaves `line` as it is.
    fn noted<'a>(
        &self,
        line: Element<'a, Message>,
        notes: Vec<(Localized, bool)>,
    ) -> Element<'a, Message> {
        if notes.is_empty() {
            return line;
        }
        let look = self.look;
        let tone = |is_refusal: bool| {
            if is_refusal {
                (look.err, look.err_soft)
            } else {
                (look.warn, look.warn_soft)
            }
        };
        let (ink, soft) = tone(notes.iter().any(|(_, is_refusal)| *is_refusal));
        let mut noted: Column<Element<'a, Message>> = Column::new()
            .padding(Padding::ZERO.bottom(SP.xs))
            .push(line);
        for (words, is_refusal) in notes {
            let note_ink = tone(is_refusal).0;
            let reason = row![
                glyph(Glyph::Warn, TS.caption, note_ink),
                text(words).size(TS.caption).color(note_ink),
            ]
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .padding([0.0, SP.m]);
            noted = noted.push(reason.boxed());
        }
        let bar = container(Space::new())
            .width(HAIRLINE * ERROR_BAR_HAIRLINES)
            .height(Length::Fill)
            .style(move |_: &Theme| filled(ink));
        container(row![bar, noted])
            .width(Length::Fill)
            .style(move |_: &Theme| filled(soft))
            .boxed()
    }

    /// The new variable's name field and Add, at the end of Variables.
    fn add_row<'a>(&self, typed: &str) -> Element<'a, Message> {
        let project_id = self.project_id;
        let field = text_input(
            t!(self.i18n, ids::PLACEHOLDER_PROJECT_PARAMETER_NAME),
            typed.to_string(),
        )
        .on_input(move |text| {
            tab_message(ProjectTabMsg::NewParameterNameEdited { project_id, text })
        })
        .on_submit(tab_message(ProjectTabMsg::ParameterAdded { project_id }))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding([SP.xs, SP.m])
        .width(TS.body * NAME_EMS);
        let add = text_button(
            Some(Glyph::Plus),
            t!(self.i18n, ids::PROJECT_PARAMETERS_ADD),
            Some(tab_message(ProjectTabMsg::ParameterAdded { project_id })),
            ButtonKind::Outline,
            self.tokens,
        );
        row![field, add]
            .spacing(SP.m)
            .padding([SP.s, SP.m])
            .align_y(Alignment::Center)
            .boxed()
    }

    /// A name or value box drawn as plain text until hovered or focused, in
    /// the face [`inline_font`] picks.
    fn inline_input<'a>(&self, value: &str, is_name: bool) -> input::TextInput<'a, Message> {
        let look = self.look;
        let placeholder = if is_name {
            t!(self.i18n, ids::PLACEHOLDER_PROJECT_PARAMETER_NAME)
        } else {
            t!(self.i18n, ids::PROJECT_PARAMETERS_NOT_SET)
        };
        let field = text_input(placeholder, value.to_string())
            .style(move |_: &Theme, status| inline_style(look, status))
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .width(Length::Fill);
        match inline_font(value, is_name) {
            Some(font) => field.font(font),
            None => field,
        }
    }
}

/// The face of an inline box: a set value or name is mono; an empty value
/// reads "Not set" in the italic UI face (mockup row 30); an empty name
/// keeps its placeholder in the UI face.
pub(super) fn inline_font(value: &str, is_name: bool) -> Option<Font> {
    if !value.is_empty() {
        Some(mono())
    } else if is_name {
        None
    } else {
        Some(Font {
            style: iced::font::Style::Italic,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
    }
}

/// A parameter row: `name`, the value box and, right of it, the result pill
/// of an expression, then the remove column.
fn value_line<'a>(
    name: Element<'a, Message>,
    value_box: input::TextInput<'a, Message>,
    shows: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut line: Row<Element<'a, Message>> = Row::new()
        .align_y(Alignment::Center)
        .height(METRICS.field)
        .push(name)
        .push(container(value_box).width(Length::Fill).boxed());
    if let Some(shows) = shows {
        line = line.push(container(shows).padding([0.0, SP.s]).boxed());
    }
    line.push(Space::new().width(TS.body * REMOVE_EMS).boxed())
        .boxed()
}

/// Push `rows` with a soft hairline between each two.
fn push_separated<'a>(
    mut list: Column<Element<'a, Message>>,
    rows: impl Iterator<Item = Element<'a, Message>>,
    look: Look,
) -> Column<Element<'a, Message>> {
    for (index, line) in rows.enumerate() {
        if index > 0 {
            list = list.push(hairline(look.line_soft));
        }
        list = list.push(line);
    }
    list
}

/// The hover layer of a row: the hover fill and, at the right, `remove`.
fn wash<'a>(look: Look, remove: Option<Element<'a, Message>>) -> Element<'a, Message> {
    let mut top = row![Space::new().width(Length::Fill)]
        .height(Length::Fill)
        .align_y(Alignment::Center);
    if let Some(remove) = remove {
        top = top.push(container(remove).center_x(TS.body * REMOVE_EMS).boxed());
    }
    container(top)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| filled(look.hover))
        .boxed()
}

fn hairline<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(move |_: &Theme| filled(color))
        .boxed()
}

fn filled(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        ..container::Style::default()
    }
}

fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A text box with no frame: the filter field inside its own frame.
fn bare_input(look: Look) -> input::Style {
    input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: look.ink4,
        value: look.ink,
        selection: Color {
            a: 0.4,
            ..look.accent
        },
    }
}

/// An inline box: no frame at rest, the field fill and a hairline on hover,
/// the accent frame while focused.
fn inline_style(look: Look, status: input::Status) -> input::Style {
    let (background, frame) = match status {
        input::Status::Focused { .. } => (look.field, look.accent),
        input::Status::Hovered => (look.field, look.line),
        input::Status::Active | input::Status::Disabled => (Color::TRANSPARENT, Color::TRANSPARENT),
    };
    input::Style {
        background: Background::Color(background),
        border: Border {
            width: HAIRLINE,
            color: frame,
            radius: RADII.small.into(),
            ..Border::default()
        },
        ..bare_input(look)
    }
}
