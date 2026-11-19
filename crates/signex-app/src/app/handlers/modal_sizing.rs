//! Window sizes of the dialogs whose height follows their content (a list of
//! unsaved documents, a long message). Computed when the window opens and again
//! when the content changes while it is open (docs/DETACHED_MODALS.md).

use iced::Size;

use crate::app::state::{ModalId, Signex};
use crate::library::modal_chrome::{
    BANNER_GAP, BANNER_GLYPH, BANNER_PAD_X, BANNER_PAD_Y, KV_ROW_GAP, NOTE_LINE_H,
};
use crate::styles::{HAIRLINE, LINE_HEIGHT, Spacing, TypeScale};

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
/// Window height of Copy to Library around its library list or its
/// no-library card: the body padding, the two-line intro and its gap, and the
/// footer (13 + 33 + 9 + 13 + 53 px on the legacy layout).
const STOCK_COPY_BASE: f32 = 123.0;
/// The no-library card of Copy to Library: one hint line inside the card's
/// padding and border.
const STOCK_COPY_EMPTY_CARD: f32 =
    17.0 + 2.0 * (crate::library::stock_copy::EMPTY_CARD_PAD_Y + HAIRLINE);
/// A one-line notice banner and the gap above it.
pub(crate) const BANNER_BLOCK: f32 =
    2.0 * BANNER_PAD_Y + TypeScale::DEFAULT.body * LINE_HEIGHT + Spacing::DEFAULT.m;
/// Window height of a recovery dialog without its fact rows: the body
/// padding, a one-line intro text and its gap, the facts box padding and
/// border, the gap and one caption line, the space above the footer and the
/// footer (13 + 17 + 9 + 22 + 9 + 15 + 13 + 53 px on the legacy layout, and 5
/// to spare).
const RECOVERY_BASE: f32 = 156.0;
/// One row of the recovery facts box and the gap after it.
const RECOVERY_FACT: f32 = TypeScale::DEFAULT.body * LINE_HEIGHT + KV_ROW_GAP;

/// Height of a notice banner showing `text` in a card `card_w` wide whose
/// body has `pad` px of padding: [`BANNER_BLOCK`] and the lines its text
/// wraps onto. [`note_growth`] counts caption-size lines, so the column is
/// scaled to the caption size and the growth back to the body size.
pub(crate) fn banner_height(card_w: f32, pad: f32, text: &str) -> f32 {
    let ts = TypeScale::DEFAULT;
    let column = card_w - 2.0 * (pad + BANNER_PAD_X) - BANNER_GLYPH - BANNER_GAP;
    BANNER_BLOCK + note_growth(column * ts.caption / ts.body, 0.0, text) * ts.body / ts.caption
}

/// New Library: what a refusal banner adds to the window besides its text
/// lines — the gap above it, its padding and the action button row.
const REFUSAL_BANNER: f32 = 60.0;
/// Characters a line of the New Library refusal banner holds.
const REFUSAL_LINE_CHARS: usize = 80;

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

impl Signex {
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
            ModalId::LibraryStockCopy => self.library.stock_copy.as_ref().map(|c| {
                use crate::library::stock_copy::{intro_texts, list_height, previous_note};
                let i18n = &self.ui_state.i18n;
                let rows = self.library.writable_libraries().len();
                let (title, text) = intro_texts(i18n, &c.label);
                let (ref_title, ref_text) = intro_texts(i18n, "x");
                let pad = Spacing::DEFAULT.l;
                let content = if rows == 0 {
                    STOCK_COPY_EMPTY_CARD
                } else {
                    list_height(rows)
                };
                let mut height = STOCK_COPY_BASE
                    + content
                    + library_intro_growth(
                        modal,
                        fixed.width,
                        ((&title, &text), (&ref_title, &ref_text)),
                    );
                if let Some(previous) = c.previous_unavailable.as_deref() {
                    let note = previous_note(i18n, previous).into_string();
                    height += banner_height(fixed.width, pad, &note);
                }
                if let Some(error) = c.error.as_deref() {
                    height += banner_height(fixed.width, pad, error);
                }
                height
            }),
            ModalId::LibraryCloseConfirm => self.library.close_library_confirm.as_ref().map(|c| {
                use crate::library::close_prompt::{LIST_MAX_H, intro_texts, list_height};
                let i18n = &self.ui_state.i18n;
                let drafts = c.dirty_editors.len() + c.dirty_paths.len();
                let (title, text) = intro_texts(i18n, &c.library_name, drafts);
                let (ref_title, ref_text) = intro_texts(i18n, "x", drafts);
                // The base height holds the longest list; the note line
                // names the library above the body.
                fixed.height - LIST_MAX_H
                    + list_height(drafts)
                    + NOTE_LINE_H
                    + library_intro_growth(
                        modal,
                        fixed.width,
                        ((&title, &text), (&ref_title, &ref_text)),
                    )
            }),
            ModalId::LibraryRecovery => self.library.recovery.as_ref().map(|d| {
                use crate::library::recovery as r;
                let i18n = &self.ui_state.i18n;
                let (title, text) = r::intro_texts(d, i18n);
                let (ref_title, ref_text) = r::reference_intro_texts(d, i18n);
                let pad = Spacing::DEFAULT.l;
                let caption = r::caption(d, i18n).into_string();
                let mut height = RECOVERY_BASE + r::fact_rows(d) as f32 * RECOVERY_FACT
                    - KV_ROW_GAP
                    + note_growth(fixed.width, pad, &caption)
                    + library_intro_growth(
                        modal,
                        fixed.width,
                        ((&title, &text), (&ref_title, &ref_text)),
                    );
                if r::note(d, self.recovery_project_file()).is_some() {
                    height += NOTE_LINE_H;
                }
                for banner in r::banners(d, i18n) {
                    height += banner_height(fixed.width, pad, &banner.into_string());
                }
                height
            }),
            ModalId::LibraryCreateOptions => self
                .library
                .create_options
                .as_ref()
                .and_then(|c| c.refusal.as_deref())
                .map(|reason| {
                    let line = crate::styles::TypeScale::DEFAULT.body * crate::styles::LINE_HEIGHT;
                    fixed.height
                        + REFUSAL_BANNER
                        + wrapped_lines(reason, REFUSAL_LINE_CHARS) as f32 * line
                }),
            ModalId::LibraryDeleteConfirm => owner
                .and_then(|path| self.library.library_browsers.get(path))
                .and_then(|b| b.delete_confirm.as_ref())
                .map(|d| {
                    use crate::library::edit_row_modal::delete_intro_texts;
                    let i18n = &self.ui_state.i18n;
                    let (title, text) = delete_intro_texts(i18n, &d.internal_pn, &d.table);
                    let (ref_title, ref_text) = delete_intro_texts(i18n, "x", "t");
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
