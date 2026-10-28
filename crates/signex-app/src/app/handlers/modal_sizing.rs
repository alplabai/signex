//! Window sizes of the dialogs whose height follows their content (a list of
//! unsaved documents, a long message). Computed when the window opens and again
//! when the content changes while it is open (docs/DETACHED_MODALS.md).

use iced::Size;

use crate::app::state::{ModalId, Signex};

/// The most a list area grows to before it scrolls, in logical pixels.
pub(crate) const LIST_MAX_HEIGHT: f32 = 160.0;
/// Height of one row of a dirty-document list.
const ROW_HEIGHT: f32 = 22.5;
/// Window height of a list dialog with an empty list: title area, message,
/// footer buttons and their padding.
const LIST_DIALOG_BASE: f32 = 112.0;
/// One extra note line (kept documents, refresh notice, blocked discard).
const NOTE_HEIGHT: f32 = 32.0;
/// Height of one line of message text, and the lines shown before it scrolls.
const MESSAGE_LINE: f32 = 16.0;
const MESSAGE_MAX: f32 = 200.0;

fn list_height(rows: usize) -> f32 {
    (rows as f32 * ROW_HEIGHT).min(LIST_MAX_HEIGHT)
}

/// Rough wrapped line count of `text` at about `chars` characters per line.
fn wrapped_lines(text: &str, chars: usize) -> usize {
    text.lines()
        .map(|line| line.chars().count().div_ceil(chars).max(1))
        .sum::<usize>()
        .max(1)
}

impl Signex {
    /// The size `modal`'s window should have right now.
    pub(crate) fn modal_window_size(&self, modal: ModalId) -> Size {
        let fixed = crate::app::handlers::erc::modal_size(modal);
        let ui = &self.ui_state;
        let height = match modal {
            ModalId::AppQuitConfirm => ui.app_quit_confirm.as_ref().map(|c| {
                LIST_DIALOG_BASE
                    + list_height(c.dirty_paths.len() + c.dirty_rows.len())
                    + if ui.quit_confirm_refreshed {
                        NOTE_HEIGHT
                    } else {
                        0.0
                    }
            }),
            ModalId::ProjectCloseConfirm => ui.project_close_confirm.as_ref().map(|c| {
                LIST_DIALOG_BASE
                    + list_height(c.dirty_paths.len() + c.dirty_rows.len())
                    + if c.kept_paths.is_empty() {
                        0.0
                    } else {
                        NOTE_HEIGHT
                    }
                    + if c.sheet_refs.discard_blocked_reason().is_some() {
                        NOTE_HEIGHT
                    } else {
                        0.0
                    }
            }),
            ModalId::ErrorNotice => self.document_state.error_notice.as_ref().map(|n| {
                148.0 + (wrapped_lines(&n.detail, 64) as f32 * MESSAGE_LINE).min(MESSAGE_MAX)
            }),
            ModalId::NetlistIncompletePrompt => self
                .document_state
                .netlist_incomplete_prompt
                .as_ref()
                .map(|p| {
                    let lines = wrapped_lines(&p.messages.join("\n"), 70) as f32;
                    262.0 + (lines * 14.0).min(120.0)
                }),
            _ => None,
        };
        let logical = height.map_or(fixed, |h| Size::new(fixed.width, h.ceil()));
        // Every window lays out at the interface scale, so it needs that much
        // more window for the same content.
        let scale = ui.ui_scale;
        Size::new(
            (logical.width * scale).ceil(),
            (logical.height * scale).ceil(),
        )
    }

    /// Resize every open dialog window whose wanted size changed since its
    /// window opened (its content, or the interface scale). The first sighting
    /// is the size the window opened with; windows that are gone are forgotten.
    pub(crate) fn resize_content_sized_modals(&mut self) -> Vec<iced::Task<crate::app::Message>> {
        use crate::app::state::WindowKind;
        self.ui_state
            .modal_window_sizes
            .retain(|id, _| self.ui_state.windows.contains_key(id));
        let windows: Vec<(iced::window::Id, ModalId)> = self
            .ui_state
            .windows
            .iter()
            .filter_map(|(id, kind)| match kind {
                WindowKind::DetachedModal(m) => Some((*id, *m)),
                _ => None,
            })
            .collect();
        let mut tasks = Vec::new();
        for (id, modal) in windows {
            let want = self.modal_window_size(modal);
            let have = self.ui_state.modal_window_sizes.get(&id).copied();
            if have != Some((want.width, want.height)) {
                self.ui_state
                    .modal_window_sizes
                    .insert(id, (want.width, want.height));
                if have.is_some() {
                    tasks.push(iced::window::resize(id, want));
                }
            }
        }
        tasks
    }
}
