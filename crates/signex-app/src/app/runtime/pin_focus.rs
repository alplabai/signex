//! Pin facts in the Properties rows of a placed symbol: the net of each pin
//! and the pin a click landed on.

use signex_engine::info_keys;
use uuid::Uuid;

/// The keys of the pins listed in `info`, in order.
pub(super) fn pin_keys(info: &[(String, String)]) -> Vec<String> {
    info.iter()
        .filter_map(|(k, _)| k.strip_prefix(info_keys::PIN_PREFIX).map(str::to_string))
        .collect()
}

/// Add each pin's net (`nets`: pin key and net name) and, when the click that
/// selected `symbol` landed on one of its pins, the `Focused Pin` row.
pub(super) fn add_pin_rows(
    info: &mut Vec<(String, String)>,
    symbol: Uuid,
    clicked: Option<(Uuid, String)>,
    nets: Vec<(String, String)>,
) {
    for (pin, net) in nets {
        info.push((format!("{}{pin}", info_keys::PIN_NET_PREFIX), net));
    }
    let Some((_, pin)) = clicked.filter(|(owner, _)| *owner == symbol) else {
        return;
    };
    if pin_keys(info).contains(&pin) {
        info.push((info_keys::FOCUSED_PIN.into(), pin));
    }
}

impl crate::app::Signex {
    /// Show `clicked` as the highlighted pin on the active canvas and drop any
    /// highlight another canvas still shows: only the selection a pin click
    /// made keeps one (#2086).
    pub(super) fn sync_focused_pin(&mut self, clicked: Option<(Uuid, String)>) {
        let state = &mut self.interaction_state;
        if state.canvas.focused_pin != clicked {
            state.canvas.focused_pin = clicked;
            state.canvas.clear_overlay_cache();
        }
        for slot in state.canvases.values_mut() {
            if slot.focused_pin.take().is_some() {
                slot.clear_overlay_cache();
            }
        }
    }
}
