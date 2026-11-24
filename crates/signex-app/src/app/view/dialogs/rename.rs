//! The Rename dialog (mockup row 23): one name field for a project file or the project
//! itself. A file keeps its extension as a fixed suffix, the sheet symbols
//! that place a sheet are named, and a name another known file has is shown
//! at once.

use super::*;
use iced::widget::{Space, column, container, stack};
use iced::{Element, Length, Padding};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{raw, text, text_input};

use super::widgets::modal_title_header;
use crate::app::path_ops::PlacingSymbol;
use crate::styles::{
    CalloutTone, DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_callout,
    dialog_field_label, dialog_footer, dialog_inline_error, dialog_intro_with,
};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Horizontal padding inside the name field.
const FIELD_PAD_X: f32 = 8.0;
/// Average glyph advance of the mono face, in text-size units, for the room
/// the suffix takes at the field's right end.
const MONO_ADVANCE_EM: f32 = 0.62;

/// The name field split for display: the part the user edits and the fixed
/// extension after it (empty for a project or a file without one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app::view) struct NameParts {
    pub stem: String,
    pub suffix: String,
}

impl NameParts {
    /// Split `buffer` (the whole new name) for the dialog on `target`. A
    /// project rename edits the bare name. A buffer that does not end in the
    /// extension (set by a caller other than the field) is shown whole; the
    /// next edit puts the extension back.
    pub(in crate::app::view) fn of(
        buffer: &str,
        target: &std::path::Path,
        is_project_rename: bool,
    ) -> Self {
        let suffix = if is_project_rename {
            String::new()
        } else {
            target
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default()
        };
        let stem = buffer.strip_suffix(suffix.as_str()).unwrap_or(buffer);
        Self {
            stem: stem.to_string(),
            suffix,
        }
    }

    /// The whole name an edit of the stem stands for.
    pub(in crate::app::view) fn compose(&self, stem: &str) -> String {
        format!("{stem}{}", self.suffix)
    }
}

