//! The parameters table card (mockup row 189): Name · Expression · Value · Unit ·
//! Used by, a row menu, the autocomplete under the row being typed, the
//! in-use delete prompt, and Add parameter. An overlay at the canvas's top
//! right, under the sketch info bar; empty while closed.

use std::path::PathBuf;

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Alignment, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ListKind, Localized, UserText, ids, t};
use signex_sketch::parameter_refs::{
    AttributeField, ParameterNameError, ParameterUse, PatternKind, RowValue, resolve_rows,
    uses_by_parameter,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text, text_input, theme_ext};

use super::SketchParametersMsg;
use super::state::{DisplayUnit, ParametersTableState, display_value, suggestions};
use crate::app::FootprintDocument;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::{DialogButtonKind, Radii, dialog_button};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Column widths, in body text heights; Used by takes the rest.
const NAME_EM: f32 = 8.0;
const EXPRESSION_EM: f32 = 16.0;
const VALUE_EM: f32 = 6.0;
const UNIT_EM: f32 = 4.0;
const MENU_EM: f32 = 2.0;
/// The card's width and the tallest its row list grows, in body heights.
const CARD_EM: f32 = 56.0;
const ROWS_MAX_EM: f32 = 30.0;
/// The card's distance from the canvas top: under the mode switcher and the
/// sketch info bar.
pub const TOP_CLEARANCE_EM: f32 = 7.0;

/// Builds the messages the card sends, for the editor at `path`.
#[derive(Clone)]
struct Sender(PathBuf);

impl Sender {
    fn msg(&self, msg: SketchParametersMsg) -> LibraryMessage {
        LibraryMessage::PrimitiveEditorEvent {
            path: self.0.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchParameters(msg)),
        }
    }
}

/// One row as shown.
struct Row {
    name: String,
    source: String,
    value: RowValue,
    uses: Vec<ParameterUse>,
}

/// The colours the card draws with.
#[derive(Clone, Copy)]
struct Look {
    ink: Color,
    muted: Color,
    name: Color,
    error: Color,
    focus: Color,
    line: Color,
}

impl Look {
    fn of(tokens: &ThemeTokens) -> Self {
        let r = crate::styles::tokens::resolve(tokens);
        Self {
            ink: theme_ext::text_primary(tokens),
            muted: r.ink4,
            name: r.info,
            error: theme_ext::error_color(tokens),
            focus: r.accent_soft,
            line: theme_ext::border_color(tokens),
        }
    }
}

/// The table card, or nothing while it is closed.
pub fn overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some(table) = editor.state.parameters_table.as_deref() else {
        return Space::new().boxed();
    };
    let send = Sender(editor.path.clone());
    let look = Look::of(tokens);
    let rows = rows_of(editor);

    let mut list = column![].spacing(SP.xs);
    if rows.is_empty() {
        list = list.push(caption(t!(i18n, ids::SKETCH_PARAMETERS_EMPTY), look.muted));
    }
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    for row in &rows {
        list = list.push(row_view(
            row, table, &rows, &names, &send, look, tokens, i18n,
        ));
    }

    let card = column![
        header(&send, look, tokens, i18n),
        column_titles(look, i18n),
        scrollable(list).height(Length::Fit.max(TS.body * ROWS_MAX_EM)),
        footer(&send, look, tokens, i18n),
    ]
    .spacing(SP.m);
    let card = container(card)
        .padding(SP.l)
        .width(Length::Fixed(TS.body * CARD_EM))
        .style(crate::styles::modal_card(tokens));
    container(column![
        Space::new().height(TS.body * TOP_CLEARANCE_EM),
        card
    ])
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .boxed()
}

fn rows_of(editor: &FootprintDocument) -> Vec<Row> {
    let Some(sketch) = editor.primitive().sketch.as_ref() else {
        return Vec::new();
    };
    let mut uses = uses_by_parameter(sketch);
    resolve_rows(&sketch.parameters)
        .into_iter()
        .map(|(name, value)| Row {
            source: sketch
                .parameters
                .get_raw(&name)
                .unwrap_or_default()
                .to_string(),
            uses: uses.remove(&name).unwrap_or_default(),
            name,
            value,
        })
        .collect()
}

