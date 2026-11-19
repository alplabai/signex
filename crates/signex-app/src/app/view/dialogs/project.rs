//! Project & document modals — Rename, Enable Version Control and the
//! Custom Selection Filter. Grid Properties lives in `grid_properties/`.
//!
//! Methods of the same `Signex` view impl, split across sibling files
//! (ADR-0001, issue #164).

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Element, Length};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::{raw, text};

use super::widgets::modal_title_header;
use crate::styles::{
    DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_footer, dialog_intro,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    // ────────────────────────────────────────────────────────────────
    // Small modals (Rename / Remove-from-Project / Close-Tab Confirm).
    // All three render through the same chrome used by Annotate / ERC:
    // `draggable_header` + the title header on the right + `wrap_modal`
    // for absolute positioning with drag-offset persistence in
    // `ui_state.modal_offsets`.
    // ────────────────────────────────────────────────────────────────

    pub(in crate::app::view) fn view_rename_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_rename_dialog_body();
        self.wrap_dialog(
            crate::app::state::ModalId::RenameDialog,
            dialog,
            super::sizes::RENAME,
        )
    }

    pub(in crate::app::view) fn view_rename_dialog_body(&self) -> Element<'_, Message> {
        use signex_widgets::{raw, text_input};

        let Some(ref st) = self.ui_state.rename_dialog else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let error_c = crate::styles::tokens::danger(tokens);

        let title = t!(
            i18n,
            if st.is_project_rename {
                ids::DIALOG_RENAME_PROJECT_TITLE
            } else {
                ids::DIALOG_RENAME_FILE_TITLE
            }
        );
        let header_content: Element<'_, Message> = modal_title_header(
            title.into_string(),
            Message::Rename(RenameMsg::Close),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::RenameDialog,
        );

        let current_label = if st.is_project_rename {
            st.target_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        } else {
            st.target_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        };
        let placeholder = t!(
            i18n,
            if st.is_project_rename {
                ids::PLACEHOLDER_RENAME_PROJECT
            } else {
                ids::PLACEHOLDER_RENAME_FILE
            }
        );
        let prompt = t!(
            i18n,
            if st.is_project_rename {
                ids::DIALOG_RENAME_PROJECT_PROMPT
            } else {
                ids::DIALOG_RENAME_FILE_PROMPT
            },
            "name" => current_label.as_str()
        );

        let mut body: iced::widget::Column<Element<'_, Message>> = column![
            dialog_intro(DialogIcon::File, prompt.into_string(), None, tokens),
            text_input(placeholder, &st.buffer)
                .style(crate::panels::form_look::text_input_style)
                .on_input(|s| Message::Rename(RenameMsg::BufferChanged(s)))
                .on_submit(Message::Rename(RenameMsg::Submit))
                .size(TS.label)
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(SP.m);

        if let Some(ref err) = st.error {
            // raw: the Rename error is set by its handler (#2061: handler-worded state text).
            body = body.push(text(raw(err.clone())).size(TS.body).color(error_c).boxed());
        }

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
                        Some(Message::Rename(RenameMsg::Submit)),
                        tokens
                    ),
                ]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::RenameDialog,
                super::sizes::RENAME,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    pub(in crate::app::view) fn view_enable_version_control_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_enable_version_control_dialog_body();
        self.wrap_dialog(
            crate::app::state::ModalId::EnableVersionControl,
            dialog,
            super::sizes::ENABLE_VERSION_CONTROL,
        )
    }

    pub(in crate::app::view) fn view_enable_version_control_dialog_body(
        &self,
    ) -> Element<'_, Message> {
        use signex_widgets::checkbox;
        let Some(ref st) = self.ui_state.enable_version_control else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, ids::DIALOG_TITLE_ENABLE_VERSION_CONTROL).into_string(),
            Message::EnableVersionControl(EnableVersionControlMsg::Close),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::EnableVersionControl,
        );

        let scope = st.scope;
        let summary_id = match scope {
            crate::app::VersionControlScope::Project => ids::DIALOG_VERSION_CONTROL_PROJECT,
            crate::app::VersionControlScope::Library => ids::DIALOG_VERSION_CONTROL_LIBRARY,
        };

        // Per-row track toggles. Surfaced as a column (no scrollable
        // — typical project / library layouts have <20 entries) so
        // the modal sizing stays predictable.
        let items_text_c = text_c;
        let items_text_muted = text_muted;
        let mut items_col: iced::widget::Column<Element<'_, Message>> = column![].spacing(SP.s);
        if !st.items.is_empty() {
            items_col = items_col.push(
                text(t!(i18n, ids::DIALOG_VERSION_CONTROL_TRACK_HEADING))
                    .size(TS.body)
                    .color(items_text_muted)
                    .boxed(),
            );
            for (idx, item) in st.items.iter().enumerate() {
                let cb: Element<'_, Message> = checkbox(item.tracked)
                    .style(crate::panels::form_look::checkbox_style)
                    .size(TS.title)
                    .on_toggle(move |_| {
                        Message::EnableVersionControl(EnableVersionControlMsg::ToggleItem(idx))
                    })
                    .boxed();
                let row_widget = row![
                    cb,
                    Space::new().width(8),
                    text(signex_i18n::UserText::new(item.relative.clone()))
                        .size(TS.body)
                        .color(items_text_c),
                    Space::new().width(Length::Fill),
                    // raw: the item's kind is set by the folder scan (#2061: handler-worded state text).
                    text(raw(item.label.clone()))
                        .size(TS.caption)
                        .color(items_text_muted),
                ]
                .align_y(iced::Alignment::Center);
                items_col = items_col.push(row_widget.boxed());
            }
        }

        let lfs_check: Element<'_, Message> = checkbox(st.use_lfs)
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.heading)
            .on_toggle(|_| Message::EnableVersionControl(EnableVersionControlMsg::ToggleLfs))
            .boxed();
        let lfs_row = row![
            lfs_check,
            Space::new().width(8),
            column![
                text(t!(i18n, ids::DIALOG_VERSION_CONTROL_LFS))
                    .size(TS.label)
                    .color(text_c),
                text(t!(i18n, ids::DIALOG_VERSION_CONTROL_LFS_HELP))
                    .size(TS.caption)
                    .color(text_muted),
            ]
            .spacing(SP.xs),
        ]
        .align_y(iced::Alignment::Start);

        // The shell's intro opens the body: what gets a repository, then why.
        let mut body_col: iced::widget::Column<Element<'_, Message>> = column![dialog_intro(
            DialogIcon::File,
            t!(i18n, summary_id, "name" => st.project_name.as_str()).into_string(),
            Some(st.intro_text.clone()),
            tokens,
        )]
        .spacing(10);
        if !st.items.is_empty() {
            body_col = body_col.push(Space::new().height(2).boxed());
            body_col = body_col.push(items_col.boxed());
        }
        body_col = body_col.push(Space::new().height(6).boxed());
        body_col = body_col.push(lfs_row.boxed());
        if st.scanning {
            body_col = body_col.push(
                text(t!(i18n, ids::DIALOG_VERSION_CONTROL_SCANNING))
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            );
        }
        if let Some(err) = st.error.as_ref() {
            // raw: the setup error is worded by the version-control job (#2061).
            body_col = body_col.push(
                text(raw(err.clone()))
                    .size(TS.body)
                    .color(crate::styles::tokens::danger(tokens))
                    .boxed(),
            );
        }

        let dialog = container(
            column![
                header,
                dialog_body(body_col),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::EnableVersionControl(
                            EnableVersionControlMsg::Close
                        )),
                        tokens
                    ),
                    dialog_button(
                        t!(
                            i18n,
                            if st.working {
                                ids::DIALOG_VERSION_CONTROL_ENABLING
                            } else {
                                ids::DIALOG_VERSION_CONTROL_ENABLE
                            }
                        ),
                        DialogButtonKind::Primary,
                        (!st.working && !st.scanning).then_some(Message::EnableVersionControl(
                            EnableVersionControlMsg::Confirm
                        )),
                        tokens
                    ),
                ]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::EnableVersionControl,
                super::sizes::ENABLE_VERSION_CONTROL,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    /// v0.18.14.1 — Custom Selection Filter modal. 8 rows of
    /// per-kind checkboxes (Pads / Tracks / Arcs / Pours / 3D Bodies
    /// / Keepouts / Cutouts / Texts) + Apply / Cancel. Apply writes
    /// the draft into `editor.state.selection_filter`.
    pub(in crate::app::view) fn view_selection_filter_custom_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_selection_filter_custom_body();
        self.wrap_dialog(
            crate::app::state::ModalId::SelectionFilterCustom,
            dialog,
            super::sizes::SELECTION_FILTER_CUSTOM,
        )
    }

    pub(in crate::app::view) fn view_selection_filter_custom_body(&self) -> Element<'_, Message> {
        use crate::library::editor::footprint::state::SelectionFilterKind as K;

        let Some(ref st) = self.ui_state.selection_filter_custom else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, ids::DIALOG_SELECTION_FILTER_TITLE).into_string(),
            Message::SelectionFilter(SelectionFilterMsg::CloseCustom),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::SelectionFilterCustom,
        );

        // A real checkbox per kind; the whole row toggles it too.
        let mk_row = |label: MessageId, kind: K, on: bool| -> Element<'_, Message> {
            let toggle = Message::SelectionFilter(SelectionFilterMsg::ToggleCustomKind(kind));
            let check = signex_widgets::checkbox(on)
                .style(crate::panels::form_look::checkbox_style)
                .size(TS.title)
                .on_toggle({
                    let toggle = toggle.clone();
                    move |_| toggle.clone()
                });
            let name = text(t!(i18n, label))
                .size(TS.body)
                .color(text_c)
                .width(Length::Fill);
            iced::widget::mouse_area(
                container(
                    row![check, Space::new().width(SP.m), name].align_y(iced::Alignment::Center),
                )
                .padding([SP.s, SP.m])
                .width(Length::Fill),
            )
            .on_press(toggle)
            .boxed()
        };

        let body = column![
            text(t!(i18n, ids::DIALOG_SELECTION_FILTER_HELP))
                .size(TS.caption)
                .color(text_muted),
            mk_row(ids::DIALOG_SELECTION_FILTER_PADS, K::Pads, st.pads),
            mk_row(ids::DIALOG_SELECTION_FILTER_TRACKS, K::Tracks, st.tracks),
            mk_row(ids::DIALOG_SELECTION_FILTER_ARCS, K::Arcs, st.arcs),
            mk_row(ids::DIALOG_SELECTION_FILTER_POURS, K::Pours, st.pours),
            mk_row(
                ids::DIALOG_SELECTION_FILTER_BODIES_3D,
                K::Bodies3d,
                st.bodies_3d
            ),
            mk_row(
                ids::DIALOG_SELECTION_FILTER_KEEPOUTS,
                K::Keepouts,
                st.keepouts
            ),
            mk_row(ids::DIALOG_SELECTION_FILTER_CUTOUTS, K::Cutouts, st.cutouts),
            mk_row(ids::DIALOG_SELECTION_FILTER_TEXTS, K::Texts, st.texts),
        ]
        .spacing(SP.s);
        let body = with_graphite_intro(
            DialogIcon::Settings,
            t!(i18n, ids::DIALOG_SELECTION_FILTER_INTRO).into_string(),
            body,
            SP.s,
            tokens,
        );

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom)),
                        tokens
                    ),
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_APPLY),
                        DialogButtonKind::Primary,
                        Some(Message::SelectionFilter(SelectionFilterMsg::ApplyCustom)),
                        tokens
                    ),
                ]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::SelectionFilterCustom,
                super::sizes::SELECTION_FILTER_CUSTOM,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

/// `body` under the shell's title-only intro.
fn with_graphite_intro<'a>(
    icon: DialogIcon,
    title: String,
    body: iced::widget::Column<Element<'a, Message>>,
    gap: f32,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    column![dialog_intro(icon, title, None, tokens), body]
        .spacing(gap)
        .boxed()
}