impl Signex {
    pub(in crate::app::view) fn view_rename_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_rename_dialog_body();
        self.wrap_dialog(
            crate::app::state::ModalId::RenameDialog,
            dialog,
            super::sizes::RENAME,
        )
    }

    pub(in crate::app::view) fn view_rename_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.rename_dialog else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let modal = crate::app::state::ModalId::RenameDialog;

        let title = t!(
            i18n,
            if st.is_project_rename {
                ids::DIALOG_RENAME_PROJECT_TITLE
            } else {
                ids::DIALOG_RENAME_FILE_TITLE
            }
        );
        let header = self.modal_header(
            modal_title_header(
                title.into_string(),
                Message::Rename(RenameMsg::Close),
                tokens,
            ),
            modal,
        );

        let current = if st.is_project_rename {
            st.target_path.file_stem()
        } else {
            st.target_path.file_name()
        }
        .and_then(|s| s.to_str())
        .unwrap_or("");
        let prompt = t!(
            i18n,
            if st.is_project_rename {
                ids::DIALOG_RENAME_PROJECT_PROMPT
            } else {
                ids::DIALOG_RENAME_FILE_PROMPT
            },
            "name" => current
        );
        let is_sheet = !st.is_project_rename
            && crate::app::file_extension::has_extension(&st.target_path, "snxsch");
        let icon = if is_sheet {
            DialogIcon::Sheet
        } else {
            DialogIcon::File
        };
        let ink3 = crate::styles::tokens::resolve(tokens).ink3;
        let path_line = text(UserText::new(st.target_path.display().to_string()))
            .size(TS.caption)
            .font(super::graphite::mono())
            .color(ink3)
            .boxed();

        let mut body = column![
            dialog_intro_with(icon, prompt.into_string(), Some(path_line), tokens),
            column![
                dialog_field_label(t!(i18n, ids::DIALOG_RENAME_NEW_NAME), tokens),
                name_field(st, i18n, tokens),
            ],
        ]
        .spacing(SP.l);
        for note in self.rename_info_notes(st, i18n) {
            body = body.push(dialog_callout(
                CalloutTone::Info,
                text(note).size(TS.body).boxed(),
                tokens,
            ));
        }
        if let Some(name) = st.name_clash.as_deref() {
            let taken = t!(i18n, ids::DIALOG_RENAME_NAME_TAKEN, "name" => name);
            body = body.push(dialog_inline_error(
                text(taken).size(TS.body).boxed(),
                tokens,
            ));
        }
        if let Some(ref err) = st.error {
            // raw: the Rename error is set by its handler (#2061: handler-worded state text).
            let err = text(raw(err.clone())).size(TS.body).boxed();
            body = body.push(dialog_inline_error(err, tokens));
        }

        // A name another known file has: Rename waits for a new one.
        let submit = st
            .name_clash
            .is_none()
            .then_some(Message::Rename(RenameMsg::Submit));
        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::Rename(RenameMsg::Close)),
                        tokens
                    ),
                    dialog_button(
                        t!(i18n, ids::DIALOG_RENAME_SUBMIT),
                        DialogButtonKind::Primary,
                        submit,
                        tokens
                    ),
                ]),
            ]
            .width(self.dialog_width(modal, super::sizes::RENAME)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    /// The info callouts under the field: a project rename whose folder keeps
    /// its name says so (mockup row 23); a sheet names the symbols that follow.
    pub(in crate::app::view) fn rename_info_notes(
        &self,
        st: &crate::app::RenameDialogState,
        i18n: &signex_i18n::I18n,
    ) -> Vec<signex_i18n::Localized> {
        let mut notes = Vec::new();
        let keeps_folder = st.is_project_rename
            && st
                .tree_path
                .first()
                .is_some_and(|&idx| self.project_rename_keeps_folder(idx, &st.target_path));
        if keeps_folder {
            notes.push(t!(i18n, ids::DIALOG_RENAME_PROJECT_FOLDER_KEPT));
        }
        if !st.placing_symbols.is_empty() {
            notes.push(placed_by_line(i18n, &st.placing_symbols));
        }
        notes
    }
}

/// The name field: mono, the stem editable, the extension a fixed suffix at
/// its right end. Every edit sends the whole name (stem plus suffix).
fn name_field<'a>(
    st: &'a crate::app::RenameDialogState,
    i18n: &signex_i18n::I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let parts = NameParts::of(&st.buffer, &st.target_path, st.is_project_rename);
    // A project name gets an example; a file name none (the mockup drops it).
    let placeholder: signex_widgets::Label<'a> = if st.is_project_rename {
        t!(i18n, ids::PLACEHOLDER_RENAME_PROJECT).into()
    } else {
        UserText::new(String::new()).into()
    };
    let suffix_room = parts.suffix.chars().count() as f32 * TS.body * MONO_ADVANCE_EM;
    let mono = super::graphite::mono();
    let compose = parts.clone();
    let input = text_input(placeholder, parts.stem.clone())
        .id(crate::app::contracts::RENAME_INPUT_ID.clone())
        .style(crate::panels::form_look::text_input_style)
        .font(mono)
        .size(TS.body)
        .padding(Padding {
            top: SP.s,
            bottom: SP.s,
            left: FIELD_PAD_X,
            right: FIELD_PAD_X + suffix_room,
        })
        .width(Length::Fill)
        .on_input(move |stem| Message::Rename(RenameMsg::BufferChanged(compose.compose(&stem))))
        .on_submit(Message::Rename(RenameMsg::Submit));
    if parts.suffix.is_empty() {
        return input.boxed();
    }
    let ink4 = crate::styles::tokens::resolve(tokens).ink4;
    let suffix = container(
        text(UserText::new(parts.suffix))
            .size(TS.body)
            .font(mono)
            .color(ink4),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
        right: FIELD_PAD_X,
        ..Padding::ZERO
    })
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Center);
    stack![input, suffix].boxed()
}