fn header<'a>(
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    row![
        text(t!(i18n, ids::SKETCH_PARAMETERS_TITLE))
            .size(TS.title)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..signex_widgets::ui_font::regular()
            })
            .color(look.ink),
        Space::new().width(Length::Fill),
        dialog_button(
            t!(i18n, ids::SKETCH_PARAMETERS_CLOSE),
            DialogButtonKind::Ghost,
            Some(send.msg(SketchParametersMsg::TableClosed)),
            tokens,
        ),
    ]
    .align_y(Alignment::Center)
    .boxed()
}

fn column_titles<'a>(look: Look, i18n: &I18n) -> Element<'a, LibraryMessage> {
    let title = |id, em: Option<f32>| {
        let width = em.map_or(Length::Fill, |em| Length::Fixed(TS.body * em));
        text(t!(i18n, id))
            .size(TS.caption)
            .color(look.muted)
            .width(width)
            .boxed()
    };
    container(
        row![
            title(ids::SKETCH_PARAMETERS_COL_NAME, Some(NAME_EM)),
            title(ids::SKETCH_PARAMETERS_COL_EXPRESSION, Some(EXPRESSION_EM)),
            title(ids::SKETCH_PARAMETERS_COL_VALUE, Some(VALUE_EM)),
            title(ids::SKETCH_PARAMETERS_COL_UNIT, Some(UNIT_EM)),
            title(ids::SKETCH_PARAMETERS_COL_USED_BY, None),
            Space::new().width(TS.body * MENU_EM),
        ]
        .spacing(SP.m),
    )
    .padding([SP.xs, SP.s])
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            color: look.line,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

#[allow(
    clippy::too_many_arguments,
    reason = "one row reads the table, the other rows' names and the look"
)]
fn row_view<'a>(
    row: &Row,
    table: &ParametersTableState,
    rows: &[Row],
    names: &[&str],
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let focused = table.focus.as_deref() == Some(row.name.as_str());
    let main = row![
        name_cell(row, table, send, look, tokens, i18n),
        expression_cell(row, send, look, tokens, i18n),
        value_cell(row, table, look, i18n),
        unit_cell(row, table, look, i18n),
        text(used_by_text(&row.uses, i18n))
            .size(TS.caption)
            .color(look.muted)
            .width(Length::Fill),
        button(
            text(t!(i18n, ids::SKETCH_PARAMETERS_ROW_MENU))
                .size(TS.body)
                .color(look.ink)
                .align_x(iced::alignment::Horizontal::Center)
                .width(Length::Fill),
        )
        .width(Length::Fixed(TS.body * MENU_EM))
        .padding(SP.xs)
        .style(crate::styles::ghost_button(tokens))
        .on_press(send.msg(SketchParametersMsg::RowMenuToggled {
            name: row.name.clone(),
        })),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Start);
    let mut block = column![main].spacing(SP.xs);
    if focused {
        let offered = suggestions(&row.source, &row.name, names.iter().copied());
        if !offered.is_empty() {
            block = block.push(autocomplete(row, &offered, rows, send, look, tokens, i18n));
        }
    }
    if table.menu.as_deref() == Some(row.name.as_str()) {
        block = block.push(row_menu(row, send, tokens, i18n));
    }
    if table.delete_prompt.as_deref() == Some(row.name.as_str()) {
        block = block.push(delete_prompt(row, send, look, tokens, i18n));
    }
    let background = focused.then_some(look.focus);
    container(block)
        .padding([SP.xs, SP.s])
        .style(move |_: &Theme| container::Style {
            background: background.map(Into::into),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn name_cell<'a>(
    row: &Row,
    table: &ParametersTableState,
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let width = Length::Fixed(TS.body * NAME_EM);
    let Some(rename) = table.rename.as_ref().filter(|r| r.from == row.name) else {
        return text(UserText::new(row.name.as_str()))
            .size(TS.dense)
            .font(mono())
            .color(look.name)
            .width(width)
            .boxed();
    };
    let field = text_input(
        t!(i18n, ids::SKETCH_PARAMETERS_RENAME_PLACEHOLDER),
        rename.draft.clone(),
    )
    .size(TS.dense)
    .font(mono())
    .padding(SP.xs)
    .style(field_style(tokens, rename.error.is_some()))
    .on_input({
        let send = send.clone();
        move |draft| send.msg(SketchParametersMsg::RenameDraftChanged(draft))
    })
    .on_submit(send.msg(SketchParametersMsg::RenameSubmitted));
    let mut cell = column![field].spacing(SP.xs).width(width);
    if let Some(error) = &rename.error {
        cell = cell.push(caption(name_error(error, i18n), look.error));
    }
    cell.boxed()
}

fn expression_cell<'a>(
    row: &Row,
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let problem = match &row.value {
        RowValue::Value(_) => None,
        RowValue::Error(error) => Some(t!(
            i18n,
            ids::SKETCH_PARAMETERS_ERROR,
            "reason" => error.to_string()
        )),
        RowValue::Blocked { by } => Some(t!(
            i18n,
            ids::SKETCH_PARAMETERS_ERROR_BLOCKED,
            "name" => by.as_str()
        )),
    };
    let name = row.name.clone();
    let send = send.clone();
    let field = text_input(
        t!(i18n, ids::SKETCH_PARAMETERS_EXPRESSION_PLACEHOLDER),
        row.source.clone(),
    )
    .size(TS.dense)
    .font(mono())
    .padding(SP.xs)
    .style(field_style(tokens, matches!(row.value, RowValue::Error(_))))
    .on_input(move |expr| {
        send.msg(SketchParametersMsg::ExpressionEdited {
            name: name.clone(),
            expr,
        })
    });
    let mut cell = column![field]
        .spacing(SP.xs)
        .width(Length::Fixed(TS.body * EXPRESSION_EM));
    if let Some(problem) = problem {
        cell = cell.push(caption(problem, look.error));
    }
    cell.boxed()
}

