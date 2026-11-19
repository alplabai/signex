//! Window sizes of the dialogs whose height follows their content (a list of
//! unsaved documents, a long message). Computed when the window opens and again
//! when the content changes while it is open (docs/DETACHED_MODALS.md).

use iced::Size;

use crate::app::state::{ModalId, Signex};

use super::intro_wrap::{Wrapped, intro_growth, note_growth};

/// The most a list area grows to before it scrolls, in logical pixels.
pub(crate) const LIST_MAX_HEIGHT: f32 = 160.0;
/// Padding and border of the bordered list box around the rows
/// (`dialog_list`: 2 x `Spacing::s` + 2 x 1 px).
pub(crate) const LIST_BOX_CHROME: f32 = 10.0;
/// Height of one row of a dirty-document list.
const ROW_HEIGHT: f32 = 24.5;
/// Window height of a list dialog with an empty list: message, the list
/// box chrome, footer buttons and the dialog padding.
const LIST_DIALOG_BASE: f32 = 98.0;
/// One extra note line (kept documents, refresh notice, blocked discard).
pub(crate) const NOTE_HEIGHT: f32 = 25.0;
/// Height of one line of message text, and the lines shown before it scrolls.
const MESSAGE_LINE: f32 = 16.0;
const MESSAGE_MAX: f32 = 200.0;
/// Window height of Copy to Library when no library is writable: the body
/// padding, the two-line intro, the one-line hint and the footer, with no list
/// area (13 + 33 + 9 + 17 + 13 + 53 px on the legacy layout).
const STOCK_COPY_EMPTY: f32 = 140.0;
/// Height of one row of the Close Project list (`dialog_row`).
pub(crate) const DIALOG_ROW_HEIGHT: f32 = 30.0;
/// The most the Close Project list grows to before it scrolls.
pub(crate) const CLOSE_LIST_MAX_HEIGHT: f32 = 180.0;
/// A one-line info or warning callout and the gap above it.
const CALLOUT_HEIGHT: f32 = 39.0;
/// How much narrower than its column a callout's text runs: its padding,
/// icon and gap.
const CALLOUT_INSET: f32 = 40.0;
/// A Remove card with a one-line description, and the gap above it; the
/// base height holds two.
const REMOVE_CARD_HEIGHT: f32 = 66.0;
/// How much narrower than the window a Remove card's text runs: the body
/// and card padding, the borders, the radio and its gap.
const REMOVE_CARD_INSET: f32 = 80.0;

/// Height a Graphite theme adds to `modal`'s window over the legacy layout
/// the sizes here were measured on: what the shell's intro adds over the
/// line it replaces, and the taller dense footer buttons. `None` for a
/// dialog the shell leaves at its legacy height.
fn graphite_intro_extra(modal: ModalId) -> Option<f32> {
    use crate::styles::{DIALOG_ICON_TILE, DIALOG_INTRO_EXTRA, LINE_HEIGHT, Spacing, TypeScale};
    let body_line = TypeScale::DEFAULT.body * LINE_HEIGHT;
    Some(match modal {
        // A title line over the old message, which becomes the secondary text.
        ModalId::AnnotateResetConfirm
        | ModalId::ReprefixConfirm
        | ModalId::RemoveDialog
        | ModalId::ProjectCloseConfirm
        | ModalId::AppQuitConfirm
        | ModalId::CreateChildSheet
        | ModalId::LibraryStockCopy
        | ModalId::LibraryCloseConfirm
        | ModalId::LibraryDeleteConfirm
        | ModalId::LibraryRecovery => DIALOG_INTRO_EXTRA,
        // The message is the title: only the tile is taller than its line.
        ModalId::RenameDialog => DIALOG_ICON_TILE - body_line,
        // A title-only intro added above the form, with the form's gap.
        ModalId::GridProperties => DIALOG_ICON_TILE + 10.0,
        ModalId::SelectionFilterCustom => DIALOG_ICON_TILE + Spacing::DEFAULT.s,
        // The tile replaces a taller glyph row, or a summary line and its gap.
        ModalId::ErrorNotice | ModalId::NetlistIncompletePrompt | ModalId::EnableVersionControl => {
            0.0
        }
        _ => return None,
    })
}

