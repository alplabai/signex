//! 3D tab — WS-F note: this tab is now subsumed by the per-Footprint
//! Body 3D editor + procedural preview pane (see
//! `library::editor::footprint::body3d` and `preview3d`). The
//! standalone "3D" component-level tab is kept as a placeholder so the
//! tab strip's order stays stable for users; clicking it just renders
//! a pointer to the Footprint tab.
//!
//! The pre-refactor "3D" tab edited a top-level `PcbSide.model_3d`
//! that no longer exists — STEP attachments now ride on the
//! `Footprint::step_attachment` field per
//! `v0.9-library-refactor-plan.md` §2.2.

pub mod state;

use iced::widget::{Space, column, container, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::LibraryMessage;
use super::super::state::ComponentEditorState;

// Legacy re-exports — WS-F note: the standalone 3D tab is deprecated;
// STEP attachments live on `Footprint::step_attachment` now. Kept
// behind `#[allow(dead_code)]` so the module surface stays stable for
// WS-E's eventual rebuild.
#[allow(unused_imports, dead_code)]
pub use state::{Model3dUploadInfo, hash_bytes_hex, is_supported_extension};

/// Render the deprecated standalone 3D tab.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    _window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let attached = editor
        .footprint
        .as_ref()
        .and_then(|fp| fp.step_attachment.as_ref());

    let summary = match attached {
        Some(att) => format!(
            "Linked STEP: {} (sha256={}…)",
            att.filename,
            &att.content_hash[..8.min(att.content_hash.len())]
        ),
        None => "No STEP attached".to_string(),
    };

    let body = column![
        text("3D Body & STEP").size(13).color(text_c),
        Space::new().height(6),
        text("3D body parameters and STEP attachments now live on the")
            .size(11)
            .color(muted),
        text("Footprint tab — see the right column there.")
            .size(11)
            .color(muted),
        Space::new().height(10),
        text(summary).size(11).color(text_c),
    ]
    .spacing(0);

    container(body)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}
