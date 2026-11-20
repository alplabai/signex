//! Window sizes of the dialogs whose height follows their content (a list of
//! unsaved documents, a long message). Computed when the window opens and again
//! when the content changes while it is open (docs/DETACHED_MODALS.md).

use iced::Size;

use crate::app::state::{ModalId, Signex};

use super::intro_wrap::{Wrapped, intro_growth, note_growth};

/// The most a list area grows to before it scrolls, in logical pixels.
pub(crate) const LIST_MAX_HEIGHT: f32 = 160.0;
/// The most the Enable Version Control file list grows to before it
/// scrolls, so the dialog fits a 768 px screen with its commit message field.
pub(crate) const TRACK_LIST_MAX_HEIGHT: f32 = 180.0;
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
/// The most the rows of the netlist-incomplete prompt's omitted-page list
/// grow to before they scroll (the heading row stays above them).
pub(crate) const OMITTED_LIST_MAX: f32 = 140.0;
/// Window height of the netlist-incomplete prompt without its list: the
/// intro, the anyway note, the footer and the dialog padding.
const NETLIST_PROMPT_BASE: f32 = 236.0;
/// The omitted-page list's heading row, its rule and the list border.
const OMITTED_LIST_CHROME: f32 = 29.0;
/// One omitted-page row and the rule under it.
const OMITTED_ROW: f32 = 31.0;
/// One callout under the Create Sheet field (an error or the copy warning):
/// a padded line and the gap above it.
const SHEET_CALLOUT_HEIGHT: f32 = 40.0;

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

/// One line of body text and of caption text.
fn body_line() -> f32 {
    crate::styles::TypeScale::DEFAULT.body * crate::styles::LINE_HEIGHT
}
fn caption_line() -> f32 {
    crate::styles::TypeScale::DEFAULT.caption * crate::styles::LINE_HEIGHT
}
/// A note under a dialog's field (callout or inline error) of `lines` body
/// lines, with the gap above it; a callout adds its padding.
fn note_block(lines: usize, padded: bool) -> f32 {
    let pad = if padded { 2.0 * 7.0 } else { 0.0 };
    crate::styles::Spacing::DEFAULT.l + pad + lines as f32 * body_line()
}

/// What the Rename dialog adds over the layout its base height was measured
/// on (mockup row 23): the path under the title, the "New name" label, the wider gap
/// above the field, then the placing-symbol note, the name clash and the
/// handler's error when they show.
fn rename_growth(d: &crate::app::RenameDialogState) -> f32 {
    let sp = crate::styles::Spacing::DEFAULT;
    let mut grow = caption_line() + sp.xs + 1.0 + body_line() + 4.0 + (sp.l - sp.m);
    if !d.placing_symbols.is_empty() {
        grow += note_block(2, true);
    }
    if d.name_clash.is_some() {
        grow += note_block(1, false);
    }
    if let Some(error) = &d.error {
        grow += note_block(wrapped_lines(error, 60), false);
    }
    grow
}

/// Window height of Enable Version Control (mockup row 26): the body padding, the
/// intro (title, mono path line, three-line sentence), the commit message field, the LFS option, Commit every
/// save (project scope) and the legacy footer the Graphite extra replaces,
/// then the file list (capped at [`TRACK_LIST_MAX_HEIGHT`]) or the scanning
/// line, and the error.
fn version_control_height(s: &crate::app::EnableVersionControlState) -> f32 {
    let sp = crate::styles::Spacing::DEFAULT;
    let title_line = crate::styles::TypeScale::DEFAULT.title * crate::styles::LINE_HEIGHT;
    let intro = title_line + sp.xs + 1.0 + caption_line() + sp.xs + 3.0 * body_line();
    let field = body_line() + 4.0 + crate::styles::ChromeMetrics::DEFAULT.field;
    let option = body_line() + sp.xs + caption_line();
    let mut height = sp.l + intro + sp.l + field + sp.l + option;
    if s.scope == crate::app::VersionControlScope::Project {
        // Its help runs to two caption lines.
        height += sp.l + option + caption_line();
    }
    if s.scanning {
        height += sp.l + caption_line();
    } else if !s.items.is_empty() {
        let rows = s.items.len() as f32;
        let list = (rows * 30.0 + (rows - 1.0)).min(TRACK_LIST_MAX_HEIGHT) + 2.0;
        height += sp.l + caption_line() + sp.s + 2.0 + list;
    }
    if let Some(error) = &s.error {
        height += note_block(wrapped_lines(error, 80), false);
    }
    height + crate::styles::dialog_footer_height()
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
                    let rows = p.omitted.len() as f32 * OMITTED_ROW;
                    NETLIST_PROMPT_BASE + OMITTED_LIST_CHROME + rows.min(OMITTED_LIST_MAX)
                }),
            // The note under the field in the new-file state, else one
            // callout per error or warning, each grown by its wrapped lines.
            ModalId::CreateChildSheet => ui.create_child_sheet_confirm.as_ref().map(|c| {
                let pad = crate::styles::Spacing::DEFAULT.l;
                let callouts: Vec<String> = [c.error.as_ref(), c.reuse_warning.as_ref()]
                    .into_iter()
                    .flatten()
                    .map(|t| t.render(&*self.ui_state.i18n))
                    .collect();
                if callouts.is_empty() {
                    return fixed.height + NOTE_HEIGHT;
                }
                callouts.iter().fold(fixed.height, |height, text| {
                    height + SHEET_CALLOUT_HEIGHT + note_growth(fixed.width - 2.0 * pad, pad, text)
                })
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
            ModalId::RenameDialog => ui
                .rename_dialog
                .as_ref()
                .map(|d| fixed.height + rename_growth(d)),
            ModalId::EnableVersionControl => ui
                .enable_version_control
                .as_ref()
                .map(version_control_height),
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
