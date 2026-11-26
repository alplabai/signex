//! The operand forms of the curve drawings (#1267):
//! `draw_ellipse center=x,y radius_x=.. radius_y=..`,
//! `draw_elliptical_arc center=x,y radius_x=.. radius_y=.. start_angle=..
//! end_angle=..` and `draw_round_rectangle start=x,y end=x,y
//! [corner_radius=..]`. Each commits through the click's `place_curve_at`,
//! one undo step, and answers with the drawing's id and kind. A point is
//! refused off the grid as for any placement; radii and angles are taken as
//! given. Width and fill come from the project defaults, never the
//! pre-placement form (#2326).

use iced::Task;
use serde_json::json;
use signex_types::schematic::CurveShape;

use super::placement_operands::run_edit;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::canvas::default_shape;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn length(args: &CommandArgs, name: &str) -> Result<f64, String> {
    match args.operand(name) {
        Some(OperandValue::Length(mm)) => Ok(*mm),
        _ => Err(format!("operand `{name}` is missing")),
    }
}

/// An angle operand kept in `[0, 360)`.
fn angle(args: &CommandArgs, name: &str) -> Result<f64, String> {
    match args.operand(name) {
        Some(OperandValue::Angle(deg)) => Ok(deg.rem_euclid(360.0) + 0.0),
        _ => Err(format!("operand `{name}` is missing")),
    }
}

/// `draw_ellipse center=x,y radius_x=.. radius_y=.. [document=..]`.
pub(super) fn ellipse(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let shape = CurveShape::Ellipse {
            center: app.grid_point(args, "center")?,
            radius_x: length(args, "radius_x")?,
            radius_y: length(args, "radius_y")?,
        };
        app.place_curve_from_operands(shape)
    })
}

/// `draw_elliptical_arc center=x,y radius_x=.. radius_y=.. start_angle=..
/// end_angle=.. [document=..]`: counter-clockwise from the start angle.
pub(super) fn elliptical_arc(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let shape = CurveShape::EllipticalArc {
            center: app.grid_point(args, "center")?,
            radius_x: length(args, "radius_x")?,
            radius_y: length(args, "radius_y")?,
            start_deg: angle(args, "start_angle")?,
            end_deg: angle(args, "end_angle")?,
        };
        app.place_curve_from_operands(shape)
    })
}

/// `draw_round_rectangle start=x,y end=x,y [corner_radius=..] [document=..]`.
pub(super) fn round_rectangle(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let start = app.grid_point(args, "start")?;
        let end = app.grid_point(args, "end")?;
        let mut shape = CurveShape::round_rect(start, end);
        if let Some(OperandValue::Length(asked)) = args.operand("corner_radius") {
            if let CurveShape::RoundRect { corner_radius, .. } = &mut shape {
                *corner_radius = *asked;
            }
            shape.clamp_corner_radius();
        }
        app.place_curve_from_operands(shape)
    })
}

impl Signex {
    /// Place `shape` with the project's shape width and fill, answering with
    /// its id and kind; refused when it has no visible size or no sweep.
    fn place_curve_from_operands(&mut self, shape: CurveShape) -> Result<CommandOutcome, String> {
        if shape.is_degenerate() {
            return Err(format!(
                "the {} has no visible size or sweeps no angle",
                shape.kind_name().replace('_', " ")
            ));
        }
        let (width, fill) = default_shape(&self.document_state);
        let id = self
            .place_curve_at(shape, width, fill)
            .ok_or_else(|| "nothing was placed".to_string())?;
        Ok(CommandOutcome::Answered(json!({
            "id": id.to_string(),
            "kind": shape.kind_name(),
        })))
    }
}
