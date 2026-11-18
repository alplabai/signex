//! Sheet-variable rows of Document Options ▸ Parameters (#1987): edit the
//! active sheet's `variables`, which shadow the project's variables in
//! `${NAME}` special strings and `=` expressions.
//!
//! A value commits on Enter as one undoable edit; a half-typed value is a
//! draft in the shared store, as a title-block field's is. Each row shows
//! what its value evaluates to, or why it does not.

use super::super::*;
use super::graphite_table::ParameterRow;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{ids, t};
use signex_types::variables::{FieldText, Layered, Resolver, TextTable, classify, variable_text};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One edit of a sheet variable in the Parameters tab.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetVariableMsg {
    /// A keystroke in a variable's value (kept as a draft).
    ValueTyped { name: String, text: String },
    /// Enter in a variable's value: its draft is written to the sheet.
    ValueSubmitted(String),
    /// A variable's Remove button.
    Removed(String),
    /// A keystroke in the Add row's name.
    NewNameTyped(String),
    /// The Add row's button, or Enter in its name: the typed name is added
    /// with an empty value.
    Added,
}

fn msg(m: SheetVariableMsg) -> PanelMsg {
    PanelMsg::SheetVariables(m)
}

/// The active sheet's variables as Parameters rows: the value field, its
/// Remove button and, for an expression, what it shows or why it does not.
pub(super) fn sheet_variable_rows<'a>(ctx: &'a PanelContext) -> Vec<ParameterRow<'a>> {
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let error_color = crate::styles::ti(ctx.tokens.error);
    let i18n = &ctx.i18n;
    let sheet = ctx.title_block_sheet.as_deref();
    let (sheet_table, project_table) = (
        TextTable(&ctx.sheet_variables),
        TextTable(&ctx.project_parameters),
    );
    let model = Layered::new(vec![&sheet_table as &dyn Resolver, &project_table]);
    let mut rows = Vec::with_capacity(ctx.sheet_variables.len());
    for (name, stored) in &ctx.sheet_variables {
        let value = sheet
            .and_then(|s| ctx.drafts.sheet_variable_text(s, Some(name.as_str())))
            .unwrap_or_else(|| stored.clone());
        let key = name.clone();
        let placeholder = signex_i18n::UserText::new(String::new());
        let input = signex_widgets::text_input(placeholder, value)
            .on_input(move |text| {
                msg(SheetVariableMsg::ValueTyped {
                    name: key.clone(),
                    text,
                })
            })
            .on_submit(msg(SheetVariableMsg::ValueSubmitted(name.clone())))
            .size(TS.body)
            .padding(SP.xs)
            .style(crate::panels::form_look::text_input_style)
            .width(Length::Fill);
        let remove = signex_widgets::button(
            text(t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_REMOVE)).size(TS.caption),
        )
        .padding([SP.xs, SP.s])
        .style(crate::styles::outline_button(&ctx.tokens))
        .on_press(msg(SheetVariableMsg::Removed(name.clone())));
        let field = row![input, remove]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center);
        let mut cell: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs);
        cell = cell.width(Length::Fill).push(field.boxed());
        // What the stored value shows, for an expression only: a literal
        // shows as typed.
        let shown = match classify(stored) {
            FieldText::Literal(_) => None,
            FieldText::Expression(_) => variable_text(&model, name),
        };
        let note = match shown {
            Some(Ok(value)) => Some((
                t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_SHOWS, "value" => value),
                muted,
            )),
            Some(Err(error)) => {
                let error = error.to_string();
                Some((
                    t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_ERROR, "error" => error),
                    error_color,
                ))
            }
            None => None,
        };
        if let Some((note, color)) = note {
            cell = cell.push(text(note).size(TS.caption).color(color).boxed());
        }
        rows.push(ParameterRow {
            name: signex_i18n::UserText::new(name.clone()).into(),
            value: container(cell)
                .padding([2.0, SP.xs])
                .width(Length::Fill)
                .boxed(),
        });
    }
    rows
}

/// The row that adds a sheet variable: a name field and its Add button.
pub(super) fn add_sheet_variable_row<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let new_name = ctx
        .title_block_sheet
        .as_deref()
        .and_then(|s| ctx.drafts.sheet_variable_text(s, None))
        .unwrap_or_default();
    let name_input =
        signex_widgets::text_input(t!(i18n, ids::PLACEHOLDER_SHEET_VARIABLE_NAME), new_name)
            .on_input(|text| msg(SheetVariableMsg::NewNameTyped(text)))
            .on_submit(msg(SheetVariableMsg::Added))
            .size(TS.body)
            .padding(SP.s)
            .style(crate::panels::form_look::text_input_style)
            .width(Length::Fill);
    let add =
        signex_widgets::button(text(t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_ADD)).size(TS.caption))
            .padding([SP.xs, SP.s])
            .style(crate::styles::outline_button(&ctx.tokens))
            .on_press(msg(SheetVariableMsg::Added));
    container(
        row![name_input, add]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}
