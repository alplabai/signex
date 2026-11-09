//! The operand handlers of the 17 placement commands (#2155): a CLI or an
//! agent names the point(s) and text, and the edit goes through the same
//! `place_*_at` commit a canvas click ends in, so undo, the dirty flag and the
//! Messages log behave as for a click. Nothing click-only follows: no tool
//! arming or reset, no repeat-placement memory, no wire continuation, no
//! shape anchor.
//!
//! The points are explicit input in mm and are not snapped: one a click would
//! snap elsewhere is refused, so nothing lands off the grid silently. What the
//! operands do not name comes from the pre-placement form and the project
//! defaults, as for a click; that includes the wire draw mode, so a diagonal
//! wire or bus bends as a click-drawn one does.

use iced::Task;
use signex_types::schematic::{LabelType, Point};

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::canvas::{GLOBAL_PORT_SHAPE, pre_placement_shape};
use crate::app::{Message, Signex};

/// What a placement id commits.
#[derive(Clone, Copy)]
enum Placement {
    /// A label of this kind and shape: what the id's armed tool places.
    Label(LabelType, &'static str),
    Power,
    NoConnect,
    Junction,
    NoErc,
    Wire,
    Bus,
    Text,
    Line,
    Rectangle,
}

type Ran = (CommandOutcome, Task<Message>);

fn run(app: &mut Signex, args: &CommandArgs, placement: Placement) -> Ran {
    let outcome = if app.document_state.edit_engine().is_none() {
        CommandOutcome::disabled("no schematic is open")
    } else if let Some(reason) = app.refuse_if_active_read_only() {
        // The gateway would refuse too; say why rather than "nothing placed".
        CommandOutcome::disabled(reason)
    } else {
        match app.place_from_operands(placement, args) {
            Ok(()) => CommandOutcome::Ran,
            Err(reason) => CommandOutcome::disabled(reason),
        }
    };
    (outcome, Task::none())
}

/// `place_net_label` / `place_local_net_label at=x,y text=.. [rotation=..]`.
pub(super) fn net_label(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Label(LabelType::Net, ""))
}

/// `place_global_label at=x,y text=.. [rotation=..]`: a bidirectional port.
pub(super) fn global_label(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(
        app,
        args,
        Placement::Label(LabelType::Global, GLOBAL_PORT_SHAPE),
    )
}

/// `place_hierarchical_label at=x,y text=.. [rotation=..]`.
pub(super) fn hierarchical_label(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Label(LabelType::Hierarchical, ""))
}

/// `place_power_symbol at=x,y net_name=.. lib_id=..`.
pub(super) fn power(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Power)
}

/// `place_no_connect at=x,y`.
pub(super) fn no_connect(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::NoConnect)
}

/// `place_junction at=x,y`.
pub(super) fn junction(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Junction)
}

/// `place_no_erc at=x,y`.
pub(super) fn no_erc(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::NoErc)
}

/// `place_wire` / `draw_wire start=x,y end=x,y`: one click-to-click run, bent
/// by the current draw mode as a click run is.
pub(super) fn wire(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Wire)
}

/// `place_bus` / `draw_bus start=x,y end=x,y`: as [`wire`], for a bus.
pub(super) fn bus(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Bus)
}

/// `place_text` / `place_text_string at=x,y text=..`.
pub(super) fn text_note(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Text)
}

/// `draw_line` / `draw_graphic_line start=x,y end=x,y`.
pub(super) fn line(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Line)
}

/// `draw_rectangle start=x,y end=x,y`: the two opposite corners.
pub(super) fn rectangle(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, Placement::Rectangle)
}

fn text(args: &CommandArgs, name: &str) -> Result<String, String> {
    match args.operand(name) {
        Some(OperandValue::Text(value)) => Ok(value.clone()),
        _ => Err(format!("operand `{name}` is missing")),
    }
}

fn placed(was_placed: bool) -> Result<(), String> {
    if was_placed {
        Ok(())
    } else {
        Err("nothing was placed".to_string())
    }
}

