//! The Layers panel's commands from their operands (#2363): what its eyes,
//! colour chips, Used On, Active Layer and View Options rows do, for an
//! agent or the CLI. Each names its board with `document` and changes that
//! board's view alone, as the panel's control does: never the board, no
//! undo step.

use iced::Task;
use signex_renderer::pcb::BoardObject;

use super::layer_strip_operands::{layer_operand, on_named_board, text_operand};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, PcbViewMsg, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `on`: required, `true` or `false`.
fn on_operand(args: &CommandArgs) -> Result<bool, String> {
    match args.operand("on") {
        Some(OperandValue::Bool(on)) => Ok(*on),
        _ => Err("the operand `on` is needed: true or false".to_string()),
    }
}

/// `kind`: a kind of object by its token.
fn object_operand(args: &CommandArgs) -> Result<BoardObject, String> {
    let token = text_operand(args, "kind");
    BoardObject::from_token(token)
        .ok_or_else(|| format!("`{token}` is not a kind of board object"))
}

/// `set_layer_visible layer=.. on=.. document=..`: a layer's eye.
pub(super) fn set_layer_visible(app: &mut Signex, args: &CommandArgs) -> Ran {
    let event = layer_operand(args).and_then(|layer| {
        on_operand(args).map(|visible| PcbViewMsg::LayerVisibilitySet { layer, visible })
    });
    on_named_board(app, args, event)
}

/// `set_active_layer layer=.. document=..`: a click on a layer's name, or
/// the Active Layer dropdown.
pub(super) fn set_active_layer(app: &mut Signex, args: &CommandArgs) -> Ran {
    let event = layer_operand(args).map(PcbViewMsg::ActiveLayerPicked);
    on_named_board(app, args, event)
}

/// `set_layer_color layer=.. [color=..] document=..`: a colour picked under
/// a layer's chip; without `color`, the palette colour again.
pub(super) fn set_layer_color(app: &mut Signex, args: &CommandArgs) -> Ran {
    let colour = match args.operand("color") {
        Some(OperandValue::Color(r, g, b)) => Some([*r, *g, *b]),
        _ => None,
    };
    let event = layer_operand(args).map(|layer| PcbViewMsg::LayerColourSet { layer, colour });
    on_named_board(app, args, event)
}

/// `show_used_layers_only on=.. document=..`: Used On.
pub(super) fn show_used_layers_only(app: &mut Signex, args: &CommandArgs) -> Ran {
    let event = on_operand(args).map(PcbViewMsg::UsedLayersOnlySet);
    on_named_board(app, args, event)
}

/// `set_object_visible kind=.. on=.. document=..`: a View Options eye;
/// `kind=all` is the All Objects row.
pub(super) fn set_object_visible(app: &mut Signex, args: &CommandArgs) -> Ran {
    let kind = if text_operand(args, "kind").eq_ignore_ascii_case("all") {
        Ok(None)
    } else {
        object_operand(args).and_then(|kind| {
            if kind.can_hide() {
                Ok(Some(kind))
            } else {
                Err(format!(
                    "the board view cannot hide {} yet",
                    kind.token()
                ))
            }
        })
    };
    let event = kind.and_then(|kind| {
        on_operand(args).map(|visible| PcbViewMsg::ObjectVisibilitySet { kind, visible })
    });
    on_named_board(app, args, event)
}

/// `set_object_transparency kind=.. value=.. document=..`: a View Options
/// transparency slider, in percent.
pub(super) fn set_object_transparency(app: &mut Signex, args: &CommandArgs) -> Ran {
    let value = text_operand(args, "value");
    let percent = value
        .trim_end_matches('%')
        .trim()
        .parse::<u8>()
        .ok()
        .filter(|p| *p <= 100)
        .ok_or_else(|| format!("`{value}` is not a transparency: a whole percent, 0 to 100"));
    let kind = object_operand(args).and_then(|kind| {
        if kind.can_fade() {
            Ok(kind)
        } else {
            Err(format!(
                "the board view cannot draw {} transparent yet",
                kind.token()
            ))
        }
    });
    let event = kind.and_then(|kind| {
        percent.map(|percent| PcbViewMsg::ObjectTransparencySet { kind, percent })
    });
    on_named_board(app, args, event)
}
