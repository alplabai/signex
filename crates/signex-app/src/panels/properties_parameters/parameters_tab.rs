//! Document Options ▸ Parameters: the one place a sheet's parameters are
//! seen and edited (maintainer decision 2026-11-18). The title-block fields
//! are editable rows, the sheet's built-in special strings are read-only
//! rows showing the value `signex_output::resolve` gives them, and the
//! sheet's variables are user rows with an Add row under them.

use super::super::*;
use super::general::seg_btn;
use super::graphite_table::{ParameterRow, parameter_table, read_only_value};
use super::sheet_variables::{add_sheet_variable_row, sheet_variable_rows};
use super::title_block::title_block_rows;
use crate::panels::form_look;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids, t};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A read-only row of the Parameters tab: a built-in special string of the
/// sheet, which nothing in the panel edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinParameter {
    DocumentName,
    SheetName,
    SheetNumber,
    SheetCount,
    ProjectName,
}

impl BuiltinParameter {
    pub const ALL: [Self; 5] = [
        Self::DocumentName,
        Self::SheetName,
        Self::SheetNumber,
        Self::SheetCount,
        Self::ProjectName,
    ];

    /// The special string whose resolved value the row shows.
    pub fn token(self) -> &'static str {
        match self {
            Self::DocumentName => "${FILENAME}",
            Self::SheetName => "${SHEETNAME}",
            Self::SheetNumber => "${SHEETNUMBER}",
            Self::SheetCount => "${SHEETCOUNT}",
            Self::ProjectName => "${PROJECT_NAME}",
        }
    }

    /// The row's name in the catalog.
    pub fn message(self) -> MessageId {
        match self {
            Self::DocumentName => ids::PROPERTIES_PARAMETER_DOCUMENT_NAME,
            Self::SheetName => ids::PROPERTIES_PARAMETER_SHEET_NAME,
            Self::SheetNumber => ids::PROPERTIES_PARAMETER_SHEET_NUMBER,
            Self::SheetCount => ids::PROPERTIES_PARAMETER_SHEET_COUNT,
            Self::ProjectName => ids::PROPERTIES_PARAMETER_PROJECT_NAME,
        }
    }
}

/// The Parameters tab of Document Options for the active sheet.
pub fn view_properties_parameters<'a>(
    ctx: &'a PanelContext,
    primary: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(form_look::section_label(
        t!(i18n, ids::PROPERTIES_SECTION_PARAMETERS),
        &ctx.tokens,
        i18n,
    ));

    // Sub-tabs: All | Parameters | Rules
    col = col.push(
        container(
            row![
                seg_btn(
                    t!(i18n, ids::PROPERTIES_TAB_ALL),
                    false,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                ),
                seg_btn(
                    t!(i18n, ids::PROPERTIES_TAB_PARAMETERS),
                    true,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                ),
                seg_btn(
                    t!(i18n, ids::PROPERTIES_TAB_RULES),
                    false,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                ),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([SP.s, SP.m])
        .style(form_look::segmented_frame)
        .boxed(),
    );

    let mut rows = title_block_rows(ctx);
    for (parameter, value) in &ctx.builtin_parameters {
        rows.push(ParameterRow {
            name: t!(i18n, parameter.message()).into(),
            value: read_only_value(&ctx.tokens, value.clone()),
        });
    }
    col = col.push(parameter_table(&ctx.tokens, rows));
    col = col.push(hint(t!(i18n, ids::PROPERTIES_TITLE_BLOCK_HINT), muted));

    // The sheet's own variables, under their own heading.
    let heading = form_label(t!(i18n, ids::PROPERTIES_SECTION_SHEET_VARIABLES));
    col = col.push(heading);
    let variables = sheet_variable_rows(ctx);
    if !variables.is_empty() {
        col = col.push(parameter_table(&ctx.tokens, variables));
    }
    col = col.push(add_sheet_variable_row(ctx));
    col.push(hint(t!(i18n, ids::PROPERTIES_SHEET_VARIABLES_HINT), muted))
}

/// A caption under a table.
fn hint<'a>(line: signex_i18n::Localized, muted: Color) -> Element<'a, PanelMsg> {
    container(text(line).size(TS.caption).color(muted))
        .padding([3.0, SP.m])
        .boxed()
}

#[cfg(test)]
#[path = "parameters_tab_tests.rs"]
mod tests;
