//! The Gerber settings commands of an Export Job (#1299, mockup row 114):
//! `set_job_gerber_units`, `set_job_gerber_format`, `set_job_gerber_zeros`,
//! `set_job_gerber_layers` and `set_job_gerber_apertures`, each naming the
//! job by `path` and the Gerber output by `name`, as the tab's Gerber
//! fields set them. A job open in a tab is edited as its draft, any other
//! job file is read and written back, as every job command does. Rules:
//! docs/OUTPUT_JOBS.md, "Gerber settings".

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_types::output_job::{
    GerberApertures, GerberFormat, GerberUnits, GerberZeros, JobOutput, OutputKind,
    gerber_layers_text, parse_gerber_layers,
};

use super::operands::{job_refusal, text_operand};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// One Gerber setting as a command reads it from its operand.
type SetGerber = fn(&mut JobOutput, &str) -> Result<(), String>;

/// The Gerber settings of `output`, as every Gerber command answers them.
pub(super) fn gerber_answer(output: &JobOutput) -> Value {
    let settings = output.gerber_settings();
    json!({
        "output": output.name,
        "units": settings.units.id(),
        "format": settings.format.to_string(),
        "zeros": settings.zeros.id(),
        "layers": gerber_layers_text(settings.layers.as_deref()),
        "apertures": settings.apertures.id(),
    })
}

/// The units `text` names on `output`.
pub(super) fn set_units(output: &mut JobOutput, text: &str) -> Result<(), String> {
    let units = GerberUnits::from_id(text)
        .ok_or_else(|| format!("`units` is mm or inch, not `{}`", text.trim()))?;
    output.gerber_units = Some(units);
    Ok(())
}

/// The coordinate format `text` names on `output`.
pub(super) fn set_format(output: &mut JobOutput, text: &str) -> Result<(), String> {
    let format = GerberFormat::parse(text).ok_or_else(|| {
        format!(
            "`format` is integer:decimal digits such as 4:6 (1 to 6, then 4 to 6), not `{}`",
            text.trim()
        )
    })?;
    output.gerber_format = Some(format);
    Ok(())
}

/// The zeros `text` names on `output`.
pub(super) fn set_zeros(output: &mut JobOutput, text: &str) -> Result<(), String> {
    let zeros = GerberZeros::from_id(text).ok_or_else(|| {
        format!(
            "`zeros` is leading, trailing or none, not `{}`",
            text.trim()
        )
    })?;
    output.gerber_zeros = Some(zeros);
    Ok(())
}

/// The layer list `text` names on `output`; `all` writes every layer.
pub(super) fn set_layers(output: &mut JobOutput, text: &str) -> Result<(), String> {
    let layers = parse_gerber_layers(text).map_err(|bad| {
        format!("`{bad}` is not a Gerber layer id; see docs/OUTPUT_JOBS.md, \"Gerber settings\"")
    })?;
    output.gerber_layers = layers;
    Ok(())
}

/// The aperture form `text` names on `output`.
pub(super) fn set_apertures(output: &mut JobOutput, text: &str) -> Result<(), String> {
    let apertures = GerberApertures::from_id(text)
        .ok_or_else(|| format!("`apertures` is standard or macros, not `{}`", text.trim()))?;
    output.gerber_apertures = Some(apertures);
    Ok(())
}

/// Set the Gerber setting `operand` of the output `name` with `set`.
fn set_gerber(
    app: &mut Signex,
    args: &CommandArgs,
    command: &'static str,
    operand: &'static str,
    set: SetGerber,
) -> Ran {
    let Some(name) = text_operand(args, "name").map(|name| name.trim().to_string()) else {
        let why = "needs the operand `name`: the Gerber output";
        return (job_refusal(Path::new(""), why), Task::none());
    };
    let Some(value) = text_operand(args, operand).map(str::to_string) else {
        let why = format!("needs the operand `{operand}`");
        return (job_refusal(Path::new(""), why), Task::none());
    };
    app.edit_job_by_command(
        command,
        args,
        Box::new(move |job| {
            let output = job
                .output_mut(&name)
                .ok_or_else(|| format!("the job has no output named {name:?}"))?;
            if output.known_kind() != Some(OutputKind::Gerber) {
                return Err(format!("output {name:?} is not a Gerber output"));
            }
            set(output, &value)?;
            Ok(gerber_answer(output))
        }),
    )
}

/// `set_job_gerber_units path= name= units=mm|inch`.
pub(crate) fn set_job_gerber_units(app: &mut Signex, args: &CommandArgs) -> Ran {
    set_gerber(app, args, "set_job_gerber_units", "units", set_units)
}

/// `set_job_gerber_format path= name= format=<integer>:<decimal>`.
pub(crate) fn set_job_gerber_format(app: &mut Signex, args: &CommandArgs) -> Ran {
    set_gerber(app, args, "set_job_gerber_format", "format", set_format)
}

/// `set_job_gerber_zeros path= name= zeros=leading|trailing|none`.
pub(crate) fn set_job_gerber_zeros(app: &mut Signex, args: &CommandArgs) -> Ran {
    set_gerber(app, args, "set_job_gerber_zeros", "zeros", set_zeros)
}

/// `set_job_gerber_layers path= name= layers=all|<id>,<id>,...`.
pub(crate) fn set_job_gerber_layers(app: &mut Signex, args: &CommandArgs) -> Ran {
    set_gerber(app, args, "set_job_gerber_layers", "layers", set_layers)
}

/// `set_job_gerber_apertures path= name= apertures=standard|macros`.
pub(crate) fn set_job_gerber_apertures(app: &mut Signex, args: &CommandArgs) -> Ran {
    set_gerber(
        app,
        args,
        "set_job_gerber_apertures",
        "apertures",
        set_apertures,
    )
}
