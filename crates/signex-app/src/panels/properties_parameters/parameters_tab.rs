//! Document Options ▸ Parameters: the one place a sheet's parameters are
//! seen and edited (maintainer decision 2026-11-18, #2531). A filter field
//! and All / Parameters / Rules chips with counts sit over three groups:
//! the title block (editable, a row inherited from the project marked with a
//! pill), the sheet's variables (with their results and an Add row) and the
//! read-only built-ins, which show the value `signex_output::resolve` gives
//! them.

use super::super::*;
use super::document_options::{DocumentOptionsMsg, ParametersFilter, msg};
use super::graphite_table::{ParameterRow, group_header, parameter_table, read_only_value};
use super::sheet_variables::{add_sheet_variable_row, sheet_variable_rows};
use super::title_block::{TitleBlockField, title_block_rows};
use crate::styles::{Spacing, TypeScale, ti};
use iced::Widget;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Fold keys of the three groups.
const TITLE_BLOCK_GROUP: &str = "param_group_title_block";
const VARIABLES_GROUP: &str = "param_group_sheet_variables";
const BUILT_IN_GROUP: &str = "param_group_built_in";

/// A read-only row of the Parameters tab: a built-in special string of the
/// sheet, which nothing in the panel edits. The clock rows show the time the
/// panel was last built (`DocumentTimes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinParameter {
    CurrentTime,
    CurrentDate,
    Time,
    DocumentFullPathAndName,
    DocumentName,
    ModifiedDate,
    SheetName,
    SheetNumber,
    SheetCount,
    ProjectName,
}

impl BuiltinParameter {
    /// Every built-in, in the order the Built-in group lists them: the
    /// sheet's names and numbers first, then the clock and file rows.
    pub const ALL: [Self; 10] = [
        Self::DocumentName,
        Self::SheetName,
        Self::SheetNumber,
        Self::SheetCount,
        Self::ProjectName,
        Self::CurrentTime,
        Self::CurrentDate,
        Self::Time,
        Self::DocumentFullPathAndName,
        Self::ModifiedDate,
    ];

    /// The special string whose resolved value the row shows.
    pub fn token(self) -> &'static str {
        match self {
            Self::CurrentTime => "${CurrentTime}",
            Self::CurrentDate => "${CurrentDate}",
            Self::Time => "${Time}",
            Self::DocumentFullPathAndName => "${DocumentFullPathAndName}",
            Self::DocumentName => "${DocumentName}",
            Self::ModifiedDate => "${ModifiedDate}",
            Self::SheetName => "${SheetName}",
            Self::SheetNumber => "${SheetNumber}",
            Self::SheetCount => "${SheetCount}",
            Self::ProjectName => "${ProjectName}",
        }
    }

    /// The row's name in the catalog.
    pub fn message(self) -> MessageId {
        match self {
            Self::CurrentTime => ids::PROPERTIES_PARAMETER_CURRENT_TIME,
            Self::CurrentDate => ids::PROPERTIES_PARAMETER_CURRENT_DATE,
            Self::Time => ids::PROPERTIES_PARAMETER_TIME,
            Self::DocumentFullPathAndName => ids::PROPERTIES_PARAMETER_DOCUMENT_FULL_PATH,
            Self::DocumentName => ids::PROPERTIES_PARAMETER_DOCUMENT_NAME,
            Self::ModifiedDate => ids::PROPERTIES_PARAMETER_MODIFIED_DATE,
            Self::SheetName => ids::PROPERTIES_PARAMETER_SHEET_NAME,
            Self::SheetNumber => ids::PROPERTIES_PARAMETER_SHEET_NUMBER,
            Self::SheetCount => ids::PROPERTIES_PARAMETER_SHEET_COUNT,
            Self::ProjectName => ids::PROPERTIES_PARAMETER_PROJECT_NAME,
        }
    }
}