/// See [`graphite_intro_extra`].
fn graphite_extra(modal: ModalId) -> f32 {
    graphite_intro_extra(modal).map_or(0.0, |intro| {
        intro + crate::styles::dense_footer_height() - crate::styles::dialog_footer_height()
    })
}

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

/// Extra height of a library dialog whose intro text wraps: the close
/// prompt and the stock copy (body padding `Spacing::l`) and the delete
/// confirm (`Spacing::xl`). Each text pairs with the one its base height
/// was measured with.
fn library_intro_growth(modal: ModalId, width: f32, texts: ((&str, &str), (&str, &str))) -> f32 {
    let pad = match modal {
        ModalId::LibraryDeleteConfirm => crate::styles::Spacing::DEFAULT.xl,
        _ => crate::styles::Spacing::DEFAULT.l,
    };
    let ((title, secondary), (ref_title, ref_secondary)) = texts;
    intro_growth(
        width,
        pad,
        Wrapped {
            text: title,
            reference: ref_title,
        },
        Wrapped {
            text: secondary,
            reference: ref_secondary,
        },
    )
}

/// Extra height of body-size `text` wrapping in a column `width` px wide,
/// beyond its first line.
fn body_growth(width: f32, text: &str) -> f32 {
    let ts = crate::styles::TypeScale::DEFAULT;
    // `note_growth` measures caption text: narrow the column by the size
    // ratio for the same characters per line, then scale the lines back up.
    note_growth(width * ts.caption / ts.body, 0.0, text) * ts.body / ts.caption
}

impl Signex {
    /// Window height of the Close Project confirm: its rows up to the list
    /// cap, then a callout each for the kept documents and the reason
    /// Discard All is withheld.
    fn close_confirm_height(&self, c: &crate::app::ProjectCloseConfirmState, width: f32) -> f32 {
        let column = width - 2.0 * crate::styles::Spacing::DEFAULT.l - CALLOUT_INSET;
        let rows = (c.dirty_paths.len() + c.dirty_rows.len()) as f32 * DIALOG_ROW_HEIGHT;
        let mut height = LIST_DIALOG_BASE + rows.min(CLOSE_LIST_MAX_HEIGHT);
        if !c.kept_paths.is_empty() {
            let names: Vec<String> = c
                .kept_paths
                .iter()
                .map(|p| crate::app::state::sheet_refs::file_label(p))
                .collect();
            let kept = signex_i18n::t!(
                self.ui_state.i18n.as_ref(),
                signex_i18n::ids::DIALOG_CLOSE_PROJECT_KEPT,
                "count" => c.kept_paths.len(),
                "names" => crate::app::state::sheet_refs::bounded_names(&names)
            );
            height += CALLOUT_HEIGHT + body_growth(column, &kept.to_string());
        }
        if let Some(reason) = c.sheet_refs.discard_blocked_reason() {
            height += CALLOUT_HEIGHT + body_growth(column, &reason);
        }
        height
    }

    /// Height the first stage of the Remove modal needs beyond its base
    /// (two cards with one-line descriptions): the notes, the lines a
    /// description wraps to, the warnings inside the cards, less a card a
    /// blocker hides.
    fn remove_dialog_growth(&self, dialog: &crate::app::RemoveDialogState, width: f32) -> f32 {
        let content = self.remove_content(dialog, self.ui_state.i18n.as_ref());
        let note_column = width - 2.0 * crate::styles::Spacing::DEFAULT.l - CALLOUT_INSET;
        let card_column = width - REMOVE_CARD_INSET;
        let hidden = 2usize.saturating_sub(content.cards.len());
        let mut growth = -(hidden as f32) * REMOVE_CARD_HEIGHT;
        for note in &content.notes {
            growth += CALLOUT_HEIGHT + body_growth(note_column, &note.plain());
        }
        for card in &content.cards {
            growth += body_growth(card_column, &card.description.to_string());
            if let Some(warning) = &card.warning {
                growth +=
                    CALLOUT_HEIGHT + body_growth(card_column - CALLOUT_INSET, &warning.to_string());
            }
        }
        growth
    }

    /// The size `modal`'s window should have right now.
    #[cfg(test)]
    pub(crate) fn modal_window_size(&self, modal: ModalId) -> Size {
        self.modal_window_size_of(modal, None)
    }