/// The note naming the sheet symbols a rename re-points.
fn placed_by_line(i18n: &signex_i18n::I18n, symbols: &[PlacingSymbol]) -> signex_i18n::Localized {
    let sheet_of = |s: &PlacingSymbol| {
        s.parent.file_name().map_or_else(
            || s.parent.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    let first = &symbols[0];
    let all = symbols
        .iter()
        .map(|s| format!("{} ({})", s.name, sheet_of(s)))
        .collect::<Vec<_>>()
        .join(", ");
    t!(
        i18n,
        ids::DIALOG_RENAME_PLACED_BY,
        "count" => symbols.len(),
        "symbol" => first.name.as_str(),
        "sheet" => sheet_of(first),
        "symbols" => all
    )
}

#[cfg(test)]
mod tests {
    use super::NameParts;
    use std::path::Path;

    /// A file's extension is a fixed suffix: the field shows the stem and an
    /// edit of it composes the whole name again.
    #[test]
    fn the_extension_is_a_fixed_suffix_and_edits_compose_the_whole_name() {
        let parts = NameParts::of("power.snxsch", Path::new("/p/power.snxsch"), false);
        assert_eq!(parts.stem, "power");
        assert_eq!(parts.suffix, ".snxsch");
        assert_eq!(parts.compose("power_stage"), "power_stage.snxsch");
        assert_eq!(parts.compose(""), ".snxsch");
        // A dotted stem keeps its dots; only the last extension is fixed.
        let dotted = NameParts::of("a.b.snxsch", Path::new("/p/a.b.snxsch"), false);
        assert_eq!(dotted.stem, "a.b");
    }

    /// A project rename edits the bare name; a buffer set without the
    /// extension is shown whole.
    #[test]
    fn a_project_or_a_bare_buffer_has_no_suffix_to_strip() {
        let project = NameParts::of("dcdc-buck", Path::new("/p/dcdc-buck.snxprj"), true);
        assert_eq!(project.stem, "dcdc-buck");
        assert_eq!(project.suffix, "");
        assert_eq!(project.compose("psu"), "psu");
        let bare = NameParts::of("p", Path::new("p.snxsch"), false);
        assert_eq!(bare.stem, "p");
        assert_eq!(bare.compose("p"), "p.snxsch");
        let none = NameParts::of("README", Path::new("/p/README"), false);
        assert_eq!((none.stem.as_str(), none.suffix.as_str()), ("README", ""));
    }

    fn rename_texts(dir: &str, is_project_rename: bool) -> Vec<String> {
        let mut app =
            crate::app::handlers::menu::export::tests::app_workspace(dir, &["top.snxsch"]);
        let target = std::path::PathBuf::from(dir).join(if is_project_rename {
            "ProjectA.snxprj"
        } else {
            "top.snxsch"
        });
        app.ui_state.rename_dialog = Some(Box::new(crate::app::RenameDialogState {
            project_id: None,
            target_path: target,
            tree_path: vec![0],
            buffer: "Renamed".to_string(),
            error: None,
            is_project_rename,
            placing_symbols: Vec::new(),
            name_clash: None,
        }));
        crate::layout_probe::lay_out_texts(
            app.view_rename_dialog_body(),
            iced::Size::new(1200.0, 900.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
    }

    /// Mockup row 23: Rename Project says, from the catalog, that the folder
    /// is not renamed when it keeps its name; a folder named after the
    /// project follows the rename, and a file rename never says it.
    #[test]
    fn rename_project_says_the_folder_is_not_renamed_only_when_it_stays() {
        const NOTE: &str = "The project folder is not renamed.";
        let kept = rename_texts("/w/a", true);
        assert!(kept.iter().any(|s| s == NOTE), "{kept:?}");

        let follows = rename_texts("/w/ProjectA", true);
        assert!(!follows.iter().any(|s| s == NOTE), "{follows:?}");

        let file = rename_texts("/w/a", false);
        assert!(!file.iter().any(|s| s == NOTE), "{file:?}");
    }
}
