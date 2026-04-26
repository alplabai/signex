//! Simulation tab placeholder. Phase 2 wires the SPICE model body
//! editor + pin-to-node mapping.

use iced::Element;
use signex_types::theme::ThemeTokens;

use super::super::messages::LibraryMessage;

pub fn view<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    super::placeholder_card(
        "Simulation",
        &[
            "SPICE model body editor (text)",
            "Pin → SPICE node binding table",
            "Auto-detect from common library shapes (BJT, MOSFET, opamp, etc.)",
            "Test bench template hookup once Signex Sim ships (v4.0)",
        ],
        tokens,
    )
}
