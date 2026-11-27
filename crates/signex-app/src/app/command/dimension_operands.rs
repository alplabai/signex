//! `place_linear_dimension` and `place_aligned_dimension` from operands
//! (#2836, mockup row 157). `start`, `end` and `at` stand for the tool's
//! three clicks: the two measured points, then a point the dimension line
//! runs through. `direction`, `units`, `precision`, `arrows` and `text` stand
//! for the tool's Properties form; what is left out takes the form's
//! defaults, never the person's form. The dimension commits through the same
//! `place_sheet_dimension` the third click ends in, so it is one undo step.
//! Points are refused off the grid, as for any placement. See docs/CLI.md,
//! "Dimensions".

use iced::Task;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use signex_types::schematic::{
    DIMENSION_MAX_PRECISION, DimensionArrows, DimensionDirection, DimensionUnits, SchDimension,
};

use super::placement_operands::run_edit;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::canvas::{DimensionToolStyle, dimension_from_clicks};
use crate::app::{Message, Signex, Tool};

type Ran = (CommandOutcome, Task<Message>);

/// `place_linear_dimension start=x,y end=x,y at=x,y [direction=..]
/// [units=..] [precision=..] [arrows=..] [text=..] [document=..]`: horizontal
/// or vertical by the axis the points are further apart along, unless
/// `direction` says otherwise.
pub(super) fn linear(app: &mut Signex, args: &CommandArgs) -> Ran {
    place(app, args, Tool::LinearDimension)
}

/// `place_aligned_dimension`, as [`linear`]: the straight distance, unless
/// `direction` says otherwise.
pub(super) fn aligned(app: &mut Signex, args: &CommandArgs) -> Ran {
    place(app, args, Tool::AlignedDimension)
}

fn place(app: &mut Signex, args: &CommandArgs, tool: Tool) -> Ran {
    run_edit(app, args, |app| {
        let style = style(args)?;
        let from = app.grid_point(args, "start")?;
        let to = app.grid_point(args, "end")?;
        let at = app.grid_point(args, "at")?;
        let dimension = dimension_from_clicks(tool, from, to, at, style);
        // The click places nothing here; an operand caller is told why.
        if dimension.measured_mm() <= 0.0 {
            return Err(format!(
                "`start` and `end` measure nothing along a {} dimension",
                word_of(&dimension.direction)
            ));
        }
        let reply = answer(&dimension);
        if !app.place_sheet_dimension(dimension) {
            return Err("nothing was placed".to_string());
        }
        Ok(CommandOutcome::Answered(reply))
    })
}

/// The form values the operands give; the form's defaults for the rest.
fn style(args: &CommandArgs) -> Result<DimensionToolStyle, String> {
    let defaults = DimensionToolStyle::default();
    Ok(DimensionToolStyle {
        units: choice(args, "units", &DimensionUnits::ALL)?.unwrap_or(defaults.units),
        precision: precision(args)?.unwrap_or(defaults.precision),
        arrows: choice(args, "arrows", &DimensionArrows::ALL)?.unwrap_or(defaults.arrows),
        direction: choice(args, "direction", &DimensionDirection::ALL)?,
        override_text: match args.operand("text") {
            Some(OperandValue::Text(text)) => Some(text.clone()),
            _ => None,
        },
        flipped: false,
    })
}

/// The text operand `name` as one of `all`, by the word the sheet file
/// stores it under (`horizontal`, `mil`, `filled_inside`); `None` when
/// absent or blank.
fn choice<T: Serialize + DeserializeOwned>(
    args: &CommandArgs,
    name: &str,
    all: &[T],
) -> Result<Option<T>, String> {
    let Some(OperandValue::Text(text)) = args.operand(name) else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    serde_json::from_value(Value::String(text.to_ascii_lowercase()))
        .map(Some)
        .map_err(|_| {
            let words: Vec<String> = all.iter().map(word_of::<T>).collect();
            format!("`{name}`: `{text}` is not one of {}", words.join(", "))
        })
}

/// The word the sheet file stores `value` under.
fn word_of<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(word)) => word,
        _ => String::new(),
    }
}

/// The `precision` operand, the decimals written; `None` when absent or blank.
fn precision(args: &CommandArgs) -> Result<Option<u8>, String> {
    let Some(OperandValue::Text(text)) = args.operand("precision") else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<u8>()
        .ok()
        .filter(|decimals| *decimals <= DIMENSION_MAX_PRECISION)
        .map(Some)
        .ok_or_else(|| {
            format!(
                "`precision`: `{text}` is not a number of decimals from 0 to {DIMENSION_MAX_PRECISION}"
            )
        })
}

/// What a dimension command answers: the dimension as placed, its text and
/// its measured length.
fn answer(dimension: &SchDimension) -> Value {
    json!({
        "id": dimension.uuid.to_string(),
        "kind": "dimension",
        "label": dimension.label(),
        "measured_mm": dimension.measured_mm(),
        "dimension": serde_json::to_value(dimension).unwrap_or(Value::Null),
    })
}
