//! Bring a clicked pin forward in the Properties rows of a placed symbol.

use signex_engine::info_keys;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

/// Add the `Focused Pin` row, and the net that pin sits on, to the rows of
/// `symbol` when the click that selected it landed on one of its pins.
pub(super) fn focus_clicked_pin(
    info: &mut Vec<(String, String)>,
    document: &SchematicSheet,
    symbol: Uuid,
    clicked: Option<(Uuid, String)>,
) {
    let Some((_, pin)) = clicked.filter(|(owner, _)| *owner == symbol) else {
        return;
    };
    let in_rows = info
        .iter()
        .any(|(k, _)| k.strip_prefix(info_keys::PIN_PREFIX) == Some(pin.as_str()));
    if !in_rows {
        return;
    }
    info.push((info_keys::FOCUSED_PIN.into(), pin.clone()));
    let netlist = signex_net::build_netlist(document);
    let net = netlist.nets.iter().find(|net| {
        net.terminals
            .iter()
            .any(|t| t.symbol == symbol && t.pin == pin)
    });
    if let Some(net) = net {
        info.push((
            format!("{}{pin}", info_keys::PIN_NET_PREFIX),
            net.name.clone(),
        ));
    }
}