/// The value the row shows: its own, or the last good one in the error
/// colour while it has none.
fn shown_value(row: &Row, table: &ParametersTableState) -> Option<(String, DisplayUnit, bool)> {
    match &row.value {
        RowValue::Value(q) => {
            let (value, unit) = display_value(*q);
            Some((value, unit, false))
        }
        RowValue::Error(_) | RowValue::Blocked { .. } => {
            let (value, unit) = display_value(*table.last_good.get(&row.name)?);
            Some((value, unit, true))
        }
    }
}

fn value_cell<'a>(
    row: &Row,
    table: &ParametersTableState,
    look: Look,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let (label, color) = match shown_value(row, table) {
        Some((value, _, false)) => (
            t!(i18n, ids::SKETCH_PARAMETERS_VALUE, "value" => value),
            look.ink,
        ),
        Some((value, _, true)) => (
            t!(i18n, ids::SKETCH_PARAMETERS_LAST_GOOD, "value" => value),
            look.error,
        ),
        None => (t!(i18n, ids::SKETCH_PARAMETERS_NO_VALUE), look.muted),
    };
    text(label)
        .size(TS.dense)
        .font(mono())
        .color(color)
        .align_x(iced::alignment::Horizontal::Right)
        .width(Length::Fixed(TS.body * VALUE_EM))
        .boxed()
}

fn unit_cell<'a>(
    row: &Row,
    table: &ParametersTableState,
    look: Look,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let width = Length::Fixed(TS.body * UNIT_EM);
    let Some((_, unit, _)) = shown_value(row, table) else {
        return Space::new().width(width).boxed();
    };
    text(t!(i18n, unit_id(unit)))
        .size(TS.dense)
        .color(look.ink)
        .width(width)
        .boxed()
}

/// What reads a parameter, one entry per object, as the locale's list.
fn used_by_text(uses: &[ParameterUse], i18n: &I18n) -> Localized {
    let mut labels: Vec<String> = Vec::new();
    for site in uses {
        let label = use_label(site, i18n).into_string();
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        return t!(i18n, ids::SKETCH_PARAMETERS_USED_BY_NONE);
    }
    t!(
        i18n,
        ids::SKETCH_PARAMETERS_USED_BY_LIST,
        "uses" => i18n.list(&labels, ListKind::And)
    )
}

