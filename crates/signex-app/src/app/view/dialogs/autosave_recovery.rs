//! The recovery prompt (#1164): a document was opened and a recovery copy
//! with edits its file lacks exists. The two versions are listed with their
//! times, the newer one marked. Discard / Decide later / Restore; Esc and the
//! header close are Decide later (`OpenOverlays::rung`), so a stray key never
//! deletes or loads anything.

use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};

use super::*;
use iced::widget::{Column, column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{DateLength, I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use super::graphite_list::{
    LIST_TILE_ROW_H, accent_pill, kind_code, kind_tile, list_box, list_row,
};
use crate::autosave::{AutosaveMsg, RecoveryChoice, RecoveryOffer, describe_age};
use crate::styles::{
    DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_footer_split,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Ages under a day show the time of day only.
const DAY_SECS: u64 = 86_400;

/// The instant `age_secs` before `checked_at`.
pub(super) fn instant_before(checked_at: SystemTime, age_secs: u64) -> DateTime<Utc> {
    let at = checked_at
        .checked_sub(Duration::from_secs(age_secs))
        .unwrap_or(checked_at);
    DateTime::<Utc>::from(at)
}

/// The local time `age_secs` before `checked_at`, in the UI locale: the time
/// of day within a day of the check, else the date and time.
pub(super) fn clock_time(i18n: &I18n, checked_at: SystemTime, age_secs: u64) -> String {
    let at = instant_before(checked_at, age_secs);
    if age_secs < DAY_SECS {
        i18n.time(at)
    } else {
        i18n.date_time(at, DateLength::Medium)
    }
}

/// The recovery copy is the newer version: it gets the "newer" pill.
fn copy_is_newer(offer: &RecoveryOffer) -> bool {
    !offer.older_than_file
}

/// The medium-weight UI face of a version's name and of the lead line.
fn medium() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// One version row: the kind tile, the version's name over its caption, and
/// the "newer" pill when it is the newer one.
fn version_row<'a>(
    i18n: &I18n,
    tile: Option<Element<'a, Message>>,
    title: Localized,
    caption: Element<'a, Message>,
    newer: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let name = text(title)
        .size(TS.body)
        .font(medium())
        .color(crate::styles::ti(tokens.text));
    let mut cells: Vec<Element<'a, Message>> = tile.into_iter().collect();
    cells.push(
        column![name, caption]
            .spacing(SP.xs / 2.0)
            .width(Length::Fill)
            .boxed(),
    );
    if newer {
        cells.push(accent_pill(t!(i18n, ids::DIALOG_RECOVERY_NEWER), tokens));
    }
    list_row(cells, LIST_TILE_ROW_H)
}

/// The one-line caption under a version's name; `mono` for a path.
fn caption<'a>(
    label: impl Into<Label<'a>>,
    mono: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let line = text(label)
        .size(TS.caption)
        .color(crate::styles::tokens::resolve(tokens).ink4)
        .wrapping(iced::widget::text::Wrapping::None);
    if mono {
        line.font(super::graphite::mono()).boxed()
    } else {
        line.boxed()
    }
}

impl Signex {
    pub(in crate::app::view) fn view_autosave_recovery(
        &self,
        offer: &RecoveryOffer,
    ) -> Element<'_, Message> {
        self.wrap_centred_dialog(
            self.autosave_recovery_card(offer, false),
            super::sizes::AUTOSAVE_RECOVERY,
        )
    }

    /// The prompt as the content of its own OS window (no header, fills it).
    pub(in crate::app::view) fn view_autosave_recovery_window(
        &self,
        offer: &RecoveryOffer,
    ) -> Element<'_, Message> {
        self.autosave_recovery_card(offer, true)
    }

    /// The header: the title, and a close that is Decide later, as its
    /// tooltip says.
    fn autosave_recovery_header<'a>(&self, later: Message) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let tip = container(
            text(t!(i18n, ids::DIALOG_RECOVERY_CLOSE_TOOLTIP))
                .size(TS.caption)
                .color(crate::styles::ti(tokens.text)),
        )
        .padding([SP.xs, SP.m])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            crate::styles::tokens::hover_overlay(tokens),
            crate::styles::ti(tokens.border),
        ));
        let close = signex_widgets::tooltip(
            super::header::neutral_close_button(tokens, later),
            tip,
            iced::widget::tooltip::Position::Bottom,
        )
        .gap(SP.xs)
        .boxed();
        super::header::graphite_header_closing(
            t!(i18n, ids::DIALOG_TITLE_AUTOSAVE_RECOVERY).into_string(),
            None,
            Some(close),
            None,
            super::header::DIALOG_HEADER_H,
            tokens,
        )
    }

    /// The two version rows: the recovery copy, then the saved file. A
    /// Component Preview row has no file: its second row names the library,
    /// with no time.
    fn recovery_versions<'a>(
        &self,
        offer: &RecoveryOffer,
        library: Option<String>,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let code: Option<MessageId> = if library.is_some() {
            Some(ids::PANELS_TAB_KIND_COMPONENT)
        } else {
            kind_code(&offer.path)
        };
        let tile = || code.map(|code| kind_tile(i18n, code, tokens));
        let copy_caption = caption(
            t!(
                i18n,
                ids::DIALOG_AUTOSAVE_RECOVERY_AUTOSAVED_AGE,
                "age" => describe_age(offer.age_secs),
                "time" => clock_time(i18n, offer.checked_at, offer.age_secs)
            ),
            false,
            tokens,
        );
        let saved_caption = match (library, offer.saved_age_secs) {
            (Some(library), _) => caption(UserText::new(library), false, tokens),
            (None, Some(age)) => caption(
                t!(
                    i18n,
                    ids::DIALOG_RECOVERY_SAVED_TIME,
                    "path" => offer.path.display().to_string(),
                    "time" => clock_time(i18n, offer.checked_at, age)
                ),
                true,
                tokens,
            ),
            (None, None) => caption(
                UserText::new(offer.path.display().to_string()),
                true,
                tokens,
            ),
        };
        let newer = copy_is_newer(offer);
        list_box(
            vec![
                version_row(
                    i18n,
                    tile(),
                    t!(i18n, ids::DIALOG_RECOVERY_COPY),
                    copy_caption,
                    newer,
                    tokens,
                ),
                version_row(
                    i18n,
                    tile(),
                    t!(i18n, ids::DIALOG_RECOVERY_SAVED),
                    saved_caption,
                    !newer,
                    tokens,
                ),
            ],
            tokens,
        )
    }

    /// The name the prompt shows and, for a Component Preview row (which has
    /// no file), the label of its library.
    fn recovery_subject(&self, offer: &RecoveryOffer) -> (String, Option<String>) {
        let (row_name, library) = self
            .library
            .editors
            .iter()
            .find(|(address, _)| address.synthetic_tab_path() == offer.path)
            .map(|(address, editor)| {
                (
                    editor.row.internal_pn.as_str().to_string(),
                    self.library.library_label(&address.library_path),
                )
            })
            .unzip();
        let name = row_name.unwrap_or_else(|| {
            offer
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| offer.path.display().to_string())
        });
        (name, library)
    }

    /// The notes under the versions. The copy is newer: what Restore does,
    /// in the info callout. The saved file is newer: the warning that
    /// Restore replaces that newer work, then what Restore does. Both end
    /// with the Discard / Decide later caption, the one place Decide later
    /// (and so Esc) is explained.
    fn recovery_notes(&self, offer: &RecoveryOffer, name: &str) -> Element<'_, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let ink4 = crate::styles::tokens::resolve(tokens).ink4;
        let hint = |label: Localized| {
            text(label)
                .size(TS.caption)
                .color(ink4)
                .width(Length::Fill)
                .boxed()
        };
        let restore = t!(
            i18n,
            ids::DIALOG_RECOVERY_RESTORE_HINT,
            "undo" => crate::keymap::shown_in("Mod+Z", i18n)
        );
        let mut notes: Vec<Element<'_, Message>> = Vec::with_capacity(3);
        if copy_is_newer(offer) {
            notes.push(super::graphite::callout(DialogIcon::Info, restore, tokens));
        } else {
            notes.push(super::graphite::callout(
                DialogIcon::Warn,
                t!(i18n, ids::DIALOG_RECOVERY_OLDER, "name" => name),
                tokens,
            ));
            notes.push(hint(restore));
        }
        notes.push(hint(t!(i18n, ids::DIALOG_RECOVERY_LATER_HINT)));
        Column::with_children(notes).spacing(SP.m).boxed()
    }

    /// Restore and Discard at the left, a quiet Decide later at the right;
    /// Esc keeps meaning Decide later.
    fn recovery_footer(&self) -> Element<'_, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let choose = |choice| Some(Message::Autosave(AutosaveMsg::Recovery(choice)));
        dialog_footer_split(
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_AUTOSAVE_RECOVERY_RESTORE),
                    DialogButtonKind::Primary,
                    choose(RecoveryChoice::Restore),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_DISCARD),
                    DialogButtonKind::Danger,
                    choose(RecoveryChoice::Discard),
                    tokens,
                ),
            ],
            vec![dialog_button(
                t!(i18n, ids::DIALOG_AUTOSAVE_RECOVERY_LATER),
                DialogButtonKind::Ghost,
                choose(RecoveryChoice::Later),
                tokens,
            )],
        )
    }

    fn autosave_recovery_card(
        &self,
        offer: &RecoveryOffer,
        windowed: bool,
    ) -> Element<'_, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let header: Element<'_, Message> = if windowed {
            super::widgets::os_title_bar_slot()
        } else {
            self.autosave_recovery_header(Message::Autosave(AutosaveMsg::Recovery(
                RecoveryChoice::Later,
            )))
        };
        let (name, library) = self.recovery_subject(offer);
        let lead = text(t!(i18n, ids::DIALOG_RECOVERY_FOUND, "name" => name.as_str()))
            .size(TS.label)
            .font(medium())
            .color(crate::styles::ti(tokens.text));
        let body: Column<Element<'_, Message>> = column![
            column![lead.boxed(), self.recovery_versions(offer, library)].spacing(SP.m),
            self.recovery_notes(offer, &name),
        ]
        .spacing(SP.l);
        let footer = self.recovery_footer();
        let dialog = container(
            column![header, dialog_body(body), footer].width(if windowed {
                Length::Fill
            } else {
                Length::Fixed(super::widgets::modal_card_width(
                    self.ui_state.window_size,
                    super::sizes::AUTOSAVE_RECOVERY.0,
                ))
            }),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        if windowed {
            dialog.width(Length::Fill).height(Length::Fill).boxed()
        } else {
            dialog.boxed()
        }
    }
}

#[cfg(test)]
#[path = "autosave_recovery_tests.rs"]
mod tests;
