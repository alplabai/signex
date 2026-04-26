//! Symbol tab placeholder. Phase 2 wraps the existing schematic
//! editor scoped to one symbol body — pin tools, drawing tools,
//! field placement, alternate-symbol picker.

use iced::Element;
use signex_types::theme::ThemeTokens;

use super::super::messages::LibraryMessage;

pub fn view<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    super::placeholder_card(
        "Symbol Editor",
        &[
            "Wrap signex-render's symbol body renderer scoped to one part",
            "Pin add/remove/reorder",
            "Pin metadata: name, type, ERC class, alternates",
            "Draw primitives — Line / Rect / Arc / Circle / Polygon",
            "Field placement (Designator, Value, Footprint, etc.)",
            "Alternate-symbol unit picker (multi-unit parts)",
            "AI-stub: 'Generate from datasheet PDF' wizard",
        ],
        tokens,
    )
}
