//! The operand handlers of the 17 placement commands (#2155): a CLI or an
//! agent names the point(s) and text, and the edit goes through the same
//! `place_*_at` commit a canvas click ends in, so undo, the dirty flag and the
//! Messages log behave as for a click. Nothing click-only follows: no tool
//! arming or reset, no repeat-placement memory, no wire continuation, no
//! shape anchor.
//!
//! The points are explicit input in mm and are not snapped: one a click would
//! snap elsewhere is refused, so nothing lands off the grid silently. A point
//! may instead be a pin reference, `R1.2`, which is the pin's tip on the sheet
//! the edit targets, on or off the grid (#2199). What the operands do not name
//! comes from the pre-placement form and the project defaults, as for a click;
//! that includes the wire draw mode, which bends a run as it does a click
//! run; a wire or bus that is still diagonal after the bend is refused. Wires
//! and buses are checked before they land and answer with a report
//! (`wire_runs`); they still commit through the click's `apply_engine_commands`.

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
            Ok(outcome) => outcome,
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

    /// The point operand `name`: a pin reference is that pin's tip, wherever
    /// it is; a coordinate is refused unless `snap` (what the click of the
    /// same tool applies) leaves it where it is.
    fn point_where_a_click_lands(
        &self,
        args: &CommandArgs,
        name: &str,
        snap: fn(&Self, f64, f64) -> (f64, f64),
    ) -> Result<Point, String> {
        let (x, y) = match args.operand(name) {
            Some(OperandValue::Point(x, y)) => (x, y),
            Some(OperandValue::PinRef { designator, pin }) => {
                return self
                    .pin_ref_tip(designator, pin)
                    .map_err(|why| format!("`{name}`: {why}"));
            }
            _ => return Err(format!("operand `{name}` is missing")),
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

    /// A power port on a pin reference whose tip has no wire end, junction or
    /// label would only touch the pin: pin to pin is no connection (ADR-0002
    /// D5.1), so the port would sit on nothing. Coordinates are unchanged.
    fn refuse_unconnected_pin(
        &self,
        args: &CommandArgs,
        at: Point,
        what: &str,
    ) -> Result<(), String> {
        let Some(OperandValue::PinRef { designator, pin }) = args.operand("at") else {
            return Ok(());
        };
        let connected = self
            .document_state
            .edit_engine()
            .is_none_or(|e| signex_net::connected_points(e.document()).contains(&at));
        if connected {
            return Ok(());
        }
        Err(format!(
            "a {what} on {designator}.{pin} would touch the pin without connecting; draw a wire to it first or place a label"
        ))
    }

    /// A label or power port named `new_name` at `at` is refused when the net
    /// already there carries another name, unless `allow_merge=true`: it would
    /// short the two (#2199; rules in `docs/CLI.md`, "Labels and power ports").
    fn refuse_rename(
        &self,
        args: &CommandArgs,
        at: Point,
        new_name: &str,
        what: &str,
    ) -> Result<(), String> {
        if matches!(args.operand("allow_merge"), Some(OperandValue::Bool(true))) {
            return Ok(());
        }
        let Some(sheet) = self.document_state.edit_engine().map(|e| e.document()) else {
            return Ok(());
        };
        match signex_net::net_name_at(sheet, at) {
            Some(existing) if existing != new_name => Err(format!(
                "`at` {},{} is on net {existing}; a {new_name} {what} there would short {existing} and {new_name}; pass allow_merge=true to merge on purpose",
                at.x, at.y
            )),
            _ => Ok(()),
        }
    }

    fn place_from_operands(
        &mut self,
        placement: Placement,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        if let Placement::Wire | Placement::Bus | Placement::Line | Placement::Rectangle = placement
        {
            return self.place_segment_from_operands(placement, args);
        }
        self.place_at_from_operands(placement, args)
            .map(|()| CommandOutcome::Ran)
    }

    /// The placements at one point.
    fn place_at_from_operands(
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
                self.refuse_rename(args, at, &spec.text, "label")?;
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
                let net_name = text(args, "net_name")?;
                self.refuse_unconnected_pin(args, at, "power port")?;
                self.refuse_rename(args, at, &net_name, "power port")?;
                placed(self.place_power_at(at, &net_name, &lib_id))
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
                Err("a segment placement is not a point placement".to_string())
            }
        }
    }

    fn place_segment_from_operands(
        &mut self,
        placement: Placement,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        // A bus connects to buses, bus entries and labels, never to a pin.
        if matches!(placement, Placement::Bus)
            && ["start", "end"]
                .iter()
                .any(|name| matches!(args.operand(name), Some(OperandValue::PinRef { .. })))
        {
            return Err(
                "a bus does not connect to pins; give `start` and `end` as coordinates".to_string(),
            );
        }
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
        if matches!(placement, Placement::Wire | Placement::Bus) {
            return self.run_segment_operands(
                args,
                start,
                end,
                matches!(placement, Placement::Bus),
            );
        }
        let (width, fill) = pre_placement_shape(&self.document_state);
        match placement {
            Placement::Line => placed(self.place_line_at(start, end, width)),
            _ => placed(self.place_rect_at(start, end, width, fill)),
        }
        .map(|()| CommandOutcome::Ran)
    }
}