/// The rows the Parameters list shows for a filter text, by group, and the
/// counts its chips carry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParameterListing {
    pub title_block: Vec<TitleBlockField>,
    pub variables: Vec<String>,
    pub builtins: Vec<BuiltinParameter>,
    /// Rules set on the sheet: none yet, so the Rules chip shows the empty
    /// card.
    pub rules: usize,
}

impl ParameterListing {
    /// The rows whose name holds `query`, ignoring case; every row when it
    /// is blank.
    pub fn filtered<'s>(
        i18n: &I18n,
        query: &str,
        variables: impl IntoIterator<Item = &'s String>,
        builtins: impl IntoIterator<Item = BuiltinParameter>,
    ) -> Self {
        let query = i18n.fold_for_search(query.trim());
        let matches = |name: &str| query.is_empty() || i18n.fold_for_search(name).contains(&query);
        let named = |id: MessageId| i18n.message_plain(id, &[]);
        Self {
            title_block: TitleBlockField::ALL
                .into_iter()
                .filter(|f| matches(named(f.message()).as_str()))
                .collect(),
            variables: variables
                .into_iter()
                .filter(|name| matches(name.as_str()))
                .cloned()
                .collect(),
            builtins: builtins
                .into_iter()
                .filter(|b| matches(named(b.message()).as_str()))
                .collect(),
            rules: 0,
        }
    }

    /// The listing of the active sheet in `ctx`.
    pub fn of(ctx: &PanelContext) -> Self {
        Self::filtered(
            &ctx.i18n,
            &ctx.document_options.parameters_query,
            ctx.sheet_variables.keys(),
            ctx.builtin_parameters
                .iter()
                .map(|(parameter, _)| *parameter),
        )
    }

    pub fn parameter_count(&self) -> usize {
        self.title_block.len() + self.variables.len() + self.builtins.len()
    }

    /// The count a filter chip carries.
    pub fn count(&self, filter: ParametersFilter) -> usize {
        match filter {
            ParametersFilter::All => self.parameter_count() + self.rules,
            ParametersFilter::Parameters => self.parameter_count(),
            ParametersFilter::Rules => self.rules,
        }
    }
}

/// The Graphite chip look on `tokens`.
fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = crate::styles::tokens::resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// The filter field over the list.
fn filter_field<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let input = signex_widgets::text_input(
        t!(ctx.i18n, ids::PROPERTIES_PARAMETERS_FILTER),
        ctx.document_options.parameters_query.clone(),
    )
    .on_input(|query| msg(DocumentOptionsMsg::ParametersQueryChanged(query)))
    .size(TS.body)
    .padding(SP.s)
    .style(crate::panels::form_look::text_input_style)
    .width(Length::Fill);
    container(input)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// All / Parameters / Rules, each with its count.
fn filter_chips<'a>(ctx: &'a PanelContext, listing: &ParameterListing) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let picked = ctx.document_options.parameters_filter;
    let metrics = ChipMetrics::from_text_size(TS.body);
    let style = chip_style(&ctx.tokens);
    let mut chips: iced::widget::Row<Element<'a, PanelMsg>> =
        iced::widget::Row::new().spacing(metrics.row_gap);
    for (filter, id) in [
        (ParametersFilter::All, ids::PROPERTIES_TAB_ALL),
        (ParametersFilter::Parameters, ids::PROPERTIES_TAB_PARAMETERS),
        (ParametersFilter::Rules, ids::PROPERTIES_TAB_RULES),
    ] {
        let count = u32::try_from(listing.count(filter)).unwrap_or(u32::MAX);
        chips = chips.push(chip(
            t!(i18n, id),
            Some(count),
            filter == picked,
            style,
            metrics,
            i18n,
            Some(msg(DocumentOptionsMsg::ParametersFilterPicked(filter))),
        ));
    }
    container(chips)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// The read-only rows of the Built-in group.
