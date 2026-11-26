//! The answer of `bom_lines` (#1988): the BOM of an open sheet's project as
//! the export dialog rolls it up (open sheets from memory, the library's
//! current part names, the cached prices, the parts the board leaves out),
//! with the columns of an optional template, computed columns included.
//! Read-only: the dialog is not touched, and the failed computed cells are
//! in the answer's `issues`, not written to Messages. See `docs/CLI.md`,
//! "BOM lines", and `docs/BOM_COLUMNS.md`.

use std::path::Path;

use serde_json::{Value, json};
use signex_output::bom::{
    BomColumn, BomGrouping, BomOptions, cell_text, columns_of_template, computed_issues,
    rollup_with_validation, template_of_columns,
};
use signex_types::bom_template::{encode_bom_template, parse_bom_template};

use crate::app::Signex;

/// A text operand has no control characters (DECISIONS 2026-11-06), so the
/// `template` operand writes each line break as `\n`; the answer's
/// `template` is written the same way, so it can be passed back.
const OPERAND_LINE_BREAK: &str = "\\n";

/// The columns a request lists.
fn requested_columns(template: Option<&str>) -> Result<Vec<BomColumn>, String> {
    match template {
        None => Ok(BomOptions::default().columns),
        Some(text) => {
            let text = text.replace(OPERAND_LINE_BREAK, "\n");
            let template = parse_bom_template(&text).map_err(|e| e.to_string())?;
            columns_of_template(&template)
        }
    }
}

/// The grouping a request names.
fn requested_grouping(grouping: Option<&str>) -> Result<BomGrouping, String> {
    match grouping.map(str::trim) {
        None | Some("grouped") => Ok(BomGrouping::Grouped),
        Some("ungrouped") => Ok(BomGrouping::Ungrouped),
        Some("flat") => Ok(BomGrouping::Flat),
        Some(other) => Err(format!(
            "unknown grouping `{other}`; use grouped, ungrouped or flat"
        )),
    }
}

/// The name a column has in the answer, with what it lists.
fn column_entry(column: &BomColumn) -> Value {
    match column {
        BomColumn::Custom(field) => {
            json!({"header": column.header(), "kind": "field", "field": field})
        }
        BomColumn::Computed(computed) => json!({
            "header": column.header(),
            "kind": "computed",
            "expression": computed.expression(),
        }),
        standard => json!({
            "header": standard.header(),
            "kind": "standard",
            "key": standard.template_key(),
        }),
    }
}

impl Signex {
    /// The variant a request names, `None` for Base. Refused for a name the
    /// project of `source` does not define.
    fn requested_variant(
        &self,
        source: &Path,
        variant: Option<&str>,
    ) -> Result<Option<String>, String> {
        let project = self.document_state.document_project(source);
        let name = match variant {
            Some(name) => name.trim().to_string(),
            None => match project.and_then(|p| p.data.active_variant.clone()) {
                Some(active) => active,
                None => return Ok(None),
            },
        };
        if signex_types::project::is_base_variant(&name) {
            return Ok(None);
        }
        let defined = project.is_some_and(|p| p.data.variant_definitions.contains(&name));
        if defined {
            Ok(Some(name))
        } else {
            Err(format!("the project defines no variant `{name}`"))
        }
    }

    /// The BOM of `source`'s scope, as JSON: with the columns of `template`
    /// (a template's text, `snxbomt/1`; the dialog's default columns without
    /// one), for `variant` (the project's active variant without one) and by
    /// `grouping` (`grouped`, the default, `ungrouped` or `flat`).
    pub(crate) fn bom_lines_answer(
        &self,
        source: &Path,
        template: Option<&str>,
        variant: Option<&str>,
        grouping: Option<&str>,
    ) -> Result<Value, String> {
        let columns = requested_columns(template)?;
        let grouping_name = grouping.map_or("grouped", str::trim);
        let grouping = requested_grouping(grouping)?;
        let variant = self.requested_variant(source, variant)?;
        let Some((mut ctx, issues)) =
            super::super::build_bom_export_scope_for_source(&self.document_state, source)
        else {
            return Err(format!("{} is not an open schematic", source.display()));
        };
        if issues.channel_naming_unreadable {
            return Err(
                "the project's channel naming cannot be read, so its designators are not known"
                    .into(),
            );
        }
        crate::library::drift::apply_library_names(&mut ctx, &self.library.drift.names);
        let opts = BomOptions {
            columns,
            grouping,
            active_variant: variant.clone(),
            price_book: crate::library::bom_prices::price_book(&self.library),
            board_left_out: self.board_left_out_of_bom(source),
            ..BomOptions::default()
        };
        let (table, _) = rollup_with_validation(&ctx, &opts);
        let lines: Vec<Value> = table
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let cells: Vec<String> = opts.columns.iter().map(|c| cell_text(row, c)).collect();
                json!({
                    "line": index + 1,
                    "references": row.references,
                    "qty": row.qty,
                    "fitted_qty": row.fitted_qty,
                    "cells": cells,
                    "computed": row.computed,
                })
            })
            .collect();
        let template = encode_bom_template(&template_of_columns(&opts.columns))
            .ok()
            .map(|text| text.replace('\n', OPERAND_LINE_BREAK));
        Ok(json!({
            "document": source.display().to_string(),
            "variant": variant.unwrap_or_else(|| signex_types::project::BASE_VARIANT.to_string()),
            "grouping": grouping_name,
            "columns": opts.columns.iter().map(column_entry).collect::<Vec<_>>(),
            "lines": lines,
            "issues": computed_issues(&table, &opts.columns),
            "warnings": issues.messages(),
            "template": template,
        }))
    }
}

#[cfg(test)]
#[path = "bom_lines_tests.rs"]
mod tests;
