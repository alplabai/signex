//! The power tree of a project (#1313, mockup row 171): the run behind
//! `query_power_tree` and the block diagram tab's Power tree view, which
//! stitches the project off the UI thread and reads its rails, converters,
//! loads and the power budget per sheet (the budget of the block that
//! stands for the sheet). The tree itself is built in
//! `signex_net::power_tree`. Rules: `docs/POWER_TREE.md`.

mod answer;
mod job;

use signex_types::project::is_base_variant;

use crate::app::state::LoadedProject;

/// The variant a run reads: `named`, else the project's active one;
/// `None` is Base. Refused for a variant the project does not have.
pub(crate) fn power_tree_variant(
    project: &LoadedProject,
    named: Option<&str>,
) -> Result<Option<String>, String> {
    let variant = match named {
        Some(name) => name.to_string(),
        None => project.data.active_variant.clone().unwrap_or_default(),
    };
    let variant = (!variant.trim().is_empty() && !is_base_variant(&variant)).then_some(variant);
    if let Some(name) = &variant
        && !project.data.variant_definitions.iter().any(|v| v == name)
    {
        return Err(format!("the project has no variant `{name}`"));
    }
    Ok(variant)
}