fn builtin_rows<'a>(ctx: &'a PanelContext, shown: &[BuiltinParameter]) -> Vec<ParameterRow<'a>> {
    shown
        .iter()
        .filter_map(|wanted| {
            let (parameter, value) = ctx.builtin_parameters.iter().find(|(p, _)| p == wanted)?;
            let value = crate::panels::graphite::hint_tooltip(
                read_only_value(&ctx.tokens, value.clone()),
                t!(ctx.i18n, ids::PROPERTIES_PARAMETERS_READ_ONLY_TIP),
                &ctx.tokens,
                iced::widget::tooltip::Position::Top,
            );
            Some(ParameterRow {
                name: t!(ctx.i18n, parameter.message()).into(),
                value,
            })
        })
        .collect()
}

/// A caption under the list.
fn hint<'a>(line: signex_i18n::Localized, muted: Color) -> Element<'a, PanelMsg> {
    container(text(line).size(TS.caption).color(muted))
        .padding([3.0, SP.m])
        .boxed()
}

/// The Parameters tab of Document Options for the active sheet.
pub fn view_properties_parameters<'a>(
    ctx: &'a PanelContext,
    _primary: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let muted = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    let listing = ParameterListing::of(ctx);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(filter_field(ctx));
    col = col.push(filter_chips(ctx, &listing));

    if ctx.document_options.parameters_filter == ParametersFilter::Rules {
        if listing.rules == 0 {
            let card = crate::panels::empty_state::empty_state_card(
                &ctx.tokens,
                t!(i18n, ids::PROPERTIES_PARAMETERS_NO_RULES),
                t!(i18n, ids::PROPERTIES_PARAMETERS_NO_RULES_BODY),
                None,
            );
            col = col.push(container(card).padding([SP.s, SP.m]).boxed());
        }
        return col;
    }

    let filtering = !ctx.document_options.parameters_query.trim().is_empty();
    if filtering && listing.parameter_count() == 0 {
        col = col.push(hint(t!(i18n, ids::PROPERTIES_PARAMETERS_NO_MATCH), muted));
    }
    if !listing.title_block.is_empty() {
        col = col.push(group_header(
            ctx,
            TITLE_BLOCK_GROUP,
            t!(i18n, ids::PROPERTIES_PARAMETERS_GROUP_TITLE_BLOCK),
            listing.title_block.len(),
            None,
        ));
        if !ctx.collapsed_sections.contains(TITLE_BLOCK_GROUP) {
            let rows = title_block_rows(ctx, &listing.title_block);
            col = col.push(parameter_table(&ctx.tokens, rows));
        }
    }
    // The variables group keeps its Add row while nothing filters it away.
    if !listing.variables.is_empty() || !filtering {
        col = col.push(group_header(
            ctx,
            VARIABLES_GROUP,
            t!(i18n, ids::PROPERTIES_PARAMETERS_GROUP_SHEET_VARIABLES),
            listing.variables.len(),
            None,
        ));
        if !ctx.collapsed_sections.contains(VARIABLES_GROUP) {
            let rows = sheet_variable_rows(ctx, &listing.variables);
            if !rows.is_empty() {
                col = col.push(parameter_table(&ctx.tokens, rows));
            }
            col = col.push(add_sheet_variable_row(ctx));
        }
    }
    if !listing.builtins.is_empty() {
        col = col.push(group_header(
            ctx,
            BUILT_IN_GROUP,
            t!(i18n, ids::PROPERTIES_PARAMETERS_GROUP_BUILT_IN),
            listing.builtins.len(),
            Some(t!(i18n, ids::PROPERTIES_PARAMETERS_READ_ONLY)),
        ));
        if !ctx.collapsed_sections.contains(BUILT_IN_GROUP) {
            let rows = builtin_rows(ctx, &listing.builtins);
            col = col.push(parameter_table(&ctx.tokens, rows));
        }
    }
    col = col.push(hint(t!(i18n, ids::PROPERTIES_TITLE_BLOCK_HINT), muted));
    col.push(hint(t!(i18n, ids::PROPERTIES_PARAMETERS_FOOTER), muted))
}

#[cfg(test)]
#[path = "parameters_tab_tests.rs"]
mod tests;
