//! The Body row's facts for the Properties panel (#1233): which body a placed
//! symbol stores and whether its library symbol has an alternate one.

use crate::panels::{BODY_ALTERNATE, BODY_ALTERNATE_MISSING, BODY_NORMAL, BODY_NORMAL_CAN_SWITCH};
use signex_types::schematic::{BodyStyle, SchematicSheet};
use uuid::Uuid;

/// `("Body Style", state)` for the placed symbol `symbol`, or nothing when it
/// is not a placed symbol with an embedded library symbol.
pub(super) fn body_style_info(sheet: &SchematicSheet, symbol: Uuid) -> Option<(String, String)> {
    let sym = sheet.symbols.iter().find(|s| s.uuid == symbol)?;
    let lib = sheet.lib_symbols.get(&sym.lib_id)?;
    let has_alternate = lib.has_alternate_body(sym.unit);
    let state = match (sym.body_style, has_alternate) {
        (BodyStyle::Alternate, true) => BODY_ALTERNATE,
        (BodyStyle::Alternate, false) => BODY_ALTERNATE_MISSING,
        (BodyStyle::Normal, true) => BODY_NORMAL_CAN_SWITCH,
        (BodyStyle::Normal, false) => BODY_NORMAL,
    };
    Some(("Body Style".to_string(), state.to_string()))
}
