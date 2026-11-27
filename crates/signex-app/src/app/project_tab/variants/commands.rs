//! The Variant Manager's top bar actions as queries (#2949), the agent and
//! CLI surface of Compare variants and Open in BOM Studio:
//! `compare_variants {document, from?, to?}` answers the comparison the
//! Variant Manager shows and `variant_bom {document, name?}` the BOM lines
//! Open in BOM Studio opens on. Both read the project's sheets as the BOM
//! export does (an open sheet of the project names the scope) and change
//! nothing. Shapes: `docs/PROJECT_TAB.md`, "Commands".

use serde_json::{Value, json};
use signex_output::BomOptions;
use signex_types::project::is_base_variant;

use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::state::ProjectId;

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

impl Signex {
    /// `compare_variants {document, from?, to?}`: `from` defaults to Base,
    /// `to` to the active variant (the first definition while Base is
    /// active). Refused for a project with no variant besides Base, an
    /// unknown name, or no open sheet.
    pub(crate) fn compare_variants(&mut self, args: &CommandArgs) -> Result<Value, String> {
        let id = self.rules_project_named(args)?;
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or("the project is no longer open")?;
        let data = &project.data;
        let Some(first) = data.variant_definitions.first() else {
            return Err("the project has no variant besides Base to compare".into());
        };
        let from = text(args, "from").map(str::to_string);
        let to = text(args, "to").map(str::to_string).or_else(|| {
            let active = data.active_variant_name();
            Some(if is_base_variant(active) {
                first.clone()
            } else {
                active.to_string()
            })
        });
        let definitions = data.variant_definitions.clone();
        let settings = data.variant_settings.clone();
        let path = project.path.display().to_string();
        let ctx = self.project_parts_context(id)?;
        let compared = signex_output::variant_matrix(&ctx, &definitions)
            .compare(&settings, from.as_deref(), to.as_deref())
            .map_err(|why| why.to_string())?;
        let mut answer = serde_json::to_value(&compared).map_err(|why| why.to_string())?;
        if let Value::Object(map) = &mut answer {
            map.insert("project".into(), json!(path));
        }
        Ok(answer)
    }

    /// `variant_bom {document, name?}`: the BOM lines of the project with
    /// `name` picked (`Base` is Base), as Open in BOM Studio opens them;
    /// none is the active variant.
    pub(crate) fn variant_bom(&mut self, args: &CommandArgs) -> Result<Value, String> {
        let id = self.rules_project_named(args)?;
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or("the project is no longer open")?;
        let data = &project.data;
        let variant = match text(args, "name") {
            None => data.active_variant_name().to_string(),
            Some(name) if is_base_variant(name) => signex_types::project::BASE_VARIANT.to_string(),
            Some(name) => data
                .variant_definitions
                .iter()
                .find(|defined| defined.eq_ignore_ascii_case(name.trim()))
                .cloned()
                .ok_or_else(|| format!("the project has no variant named `{}`", name.trim()))?,
        };
        let path = project.path.display().to_string();
        let ctx = self.project_parts_context(id)?;
        let opts = BomOptions {
            active_variant: Some(variant.clone()).filter(|name| !is_base_variant(name)),
            ..BomOptions::default()
        };
        let (table, _) = self.bom_table_of(ctx, &opts);
        let lines: Vec<Value> = table
            .rows
            .iter()
            .map(|row| {
                json!({
                    "designators": row.references,
                    "quantity": row.fitted_qty,
                    "name": row.name,
                    "value": row.value,
                    "footprint": row.footprint,
                })
            })
            .collect();
        let parts: u32 = table.rows.iter().map(|row| row.fitted_qty).sum();
        Ok(json!({
            "project": path,
            "variant": variant,
            "lines": lines,
            "parts": parts,
        }))
    }

    /// The project's sheets as its BOM export reads them, named by an open
    /// sheet of the project (`project_bom_source`).
    fn project_parts_context(&self, id: ProjectId) -> Result<signex_output::ExportContext, String> {
        let source = self
            .project_bom_source(id)
            .ok_or("no sheet of the project is open; open one first (`open_document`)")?;
        crate::app::handlers::menu::export::build_bom_export_scope_for_source(
            &self.document_state,
            &source,
        )
        .map(|(ctx, _)| ctx)
        .ok_or_else(|| "the project's sheets could not be read".to_string())
    }
}