fn use_label(site: &ParameterUse, i18n: &I18n) -> Localized {
    match site {
        ParameterUse::Parameter { name } => {
            t!(i18n, ids::SKETCH_PARAMETERS_USE_PARAMETER, "name" => name.as_str())
        }
        ParameterUse::PlaneOffset { .. } => t!(i18n, ids::SKETCH_PARAMETERS_USE_PLANE),
        ParameterUse::Dimension { angle: false, .. } => {
            t!(i18n, ids::SKETCH_PARAMETERS_USE_DISTANCE)
        }
        ParameterUse::Dimension { angle: true, .. } => t!(i18n, ids::SKETCH_PARAMETERS_USE_ANGLE),
        ParameterUse::Pattern { kind, .. } => t!(
            i18n,
            match kind {
                PatternKind::Linear => ids::SKETCH_PARAMETERS_USE_LINEAR,
                PatternKind::Grid => ids::SKETCH_PARAMETERS_USE_GRID,
                PatternKind::Circular => ids::SKETCH_PARAMETERS_USE_CIRCULAR,
            }
        ),
        ParameterUse::Pad { number, .. } => {
            t!(i18n, ids::SKETCH_PARAMETERS_USE_PAD, "number" => number.as_str())
        }
        ParameterUse::Attribute { field, .. } => t!(
            i18n,
            match field {
                AttributeField::PourClearance
                | AttributeField::PourMinThickness
                | AttributeField::ThermalGap
                | AttributeField::ThermalSpokeWidth => ids::SKETCH_PARAMETERS_USE_POUR,
                AttributeField::CutoutEdgeRadius => ids::SKETCH_PARAMETERS_USE_CUTOUT,
                AttributeField::MountingHoleDrill => ids::SKETCH_PARAMETERS_USE_HOLE,
                AttributeField::VScoreDepth | AttributeField::VScoreMinWeb => {
                    ids::SKETCH_PARAMETERS_USE_V_SCORE
                }
            }
        ),
    }
}

fn name_error(error: &ParameterNameError, i18n: &I18n) -> Localized {
    match error {
        ParameterNameError::Empty => t!(i18n, ids::SKETCH_PARAMETERS_NAME_EMPTY),
        ParameterNameError::NotAName(_) => t!(i18n, ids::SKETCH_PARAMETERS_NAME_INVALID),
        ParameterNameError::Reserved(name) => {
            t!(i18n, ids::SKETCH_PARAMETERS_NAME_RESERVED, "name" => name.as_str())
        }
        ParameterNameError::Taken(name) => {
            t!(i18n, ids::SKETCH_PARAMETERS_NAME_TAKEN, "name" => name.as_str())
        }
        ParameterNameError::Missing(name) => {
            t!(i18n, ids::SKETCH_PARAMETERS_NAME_MISSING, "name" => name.as_str())
        }
    }
}

/// The names offered under the row being typed, with their values.
fn autocomplete<'a>(
    row: &Row,
    offered: &[super::state::Suggestion],
    rows: &[Row],
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut list = column![caption(
        t!(i18n, ids::SKETCH_PARAMETERS_AUTOCOMPLETE_TITLE),
        look.muted
    )]
    .spacing(SP.xs);
    for suggestion in offered {
        let value = rows
            .iter()
            .find(|r| r.name == suggestion.name)
            .and_then(|r| match r.value {
                RowValue::Value(q) => Some(display_value(q)),
                RowValue::Error(_) | RowValue::Blocked { .. } => None,
            });
        let mut line = row![
            text(UserText::new(suggestion.name.as_str()))
                .size(TS.dense)
                .font(mono())
                .color(look.name),
            Space::new().width(Length::Fill),
        ]
        .spacing(SP.m);
        if let Some((value, unit)) = value {
            line = line.push(
                text(t!(i18n, ids::SKETCH_PARAMETERS_VALUE, "value" => value))
                    .size(TS.caption)
                    .color(look.muted)
                    .boxed(),
            );
            line = line.push(
                text(t!(i18n, unit_id(unit)))
                    .size(TS.caption)
                    .color(look.muted)
                    .boxed(),
            );
        }
        list = list.push(
            button(line)
                .width(Length::Fill)
                .padding([SP.xs, SP.s])
                .style(crate::styles::ghost_button(tokens))
                .on_press(send.msg(SketchParametersMsg::ExpressionEdited {
                    name: row.name.clone(),
                    expr: suggestion.completed.clone(),
                }))
                .boxed(),
        );
    }
    popup(list.boxed(), tokens)
}

