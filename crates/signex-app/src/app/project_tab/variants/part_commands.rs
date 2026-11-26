//! The Variant Manager's part edits as catalog commands (#3038), the agent
//! and CLI surface of a cell's fit, Alternate, a parameter override and
//! Duplicate (Set as default is `project_set_default_variant`). A part is
//! named by `designator` as the Variant Manager lists it (`CH2_R5` for a
//! channel's copy), read now from the project's sheets as the tab reads
//! them, so the setting lands on the placed part the matrix links. Each edit
//! runs the same section command as the tab, one step of the General
//! history, and answers with the Variants section and the part's settings.
//! Shapes: `docs/PROJECT_TAB.md`, "Commands".

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectSectionCommand;
use signex_i18n::{I18n, Translatable, ids};
use signex_output::{VariantMatrix, VariantRow};
use signex_types::project::{
    BASE_VARIANT, FOOTPRINT_PARAMETER, MPN_PARAMETER, PartRef, PlacedPart, VALUE_PARAMETER,
    is_base_variant,
};

use super::super::commands::{Ran, refused, variants_edit, variants_unchanged};
use super::cells::{copy_name, fit_repeats_sheets, sheet_fits_in};
use super::read_failed;
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;

/// A refusal with a catalog message.
fn declined(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(on)) => Some(*on),
        _ => None,
    }
}

/// The part an edit names: its project, the variant as written, and the
/// parts read with the part's row.
struct NamedPart {
    project: ProjectId,
    variant: String,
    matrix: VariantMatrix,
    row: usize,
}

impl NamedPart {
    fn row(&self) -> &VariantRow {
        &self.matrix.rows[self.row]
    }

    fn part_ref(&self) -> PartRef {
        let row = self.row();
        PartRef::new(row.designator.clone(), row.link.clone())
    }
}

/// Read the project's parts and find the part `designator` names; the
/// refusal when the project or the part cannot be found.
fn named_part(app: &mut Signex, args: &CommandArgs) -> Result<NamedPart, Ran> {
    let project = app.rules_project_named(args).map_err(refused)?;
    let variant = text(args, "variant").unwrap_or_default().to_string();
    let designator = text(args, "designator").unwrap_or_default().trim();
    let Some(loaded) = app.document_state.project_by_id(project) else {
        return Err(refused("the project is no longer open"));
    };
    let matrix = app
        .read_variant_parts_now(loaded)
        .map_err(|why| refused(read_failed(&I18n::en_us(), &why).to_string()))?;
    let Some(row) = matrix
        .rows
        .iter()
        .position(|row| row.designator == designator)
    else {
        let reason = Translatable::new(ids::VARIANT_COMMAND_NO_PART);
        return Err(declined(reason.arg("designator", designator.to_string())));
    };
    Ok(NamedPart {
        project,
        variant,
        matrix,
        row,
    })
}

/// `ran` with `parts` and what the variant now sets on each added to its
/// answer.
fn with_parts(app: &Signex, named: &NamedPart, parts: &[PartRef], ran: Ran) -> Ran {
    let (mut answer, task) = match ran {
        (CommandOutcome::Answered(answer), task) => (answer, task),
        other => return other,
    };
    let settings = app
        .document_state
        .project_by_id(named.project)
        .map(|project| project.data.variant_settings.clone())
        .unwrap_or_default();
    let listed: Vec<Value> = parts
        .iter()
        .map(|part| {
            let own = settings.part(Some(named.variant.as_str()), part.placed());
            json!({
                "designator": part.designator,
                "fitted": own.and_then(|own| own.fitted),
                "parameters": own.map(|own| own.parameters.clone()).unwrap_or_default(),
            })
        })
        .collect();
    if let Value::Object(map) = &mut answer {
        map.insert("parts".into(), Value::Array(listed));
    }
    (CommandOutcome::Answered(answer), task)
}

/// Whether `scope` asks for every channel's copy (`channels`) or the copy
/// named (`part`, the default); refused for anything else.
fn every_channel(args: &CommandArgs) -> Result<bool, Ran> {
    match text(args, "scope") {
        None | Some("part") => Ok(false),
        Some("channels") => Ok(true),
        Some(_) => Err(declined(Translatable::new(ids::VARIANT_COMMAND_BAD_SCOPE))),
    }
}

