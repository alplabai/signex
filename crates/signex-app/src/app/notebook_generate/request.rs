//! A Generate request from the command's operands: the project the
//! `document` operand is (or belongs to), and the options the others name.
//! Every operand is checked before anything runs; a bad one refuses the call
//! with the reason.

use signex_notebook::generate::{
    ChecksSource, GenerateOptions, Template, parse_fail_on, parse_sections, variant_of,
};

use super::GenerateRequest;
use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::document_ids::DocumentKind;

const ID: &str = "generate_design_notebook";

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn flag(args: &CommandArgs, name: &str) -> bool {
    matches!(args.operand(name), Some(OperandValue::Bool(true)))
}

/// The options the operands name; every one left out keeps its default.
pub(super) fn options_from_args(args: &CommandArgs) -> Result<GenerateOptions, String> {
    let mut options = GenerateOptions::default();
    if let Some(list) = text(args, "sections") {
        options.sections = parse_sections(list).map_err(|why| format!("`{ID}`: {why}"))?;
    }
    if let Some(token) = text(args, "template") {
        options.template = Template::parse(token.trim()).ok_or_else(|| {
            format!(
                "`{ID}`: `{token}` is not a template; the template is {}",
                Template::DesignReportA4.token()
            )
        })?;
    }
    if let Some(token) = text(args, "checks") {
        options.checks = ChecksSource::parse(token.trim())
            .ok_or_else(|| format!("`{ID}`: `checks` is `run` or `last`, not `{token}`"))?;
    }
    if let Some(list) = text(args, "fail_on") {
        options.fail_on = parse_fail_on(list).map_err(|why| format!("`{ID}`: {why}"))?;
    }
    let typed = |name| text(args, name).map(str::trim).filter(|t| !t.is_empty());
    options.document = typed("document_number").map(str::to_string);
    options.revision = typed("revision").map(str::to_string);
    Ok(options)
}

/// The request `args` makes, or why it is refused: no `document`, one that
/// is in no open project, a bad option or a variant the project lacks.
pub(crate) fn request_from_args(
    app: &Signex,
    args: &CommandArgs,
) -> Result<GenerateRequest, String> {
    let Some(OperandValue::Document(named)) = args.operand("document") else {
        return Err(format!("`{ID}` needs the operand `document`"));
    };
    let entry = app.resolve_document_target(named)?;
    let path = match entry.kind {
        DocumentKind::Project => entry.path.clone(),
        _ => entry.project.as_ref().map(|project| project.path.clone()),
    };
    let project = path.and_then(|path| {
        app.document_state
            .projects
            .iter()
            .find(|project| project.path == path)
    });
    let Some(project) = project else {
        return Err(format!(
            "`{ID}` reports on a project, and {} is not part of an open one",
            entry.name
        ));
    };
    let options = options_from_args(args)?;
    let variant =
        variant_of(&project.data, text(args, "variant")).map_err(|why| format!("`{ID}`: {why}"))?;
    Ok(GenerateRequest {
        project: project.path.clone(),
        options,
        variant: Some(variant),
        save: flag(args, "save"),
        update: flag(args, "update"),
    })
}