fn unit_id(unit: DisplayUnit) -> signex_i18n::MessageId {
    match unit {
        DisplayUnit::Mm => ids::SKETCH_PARAMETERS_UNIT_MM,
        DisplayUnit::Deg => ids::SKETCH_PARAMETERS_UNIT_DEG,
        DisplayUnit::Count => ids::SKETCH_PARAMETERS_UNIT_COUNT,
    }
}

/// Rename… and Delete (Caner 2026-11-20).
fn row_menu<'a>(
    row: &Row,
    send: &Sender,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let menu = row![
        dialog_button(
            t!(i18n, ids::SKETCH_PARAMETERS_RENAME),
            DialogButtonKind::Ghost,
            Some(send.msg(SketchParametersMsg::RenameStarted {
                name: row.name.clone(),
            })),
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::SKETCH_PARAMETERS_DELETE),
            DialogButtonKind::Danger,
            Some(send.msg(SketchParametersMsg::DeleteRequested {
                name: row.name.clone(),
            })),
            tokens,
        ),
    ]
    .spacing(SP.s);
    container(popup(menu.boxed(), tokens))
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .boxed()
}

/// Deleting a used parameter asks first, naming its uses.
fn delete_prompt<'a>(
    row: &Row,
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let question = t!(
        i18n,
        ids::SKETCH_PARAMETERS_DELETE_PROMPT,
        "name" => row.name.as_str(),
        "uses" => used_by_text(&row.uses, i18n).into_string()
    );
    let prompt = column![
        text(question).size(TS.body).color(look.ink),
        row![
            Space::new().width(Length::Fill),
            dialog_button(
                t!(i18n, ids::SKETCH_PARAMETERS_CANCEL),
                DialogButtonKind::Cancel,
                Some(send.msg(SketchParametersMsg::DeleteCancelled)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::SKETCH_PARAMETERS_REPLACE_WITH_VALUE),
                DialogButtonKind::Danger,
                Some(send.msg(SketchParametersMsg::DeleteConfirmed)),
                tokens,
            ),
        ]
        .spacing(SP.s),
    ]
    .spacing(SP.s);
    popup(prompt.boxed(), tokens)
}

fn footer<'a>(
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    row![
        dialog_button(
            t!(i18n, ids::SKETCH_PARAMETERS_ADD),
            DialogButtonKind::Secondary,
            Some(send.msg(SketchParametersMsg::AddClicked)),
            tokens,
        ),
        caption(t!(i18n, ids::SKETCH_PARAMETERS_UNITS_HINT), look.muted),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

fn caption<'a>(label: impl Into<Label<'a>>, color: Color) -> Element<'a, LibraryMessage> {
    text(label).size(TS.caption).color(color).boxed()
}

/// A raised card inside the table: the autocomplete, row menu or prompt.
fn popup<'a>(
    content: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let (raised, line) = (r.raised, r.line_strong);
    container(content)
        .padding(SP.s)
        .style(move |_: &Theme| container::Style {
            background: Some(raised.into()),
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

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A form field, its border and text in the error colour when `bad`.
pub(crate) fn field_style(
    tokens: &ThemeTokens,
    bad: bool,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    let r = crate::styles::tokens::resolve(tokens);
    let (field, line_strong, placeholder) = (r.field, r.line_strong, r.ink4);
    let accent = theme_ext::accent(tokens);
    let line = theme_ext::border_color(tokens);
    let error = theme_ext::error_color(tokens);
    let ink = theme_ext::text_primary(tokens);
    move |_: &Theme, status: text_input::Status| {
        let border = match status {
            _ if bad => error,
            text_input::Status::Focused { .. } => accent,
            text_input::Status::Hovered => line_strong,
            _ => line,
        };
        text_input::Style {
            background: field.into(),
            border: Border {
                width: 1.0,
                radius: Radii::GRAPHITE.control.into(),
                color: border,
                ..Border::default()
            },
            placeholder,
            value: if bad { error } else { ink },
            selection: Color { a: 0.4, ..accent },
        }
    }
}
