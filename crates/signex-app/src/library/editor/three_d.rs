//! 3D tab placeholder. Phase 2 wraps the model viewer + alignment
//! UI.

use iced::Element;
use signex_types::theme::ThemeTokens;

use super::super::messages::LibraryMessage;

pub fn view<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    super::placeholder_card(
        "3D Model",
        &[
            "STEP / WRL / GLB upload",
            "Pad-to-pin alignment preview",
            "Offset + rotation overlay",
            "Bake-to-asset workflow (cached *.glb in shared/3d-models)",
        ],
        tokens,
    )
}
