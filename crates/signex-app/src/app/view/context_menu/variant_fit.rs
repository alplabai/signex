//! The In variant flyout's fit rows on a repeated sheet (#2662, mockup row
//! 32): on a channel tab, Fitted and Not fitted each offer "This channel
//! only (CH2)" and "All channels"; on Editor, which shows the sheet once for
//! every channel, only "All channels". Each row sets the copies the channel
//! strip lists for the selected part, as the cards do: one undo step of the
//! General history. Before a compile the strip lists no copies and the rows
//! say to compile; in Base they say to pick a variant.

use super::*;

use super::items::{dd_disabled, dd_kb, text};
use crate::panels::compiled_design::ChannelCopies;
use signex_i18n::{I18n, MessageId, menu_ids};
use signex_types::project::PartRef;
use signex_widgets::active_bar_dropdown::DropdownEntry;

impl Signex {
    /// The copies of the part selected in `window_id`, when its sheet is
    /// the repeated sheet the channel strip shows; `None` on a plain sheet.
    pub(super) fn canvas_menu_channel_copies(
        &self,
        window_id: iced::window::Id,
    ) -> Option<ChannelCopies> {
        let path = self
            .document_state
            .sheet_path_for_window(window_id, &self.ui_state)?;
        let strip = self
            .document_state
            .panel_ctx
            .compiled_design
            .strip
            .as_ref()?;
        if strip.sheet != *path {
            return None;
        }
        let engine = self
            .document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let canvas = self.interaction_state.canvas_for_window(window_id);
        let symbol = crate::app::handlers::placed_part::selected_placed_symbol(engine, canvas)?;
        Some(strip.copies_of(symbol.uuid))
    }
}

/// The Fitted and Not fitted rows for `copies` in the active variant
/// (`is_base` while Base is active).
pub(super) fn channel_fit_rows(
    i18n: &I18n,
    copies: &ChannelCopies,
    is_base: bool,
) -> Vec<DropdownEntry<Message>> {
    let reason = match (is_base, copies.all.is_empty()) {
        (true, _) => Some(menu_ids::CTX_IN_VARIANT_IN_BASE),
        (false, true) => Some(menu_ids::CTX_IN_VARIANT_COMPILE_FIRST),
        (false, false) => None,
    };
    let mut rows = Vec::with_capacity(4);
    for (fit, fitted) in [
        (signex_i18n::ids::PROJECT_VARIANTS_FITTED, true),
        (signex_i18n::ids::PROJECT_VARIANTS_NOT_FITTED, false),
    ] {
        let fit = text(i18n, fit);
        if let Some((channel, part)) = &copies.shown {
            let label = i18n.message_plain(
                menu_ids::CTX_IN_VARIANT_THIS_CHANNEL,
                &[
                    ("fit", fit.clone().into()),
                    ("channel", channel.clone().into()),
                ],
            );
            rows.push(fit_row(i18n, label, vec![part.clone()], fitted, reason));
        }
        let label = i18n.message_plain(
            menu_ids::CTX_IN_VARIANT_ALL_CHANNELS,
            &[("fit", fit.into())],
        );
        rows.push(fit_row(i18n, label, copies.all.clone(), fitted, reason));
    }
    rows
}

/// A live row that sets `parts`, or a greyed one saying `reason`.
fn fit_row(
    i18n: &I18n,
    label: String,
    parts: Vec<PartRef>,
    fitted: bool,
    reason: Option<MessageId>,
) -> DropdownEntry<Message> {
    let Some(reason) = reason else {
        let action = ContextAction::VariantFit { parts, fitted };
        return dd_kb(None, &label, "", action);
    };
    match dd_disabled(None, &label, None) {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.tooltip(text(i18n, reason))),
        other => other,
    }
}