    /// [`Self::modal_window_size`] for the window of the browser at `owner`
    /// (the per-browser delete confirm shows that browser's row).
    pub(crate) fn modal_window_size_of(
        &self,
        modal: ModalId,
        owner: Option<&std::path::Path>,
    ) -> Size {
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
            ModalId::ProjectCloseConfirm => ui
                .project_close_confirm
                .as_ref()
                .map(|c| self.close_confirm_height(c, fixed.width)),
            ModalId::RemoveDialog => ui
                .remove_dialog
                .as_ref()
                .filter(|d| !d.unsaved_prompt)
                .map(|d| fixed.height + self.remove_dialog_growth(d, fixed.width)),
            ModalId::ErrorNotice => self.document_state.error_notice.as_ref().map(|n| {
                let detail = n.detail_text(&self.ui_state.i18n);
                130.0 + (wrapped_lines(&detail, 64) as f32 * MESSAGE_LINE).min(MESSAGE_MAX)
            }),
            ModalId::NetlistIncompletePrompt => self
                .document_state
                .netlist_incomplete_prompt
                .as_ref()
                .map(|p| {
                    let lines = wrapped_lines(&p.messages.join("\n"), 70) as f32;
                    236.0 + (lines * 14.0).min(120.0)
                }),
            ModalId::LibraryStockCopy => self.library.stock_copy.as_ref().and_then(|c| {
                if !self.library.writable_libraries().is_empty() {
                    return None;
                }
                let notes =
                    usize::from(c.previous_unavailable.is_some()) + usize::from(c.error.is_some());
                let noun = c.kind.noun();
                let (title, text) = crate::library::stock_copy::intro_texts(&c.label, noun);
                let (ref_title, ref_text) = crate::library::stock_copy::intro_texts("x", noun);
                let pad = crate::styles::Spacing::DEFAULT.l;
                let mut wrapped = library_intro_growth(
                    modal,
                    fixed.width,
                    ((&title, &text), (&ref_title, &ref_text)),
                );
                if let Some(previous) = c.previous_unavailable.as_deref() {
                    let note =
                        crate::library::stock_copy::previous_note(&self.ui_state.i18n, previous)
                            .into_string();
                    wrapped += note_growth(fixed.width, pad, &note);
                }
                if let Some(error) = c.error.as_deref() {
                    wrapped += note_growth(fixed.width, pad, error);
                }
                Some(STOCK_COPY_EMPTY + notes as f32 * NOTE_HEIGHT + wrapped)
            }),
            ModalId::LibraryCloseConfirm => self.library.close_library_confirm.as_ref().map(|c| {
                let drafts = c.dirty_editors.len() + c.dirty_paths.len();
                let (title, text) =
                    crate::library::close_prompt::intro_texts(&c.library_name, drafts);
                let (ref_title, ref_text) = crate::library::close_prompt::intro_texts("x", drafts);
                fixed.height
                    + library_intro_growth(
                        modal,
                        fixed.width,
                        ((title, &text), (ref_title, &ref_text)),
                    )
            }),
            ModalId::LibraryDeleteConfirm => owner
                .and_then(|path| self.library.library_browsers.get(path))
                .and_then(|b| b.delete_confirm.as_ref())
                .map(|d| {
                    let (title, text) = crate::library::edit_row_modal::delete_intro_texts(
                        &d.internal_pn,
                        &d.table,
                    );
                    let (ref_title, ref_text) =
                        crate::library::edit_row_modal::delete_intro_texts("x", "t");
                    fixed.height
                        + library_intro_growth(
                            modal,
                            fixed.width,
                            ((&title, &text), (&ref_title, &ref_text)),
                        )
                }),
            _ => None,
        };
        let logical = height.map_or(fixed, |h| Size::new(fixed.width, h));
        let extra = graphite_extra(modal);
        let logical = Size::new(logical.width, (logical.height + extra).ceil());
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
            let owner = self
                .ui_state
                .library_modal_owners
                .get(&id)
                .map(|p| p.as_path());
            let want =
                self.dialog_window_size_of(modal, owner, self.dialog_window_is_borderless(id));
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