/// The parts an edit names: the copy `named` lists, or every channel's copy
/// of it; refused for every channel of a part on a sheet that is not
/// repeated.
fn edited_parts(named: &NamedPart, every_channel: bool) -> Result<Vec<PartRef>, Ran> {
    if !every_channel {
        return Ok(vec![named.part_ref()]);
    }
    let Some(channel) = &named.row().channel else {
        let reason = Translatable::new(ids::VARIANT_COMMAND_NOT_REPEATED);
        return Err(declined(
            reason.arg("designator", named.row().designator.clone()),
        ));
    };
    Ok(named
        .matrix
        .channel_copies(&channel.part)
        .iter()
        .map(|row| PartRef::new(row.designator.clone(), row.link.clone()))
        .collect())
}

/// `project_set_variant_fit {document, variant, designator, fitted?,
/// scope?}`: the part fitted or not in `variant`, or as on its sheets with
/// no `fitted`. `scope` `channels` sets every channel's copy of a part on a
/// repeated sheet at once; `part` (the default) the copy named. Picking
/// the fit the sheets already give a part that sets none changes nothing.
pub(crate) fn project_set_variant_fit(app: &mut Signex, args: &CommandArgs) -> Ran {
    let every_channel = match every_channel(args) {
        Ok(every_channel) => every_channel,
        Err(ran) => return ran,
    };
    let named = match named_part(app, args) {
        Ok(named) => named,
        Err(ran) => return ran,
    };
    let fitted = flag(args, "fitted");
    if every_channel {
        return fit_every_channel(app, args, &named, fitted);
    }
    let part = named.part_ref();
    if let Some(picked) = fitted
        && fit_repeats(app, &named, picked)
    {
        let ran = variants_unchanged(app, named.project);
        return with_parts(app, &named, &[part], ran);
    }
    let command = ProjectSectionCommand::SetVariantPartFit {
        variant: named.variant.clone(),
        designator: part.designator.clone(),
        part: part.part.clone(),
        fitted,
    };
    let ran = variants_edit(app, args, command);
    with_parts(app, &named, &[part], ran)
}

/// Every channel's copy of the part `named` lists fitted or not, as the
/// Variant Manager's every-channel fit sets them: one step.
fn fit_every_channel(
    app: &mut Signex,
    args: &CommandArgs,
    named: &NamedPart,
    fitted: Option<bool>,
) -> Ran {
    let parts = match edited_parts(named, true) {
        Ok(parts) => parts,
        Err(ran) => return ran,
    };
    let command = ProjectSectionCommand::SetVariantPartsFit {
        variant: named.variant.clone(),
        parts: parts.clone(),
        fitted,
    };
    let ran = variants_edit(app, args, command);
    with_parts(app, named, &parts, ran)
}

/// Whether picking `picked` repeats the fit the part's sheets give it.
fn fit_repeats(app: &Signex, named: &NamedPart, picked: bool) -> bool {
    let Some(project) = app.document_state.project_by_id(named.project) else {
        return false;
    };
    let row = named.row();
    fit_repeats_sheets(
        &project.data.variant_settings,
        &named.matrix,
        Some(named.variant.as_str()),
        PlacedPart::new(&row.designator, row.link.as_ref()),
        picked,
    )
}

/// The parameters an alternate part sets: its part number, value and
/// footprint, each `None` (the part's own again) when not given. The one
/// shape `project_set_variant_alternate` and the Alternate part picker
/// (mockup row 208) both write.
pub(crate) fn alternate_parameters(
    mpn: Option<String>,
    value: Option<String>,
    footprint: Option<String>,
) -> Vec<(String, Option<String>)> {
    vec![
        (MPN_PARAMETER.to_string(), mpn),
        (VALUE_PARAMETER.to_string(), value),
        (FOOTPRINT_PARAMETER.to_string(), footprint),
    ]
}

/// The section command that gives `parts` the values `parameters` in
/// `variant`, as one step: the one edit `project_set_variant_alternate`,
/// `project_set_variant_override` and the canvas's Alternate part picker
/// and Edit value popover (mockup rows 208 and 209) all apply. `None` for
/// no part.
pub(crate) fn variant_parameters_command(
    variant: &str,
    parts: &[PartRef],
    parameters: Vec<(String, Option<String>)>,
) -> Option<ProjectSectionCommand> {
    match parts {
        [] => None,
        [part] => Some(match <[_; 1]>::try_from(parameters) {
            Ok([(parameter, value)]) => ProjectSectionCommand::SetVariantPartParameter {
                variant: variant.to_string(),
                designator: part.designator.clone(),
                part: part.part.clone(),
                parameter,
                value,
            },
            Err(parameters) => ProjectSectionCommand::SetVariantPartParameters {
                variant: variant.to_string(),
                designator: part.designator.clone(),
                part: part.part.clone(),
                parameters,
            },
        }),
        _ => Some(ProjectSectionCommand::SetVariantPartsParameters {
            variant: variant.to_string(),
            parts: parts.to_vec(),
            parameters,
        }),
    }
}

