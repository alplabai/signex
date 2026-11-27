//! The catalog commands that set what the Gerber Viewer shows, so every
//! state of mockup row 193 is reachable by id: a pad or a plane selected
//! (`select_gerber_object`), the board seen from the bottom
//! (`flip_gerber_view`), the holes on top (`show_gerber_drills_on_top`), a
//! net highlighted (`highlight_gerber_net`) and the Open menu
//! (`show_gerber_open_menu`). These are the operand handlers; the palette's
//! form without operands is the toolbar's own message (`bridge.rs`). Each
//! answers with `gerber_viewer_answer`. Rules: `docs/GERBER_VIEWER.md`,
//! "Setting the view by id".

use iced::Task;
use pcb_geom::PointNm;
use signex_i18n::{Translatable, ids};

use super::state::OpenPackage;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// How far from an object a point given by id may land, in nm: a point
/// inside a pad or a region selects it; a marker (an object with no shape)
/// needs its own point, to 10 µm.
const COMMAND_TOLERANCE_NM: i64 = 10_000;

/// Nanometres in a millimetre.
const NM_PER_MM: f64 = 1.0e6;

/// `select_gerber_object [at=x,y] [layer=<file>]`. `at` selects the
/// topmost visible object there, as a click does; with `layer` too, the
/// topmost object of that layer there, even under another layer. `layer`
/// alone picks the layer row, so Properties shows that layer. Neither
/// clears the selection. A point that hits nothing selects nothing.
pub(crate) fn select_gerber_object(app: &mut Signex, args: &CommandArgs) -> Ran {
    let at = match args.operand("at") {
        None => None,
        Some(OperandValue::Point(x, y)) => Some(point_nm(*x, *y)),
        Some(_) => return refused(Translatable::new(ids::GERBER_VIEWER_COMMAND_AT_NOT_A_POINT)),
    };
    let file = text_operand(args, "layer");
    let Some(open) = app.document_state.gerber_viewer.open.as_mut() else {
        return no_package();
    };
    let layer = match file {
        None => None,
        Some(file) => match visible_layer(open, file) {
            Ok(layer) => Some(layer),
            Err(reason) => return refused(reason),
        },
    };
    match (at, layer) {
        (Some(at), Some(layer)) => {
            let hit = open.object_on_layer_at(layer, at, COMMAND_TOLERANCE_NM);
            open.select(hit);
        }
        (Some(at), None) => open.select(open.object_at(at, COMMAND_TOLERANCE_NM)),
        (None, Some(layer)) => open.pick_layer(layer),
        (None, None) => open.select(None),
    }
    open.revision += 1;
    answered(app)
}

/// `flip_gerber_view [on=..]`: the board seen from the bottom, or the top;
/// without `on` the view turns over, as the toolbar's Flip does.
pub(crate) fn flip_gerber_view(app: &mut Signex, args: &CommandArgs) -> Ran {
    let on = bool_operand(args, "on");
    on_package(app, |open| open.flipped = on.unwrap_or(!open.flipped))
}

/// `show_gerber_drills_on_top [on=..]`: the holes drawn over everything,
/// or in their list place; without `on` the toolbar's Drills is pressed.
pub(crate) fn show_gerber_drills_on_top(app: &mut Signex, args: &CommandArgs) -> Ran {
    let on = bool_operand(args, "on");
    on_package(app, |open| {
        open.drills_on_top = on.unwrap_or(!open.drills_on_top);
    })
}

/// `highlight_gerber_net [net=..] [on=..]`: `net` keeps that X2 net at full
/// strength; `on=false` lets the highlight go; otherwise the selected
/// object's net is taken, and without `on` a highlight already on is let
/// go, as the toolbar's Highlight net does. Refused with the toolbar's own
/// reason when there is no net to take.
pub(crate) fn highlight_gerber_net(app: &mut Signex, args: &CommandArgs) -> Ran {
    let on = bool_operand(args, "on");
    let named = text_operand(args, "net");
    let Some(open) = app.document_state.gerber_viewer.open.as_mut() else {
        return no_package();
    };
    let net = match (on, named) {
        (Some(false), _) => None,
        (_, Some(net)) if open.has_net(net) => Some(net.to_string()),
        (_, Some(net)) => {
            let reason = Translatable::new(ids::GERBER_VIEWER_COMMAND_NO_SUCH_NET).arg("net", net);
            return refused(reason);
        }
        (None, None) if open.net.is_some() => None,
        (_, None) => match selected_net(open) {
            Ok(net) => Some(net),
            Err(reason) => return refused(reason),
        },
    };
    open.net = net;
    open.revision += 1;
    answered(app)
}

/// `show_gerber_open_menu [on=..]`: the tab's Open menu shows or closes;
/// without `on` it toggles, as the Open button does. Showing it checks the
/// recent packages it lists. It needs no package.
pub(crate) fn show_gerber_open_menu(app: &mut Signex, args: &CommandArgs) -> Ran {
    let viewer = &mut app.document_state.gerber_viewer;
    let on = bool_operand(args, "on").unwrap_or(!viewer.open_menu);
    viewer.open_menu = on;
    let task = if on {
        app.check_recent_gerber_packages()
    } else {
        Task::none()
    };
    (CommandOutcome::Answered(app.gerber_viewer_answer()), task)
}

/// A change to the open package, answered; refused with nothing open.
fn on_package(app: &mut Signex, change: impl FnOnce(&mut OpenPackage)) -> Ran {
    let Some(open) = app.document_state.gerber_viewer.open.as_mut() else {
        return no_package();
    };
    change(open);
    open.revision += 1;
    answered(app)
}

/// The layer read from `file`, if it is shown.
fn visible_layer(open: &OpenPackage, file: &str) -> Result<usize, Translatable> {
    let Some(layer) = open.layer_of_file(file) else {
        return Err(Translatable::new(ids::GERBER_VIEWER_COMMAND_NO_LAYER).arg("layer", file));
    };
    if open.looks.get(layer).is_some_and(|look| look.visible) {
        Ok(layer)
    } else {
        Err(Translatable::new(ids::GERBER_VIEWER_COMMAND_LAYER_HIDDEN).arg("layer", file))
    }
}

/// The selected object's net, or why Highlight net has none to take.
fn selected_net(open: &OpenPackage) -> Result<String, Translatable> {
    if !open.package.has_net_attributes() {
        return Err(Translatable::new(ids::GERBER_VIEWER_HIGHLIGHT_NET_NO_X2));
    }
    open.selected_net()
        .map(str::to_string)
        .ok_or_else(|| Translatable::new(ids::GERBER_VIEWER_HIGHLIGHT_NET_NO_NET))
}

fn bool_operand(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(on)) => Some(*on),
        _ => None,
    }
}

fn text_operand<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// `x`, `y` in mm as a point of the files' frame in nm.
fn point_nm(x: f64, y: f64) -> PointNm {
    let nm = |mm: f64| (mm * NM_PER_MM).round() as i64;
    PointNm::new(nm(x), nm(y))
}

fn answered(app: &Signex) -> Ran {
    (
        CommandOutcome::Answered(app.gerber_viewer_answer()),
        Task::none(),
    )
}

fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn no_package() -> Ran {
    refused(Translatable::new(ids::GERBER_VIEWER_COMMAND_NO_PACKAGE))
}
