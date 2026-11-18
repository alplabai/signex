//! Sheet Variables section of the sheet's Properties (#1987): edits the
//! active sheet's `variables`, which shadow the project's variables in
//! `${NAME}` special strings and `=` expressions.
//!
//! A value commits on Enter as one undoable edit; a half-typed value is a
//! draft in the shared store, as a title-block field's is. Each row shows
//! what its value evaluates to, or why it does not.

use super::super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_types::variables::{FieldText, Layered, Resolver, TextTable, classify, variable_text};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One edit in the Sheet Variables section.
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

/// The Sheet Variables collapsible section for the active sheet.
pub fn view_sheet_variables_section<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    use signex_i18n::{ids, t};
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let error_color = crate::styles::ti(ctx.tokens.error);
    let i18n = &ctx.i18n;
    let sheet = ctx.title_block_sheet.as_deref();
    ctx.collapsible(
        "prop_sheet_variables",
        ids::PROPERTIES_SECTION_SHEET_VARIABLES,
        &ctx.collapsed_sections,
        move || {
            let (sheet_table, project_table) = (
                TextTable(&ctx.sheet_variables),
                TextTable(&ctx.project_parameters),
            );
            let model = Layered::new(vec![&sheet_table as &dyn Resolver, &project_table]);
            let mut c = Column::new().spacing(0).width(Length::Fill);
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
                    .padding(SP.s)
                    .style(crate::panels::form_look::text_input_style)
                    .width(Length::FillPortion(PROPERTY_CONTROL_PORTION));
                let remove = signex_widgets::button(
                    text(t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_REMOVE)).size(TS.caption),
                )
                .padding([SP.xs, SP.s])
                .style(crate::styles::outline_button(&ctx.tokens))
                .on_press(msg(SheetVariableMsg::Removed(name.clone())));
                c = c.push(
                    container(
                        row![
                            property_label(signex_i18n::UserText::new(name.clone()), muted),
                            input,
                            remove,
                        ]
                        .spacing(SP.m)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([2, PROPERTY_ROW_PAD_X])
                    .width(Length::Fill)
                    .boxed(),
                );
                // What the stored value shows, for an expression only: a
                // literal shows as typed.
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
                    c = c.push(
                        container(text(note).size(TS.caption).color(color))
                            .padding([0.0, SP.m])
                            .boxed(),
                    );
                }
            }
            let new_name = sheet
                .and_then(|s| ctx.drafts.sheet_variable_text(s, None))
                .unwrap_or_default();
            let name_input = signex_widgets::text_input(
                t!(i18n, ids::PLACEHOLDER_SHEET_VARIABLE_NAME),
                new_name,
            )
            .on_input(|text| msg(SheetVariableMsg::NewNameTyped(text)))
            .on_submit(msg(SheetVariableMsg::Added))
            .size(TS.body)
            .padding(SP.s)
            .style(crate::panels::form_look::text_input_style)
            .width(Length::Fill);
            let add = signex_widgets::button(
                text(t!(i18n, ids::PROPERTIES_SHEET_VARIABLE_ADD)).size(TS.caption),
            )
            .padding([SP.xs, SP.s])
            .style(crate::styles::outline_button(&ctx.tokens))
            .on_press(msg(SheetVariableMsg::Added));
            c = c.push(
                container(
                    row![name_input, add]
                        .spacing(SP.m)
                        .align_y(iced::Alignment::Center),
                )
                .padding([2, PROPERTY_ROW_PAD_X])
                .width(Length::Fill)
                .boxed(),
            );
            c = c.push(
                container(
                    text(t!(i18n, ids::PROPERTIES_SHEET_VARIABLES_HINT))
                        .size(TS.caption)
                        .color(muted),
                )
                .padding([3.0, SP.m])
                .boxed(),
            );
            c
        },
    )
    .boxed()
}