impl Signex {
    /// The point operand `name`, refused when a click would snap it
    /// elsewhere (snapping on, and off the grid).
    fn grid_point(&self, args: &CommandArgs, name: &str) -> Result<Point, String> {
        self.point_where_a_click_lands(args, name, Self::snap_to_grid)
    }

    /// The point operand `name`, refused unless `snap` (what the click of
    /// the same tool applies) leaves it where it is.
    fn point_where_a_click_lands(
        &self,
        args: &CommandArgs,
        name: &str,
        snap: fn(&Self, f64, f64) -> (f64, f64),
    ) -> Result<Point, String> {
        let Some(OperandValue::Point(x, y)) = args.operand(name) else {
            return Err(format!("operand `{name}` is missing"));
        };
        let (sx, sy) = snap(self, *x, *y);
        let nm = |mm: f64| (mm * 1e6).round() as i64;
        if (nm(sx), nm(sy)) != (nm(*x), nm(*y)) {
            // The snap may be the grid or, for a wire, a nearby pin end, wire
            // end or junction (the electrical grid, Shift+E).
            return Err(format!(
                "`{name}` is not where a click lands (the {} mm grid, and for a wire the electrical grid): a click there lands on {sx},{sy}; pass that point",
                self.ui_state.grid_size_mm
            ));
        }
        Ok(Point::new(*x, *y))
    }

    fn place_from_operands(
        &mut self,
        placement: Placement,
        args: &CommandArgs,
    ) -> Result<(), String> {
        match placement {
            Placement::Label(kind, shape) => {
                let at = self.grid_point(args, "at")?;
                let mut spec = self.label_spec_from_form();
                spec.label_type = kind;
                spec.shape = shape.to_string();
                spec.text = text(args, "text")?;
                if let Some(OperandValue::Angle(degrees)) = args.operand("rotation") {
                    spec.rotation = *degrees;
                }
                placed(self.place_label_at(at, spec))
            }
            Placement::Power => {
                let at = self.grid_point(args, "at")?;
                let lib_id = text(args, "lib_id")?;
                // A click only picks a `power:` style; any other id would turn
                // a real part's library entry into a power port.
                if !lib_id.starts_with("power:") {
                    return Err(format!(
                        "`lib_id` must name a power port style, `power:<name>` (e.g. power:GND, power:VCC), not {lib_id}"
                    ));
                }
                placed(self.place_power_at(at, &text(args, "net_name")?, &lib_id))
            }
            Placement::NoConnect => placed(self.place_no_connect_at(self.grid_point(args, "at")?)),
            Placement::Junction => placed(self.place_junction_at(self.grid_point(args, "at")?)),
            Placement::NoErc => {
                placed(self.place_no_erc_directive_at(self.grid_point(args, "at")?))
            }
            Placement::Text => {
                placed(self.place_text_at(self.grid_point(args, "at")?, text(args, "text")?))
            }
            Placement::Wire | Placement::Bus | Placement::Line | Placement::Rectangle => {
                self.place_segment_from_operands(placement, args)
            }
        }
    }

    fn place_segment_from_operands(
        &mut self,
        placement: Placement,
        args: &CommandArgs,
    ) -> Result<(), String> {
        // A wire click also pulls onto a nearby pin end, wire end or junction
        // (`snap_wire_point`); a bus, line or rectangle click only snaps to
        // the grid.
        let snap = match placement {
            Placement::Wire => Self::snap_wire_point,
            _ => Self::snap_to_grid,
        };
        let (start, end) = (
            self.point_where_a_click_lands(args, "start", snap)?,
            self.point_where_a_click_lands(args, "end", snap)?,
        );
        // A zero-length segment places nothing; say so rather than report
        // a run.
        if start == end {
            return Err("`start` and `end` are the same point".to_string());
        }
        let mode = self.interaction_state.draw_mode;
        let (width, fill) = pre_placement_shape(&self.document_state);
        match placement {
            Placement::Wire => placed(self.place_wire_at(start, end, mode).1),
            Placement::Bus => placed(self.place_bus_at(start, end, mode).1),
            Placement::Line => placed(self.place_line_at(start, end, width)),
            _ => placed(self.place_rect_at(start, end, width, fill)),
        }
    }
}
