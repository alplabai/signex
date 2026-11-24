//! Sheet-variable rows of Document Options ▸ Parameters (#1987): edit the
//! active sheet's `variables`, which shadow the project's variables in
//! `${NAME}` special strings and `=` expressions.
//!
//! A value commits on Enter as one undoable edit; a half-typed value is a
//! draft in the shared store, as a title-block field's is. An expression
//! row shows its result in a pill, or an inline error under the row (#2531).

use super::super::*;
use super::graphite_table::ParameterRow;
use crate::styles::{Spacing, TypeScale, ti};
use iced::Widget;
use signex_i18n::{ids, t};
use signex_types::variables::{FieldText, Layered, Resolver, TextTable, classify, variable_text};
use signex_widgets::chips::{ChipMetrics, ChipTone, kind_chip};

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

/// What a variable's stored value shows: `None` for a literal (shown as
/// typed), else the expression's result or why it has none.
pub(super) fn variable_result(ctx: &PanelContext, name: &str) -> Option<Result<String, String>> {
    let stored = ctx.sheet_variables.get(name)?;
    if let FieldText::Literal(_) = classify(stored) {
        return None;
    }
    let (sheet_table, project_table) = (
        TextTable(&ctx.sheet_variables),
        TextTable(&ctx.project_parameters),
    );
    let model = Layered::new(vec![&sheet_table as &dyn Resolver, &project_table]);
    variable_text(&model, name).map(|shown| shown.map_err(|error| error.to_string()))
}

/// A small icon button in the ghost look.
fn icon_button<'a>(
    handle: iced::widget::svg::Handle,
    ink: Color,
    on_press: PanelMsg,
) -> Element<'a, PanelMsg> {
    let glyph = svg(handle)
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        });
    signex_widgets::button(glyph)
        .padding(SP.xs)
        .style(crate::panels::form_look::ghost_button)
        .on_press(on_press)
        .boxed()
}

/// The variables in `names` as Parameters rows: the value field, the
/// result pill and the remove button; an error line under a row whose
/// expression does not evaluate.
pub(super) fn sheet_variable_rows<'a>(
    ctx: &'a PanelContext,
    names: &[String],
) -> Vec<ParameterRow<'a>> {
    let r = crate::styles::tokens::resolve(&ctx.tokens);
    let error_color = ti(ctx.tokens.error);
    let i18n = &ctx.i18n;
    let sheet = ctx.title_block_sheet.as_deref();
    let metrics = ChipMetrics::from_text_size(TS.body);
    let mut rows = Vec::with_capacity(names.len());
    for name in names {
        let Some(stored) = ctx.sheet_variables.get(name) else {
            continue;
        };
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
        let result = variable_result(ctx, name);
        let mut field = row![input].spacing(SP.xs).align_y(iced::Alignment::Center);
        match &result {
            Some(Ok(shown)) => {
                let tone = ChipTone {
                    fill: r.accent_soft,
                    ink: r.accent_soft_ink,
                };
                let pill = kind_chip(signex_i18n::UserText::new(shown.clone()), tone, metrics);
                field = field.push(crate::panels::graphite::hint_tooltip(
                    pill,
                    t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_SHOWS, "value" => shown.clone()),
                    &ctx.tokens,
                    iced::widget::tooltip::Position::Left,
                ));
            }
            Some(Err(_)) => {
                let tone = ChipTone {
                    fill: r.err_soft,
                    ink: error_color,
                };
                let unresolved = t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_UNRESOLVED);
                field = field.push(kind_chip(unresolved, tone, metrics));
            }
            None => {}
        }
        let remove = icon_button(
            crate::icons::icon_close(ctx.theme_id),
            r.ink3,
            msg(SheetVariableMsg::Removed(name.clone())),
        );
        field = field.push(crate::panels::graphite::hint_tooltip(
            remove,
            t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_REMOVE),
            &ctx.tokens,
            iced::widget::tooltip::Position::Left,
        ));
        let mut cell: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs);
        cell = cell.width(Length::Fill).push(field.boxed());
        if let Some(Err(error)) = result {
            let line = t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_ERROR, "error" => error);
            cell = cell.push(text(line).size(TS.caption).color(error_color).boxed());
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

/// The row that adds a sheet variable at the end of its group: a name
/// field and the Add variable button; Enter in the name adds it too.
pub(super) fn add_sheet_variable_row<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let ink = ti(ctx.tokens.text);
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
    let glyph = svg(crate::icons::icon_plus(ctx.theme_id))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        });
    let add = signex_widgets::button(
        row![
            glyph,
            text(t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_ADD_ROW)).size(TS.caption)
        ]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.s])
    .style(crate::panels::form_look::ghost_button)
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
