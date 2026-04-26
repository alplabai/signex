//! History diff view — WS-E PENDING placeholder.
//!
//! WS-B rewrote `RevisionDiff` over the new binding-record fields
//! (`symbol_changed`, `footprint_changed`, `sim_changed`,
//! `pin_map_changed`, `params_changed`, `mpn_changed`,
//! `alternates_changed`, `supply_changed`, `datasheet_changed`,
//! `lifecycle_changed`, `plm_changed`). The pre-refactor diff card
//! used `SymbolDiff` / `FootprintDiff` / `SupplierDiff` which no
//! longer exist. WS-E owns the rebuild — render each `*_changed` flag
//! as a coloured chip plus a deeper Symbol/Footprint comparison via
//! the resolved primitives.
//!
//! TODO(merge-with-WS-E): wire `signex_library::diff::diff_revisions`
//! against the new `RevisionDiff` shape.

use iced::widget::{Space, column, container, text};
use iced::{Element, Length};
use signex_library::diff::diff_revisions;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::super::messages::LibraryMessage;
use super::super::super::state::ComponentEditorState;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let Some(version) = editor.history_selected else {
        return container(
            text("Pick a revision row above to see its diff.")
                .size(11)
                .color(muted),
        )
        .padding(10)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .into();
    };

    let head_idx = editor
        .component
        .revisions
        .iter()
        .position(|r| r.version == editor.component.head);
    let pick_idx = editor
        .component
        .revisions
        .iter()
        .position(|r| r.version == version);

    let summary = match (head_idx, pick_idx) {
        (Some(h), Some(p)) if h != p => {
            let a = &editor.component.revisions[p];
            let b = &editor.component.revisions[h];
            let d = diff_revisions(a, b);
            format!(
                "Diff v{} -> v{}: symbol={} footprint={} sim={} pin_map={} params={} mpn={} alternates={} supply={} datasheet={} lifecycle={}",
                a.version,
                b.version,
                d.symbol_changed,
                d.footprint_changed,
                d.sim_changed,
                d.pin_map_changed,
                d.params_changed,
                d.mpn_changed,
                d.alternates_changed,
                d.supply_changed,
                d.datasheet_changed,
                d.lifecycle_changed,
            )
        }
        _ => "Picked revision is the head -- nothing to diff against.".to_string(),
    };

    container(
        column![
            text("Visual Diff").size(13).color(text_c),
            Space::new().height(6),
            text(summary).size(11).color(text_c),
            Space::new().height(8),
            text("Visual diff renderer rebuild lives in WS-E (Symbol + Footprint primitive overlay).")
                .size(11)
                .color(muted),
        ]
        .spacing(0),
    )
    .padding(10)
    .style(crate::styles::modal_card(tokens))
    .width(Length::Fill)
    .into()
}
