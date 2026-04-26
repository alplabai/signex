//! Footprint tab placeholder. Phase 2 wraps the upcoming PCB
//! footprint editor.

use iced::Element;
use signex_types::theme::ThemeTokens;

use super::super::messages::LibraryMessage;

pub fn view<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    super::placeholder_card(
        "Footprint Editor",
        &[
            "Pad placement (THT / SMD / NPTH)",
            "Courtyard, fab, paste / solder mask layers",
            "Silk + assembly drawings",
            "3D model alignment overlay",
            "Live DRC against the active project's rule set",
        ],
        tokens,
    )
}