/// Apply the parameters `parameters` to the parts `args` names (`scope`
/// as in [`project_set_variant_fit`]) and answer with their settings.
fn set_parameters(
    app: &mut Signex,
    args: &CommandArgs,
    parameters: Vec<(String, Option<String>)>,
) -> Ran {
    let every_channel = match every_channel(args) {
        Ok(every_channel) => every_channel,
        Err(ran) => return ran,
    };
    let named = match named_part(app, args) {
        Ok(named) => named,
        Err(ran) => return ran,
    };
    let parts = match edited_parts(&named, every_channel) {
        Ok(parts) => parts,
        Err(ran) => return ran,
    };
    let Some(command) = variant_parameters_command(&named.variant, &parts, parameters) else {
        let reason = Translatable::new(ids::VARIANT_COMMAND_NO_PART);
        return declined(reason.arg("designator", named.row().designator.clone()));
    };
    let ran = variants_edit(app, args, command);
    with_parts(app, &named, &parts, ran)
}

/// `project_set_variant_alternate {document, variant, designator, mpn?,
/// value?, footprint?, scope?}`: the part replaced in `variant` by the part
/// numbered `mpn`, with its value and footprint, as one step; each one left
/// out is the part's own again. With none, the part is its own again.
/// `scope` as in [`project_set_variant_fit`].
pub(crate) fn project_set_variant_alternate(app: &mut Signex, args: &CommandArgs) -> Ran {
    let given = |name: &str| {
        text(args, name)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let (mpn, value, footprint) = (given("mpn"), given("value"), given("footprint"));
    if mpn.is_none() && (value.is_some() || footprint.is_some()) {
        return declined(Translatable::new(ids::VARIANT_COMMAND_ALTERNATE_NEEDS_MPN));
    }
    set_parameters(app, args, alternate_parameters(mpn, value, footprint))
}

/// `project_set_variant_override {document, variant, designator, field,
/// value?, scope?}`: the part's `field` (`Value`, `Footprint` or a field
/// name) in `variant`; an empty or missing `value` is the part's own value
/// again, as an emptied override in the Variant Manager. `scope` as in
/// [`project_set_variant_fit`].
pub(crate) fn project_set_variant_override(app: &mut Signex, args: &CommandArgs) -> Ran {
    let field = text(args, "field").unwrap_or_default().to_string();
    let value = text(args, "value")
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    set_parameters(app, args, vec![(field, value)])
}

/// `project_duplicate_variant {document, name, value?}`: a copy of the
/// variant `name` (`Base` copies Base) added as `value`, by default the
/// first free "<name> copy". The copy keeps what `name` changes on its
/// parts and the fits its sheets give them, as Duplicate does.
pub(crate) fn project_duplicate_variant(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let from = text(args, "name").unwrap_or_default().trim();
    let Some(project) = app.document_state.project_by_id(id) else {
        return refused("the project is no longer open");
    };
    // The definition's own spelling, as the Variant Manager names the copy.
    let from = project
        .data
        .variant_definitions
        .iter()
        .find(|defined| defined.eq_ignore_ascii_case(from))
        .map_or(from, String::as_str);
    let (source, sheet_fits) = if is_base_variant(from) {
        (BASE_VARIANT.to_string(), Vec::new())
    } else {
        match app.read_variant_parts_now(project) {
            Ok(matrix) => (from.to_string(), sheet_fits_in(&matrix, Some(from))),
            Err(why) => return refused(read_failed(&I18n::en_us(), &why).to_string()),
        }
    };
    let name = match text(args, "value") {
        Some(name) => name.to_string(),
        None => match copy_name(&project.data, &source, &app.ui_state.i18n) {
            Some(name) => name,
            None => return declined(Translatable::new(ids::VARIANT_COMMAND_NO_COPY_NAME)),
        },
    };
    let command = ProjectSectionCommand::DuplicateVariant {
        from: source,
        name: name.clone(),
        sheet_fits,
    };
    match variants_edit(app, args, command) {
        (CommandOutcome::Answered(mut answer), task) => {
            if let Value::Object(map) = &mut answer {
                map.insert("name".into(), json!(name));
            }
            (CommandOutcome::Answered(answer), task)
        }
        other => other,
    }
}
