//! Project & document modals — Rename, Enable Version Control, Grid
//! Properties, and the Custom Selection Filter.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Element, Length};
use signex_widgets::{raw, text};

use super::widgets::modal_title_header;
use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_footer, dialog_message};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    // ────────────────────────────────────────────────────────────────
    // Small Altium-style modals (Rename / Remove-from-Project / Close-
    // Tab Confirm). All three render through the same chrome used by
    // Annotate / ERC: `draggable_header` + the title header on the
    // right + `wrap_modal` for absolute positioning with drag-offset
    // persistence in `ui_state.modal_offsets`.
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
        let error_c = crate::styles::tokens::danger(tokens);

        let title = if st.is_project_rename {
            "Rename Project"
        } else {
            "Rename File"
        };
        let header_content: Element<'_, Message> =
            modal_title_header(title, Message::Rename(RenameMsg::Close), tokens);
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
        let placeholder = if st.is_project_rename {
            "new-project-name"
        } else {
            "new-name.snxsch"
        };
        let prompt = if st.is_project_rename {
            format!("Rename project \"{}\"", current_label)
        } else {
            format!("Rename \"{}\"", current_label)
        };

        let mut body: iced::widget::Column<Element<'_, Message>> = column![
            dialog_message(prompt, tokens),
            text_input(raw(placeholder), &st.buffer)
                .on_input(|s| Message::Rename(RenameMsg::BufferChanged(s)))
                .on_submit(Message::Rename(RenameMsg::Submit))
                .size(TS.label)
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(SP.m);

        if let Some(ref err) = st.error {
            body = body.push(text(raw(err.clone())).size(TS.body).color(error_c).boxed());
        }

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        "Cancel",
                        DialogButtonKind::Secondary,
                        Some(Message::Rename(RenameMsg::Close)),
                        tokens
                    ),
                    dialog_button(
                        "Rename",
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
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let header_content: Element<'_, Message> = modal_title_header(
            "Enable Version Control",
            Message::EnableVersionControl(EnableVersionControlMsg::Close),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::EnableVersionControl,
        );

        let scope = st.scope;
        let intro: Element<'_, Message> = dialog_message(st.intro_text.as_str(), tokens);
        let summary_label = match scope {
            crate::app::VersionControlScope::Project => "Project",
            crate::app::VersionControlScope::Library => "Library",
        };
        let summary = text(raw(format!("{summary_label}: {}", st.project_name)))
            .size(TS.label)
            .color(text_c);

        // Per-row track toggles. Surfaced as a column (no scrollable
        // — typical project / library layouts have <20 entries) so
        // the modal sizing stays predictable.
        let items_text_c = text_c;
        let items_text_muted = text_muted;
        let mut items_col: iced::widget::Column<Element<'_, Message>> = column![].spacing(SP.s);
        if !st.items.is_empty() {
            items_col = items_col.push(
                text(raw("Track in repository"))
                    .size(TS.body)
                    .color(items_text_muted)
                    .boxed(),
            );
            for (idx, item) in st.items.iter().enumerate() {
                let cb: Element<'_, Message> = checkbox(item.tracked)
                    .size(TS.title)
                    .on_toggle(move |_| {
                        Message::EnableVersionControl(EnableVersionControlMsg::ToggleItem(idx))
                    })
                    .boxed();
                let row_widget = row![
                    cb,
                    Space::new().width(8),
                    text(raw(item.relative.clone()))
                        .size(TS.body)
                        .color(items_text_c),
                    Space::new().width(Length::Fill),
                    text(raw(item.label.clone()))
                        .size(TS.caption)
                        .color(items_text_muted),
                ]
                .align_y(iced::Alignment::Center);
                items_col = items_col.push(row_widget.boxed());
            }
        }

        let lfs_check: Element<'_, Message> = checkbox(st.use_lfs)
            .size(TS.heading)
            .on_toggle(|_| Message::EnableVersionControl(EnableVersionControlMsg::ToggleLfs))
            .boxed();
        let lfs_row = row![
            lfs_check,
            Space::new().width(8),
            column![
                text(raw("Track binary 3D models via Git LFS"))
                    .size(TS.label)
                    .color(text_c),
                text(raw("Routes `*.step / *.stp / *.wrl / *.iges` through Git LFS. Requires `git lfs` locally."))
                    .size(TS.caption)
                    .color(text_muted),
            ]
            .spacing(SP.xs),
        ]
        .align_y(iced::Alignment::Start);

        let mut body_col: iced::widget::Column<Element<'_, Message>> =
            column![summary, intro].spacing(10);
        if !st.items.is_empty() {
            body_col = body_col.push(Space::new().height(2).boxed());
            body_col = body_col.push(items_col.boxed());
        }
        body_col = body_col.push(Space::new().height(6).boxed());
        body_col = body_col.push(lfs_row.boxed());
        if st.scanning {
            body_col = body_col.push(
                text(raw("Reading the project folder…"))
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            );
        }
        if let Some(err) = st.error.as_ref() {
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
                        "Cancel",
                        DialogButtonKind::Secondary,
                        Some(Message::EnableVersionControl(
                            EnableVersionControlMsg::Close
                        )),
                        tokens
                    ),
                    dialog_button(
                        if st.working { "Enabling…" } else { "Enable" },
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

    /// v0.18.11 — Cartesian Grid Editor modal (Ctrl+G in a footprint
    /// editor). Mirrors Altium's "Cartesian Grid Editor [mm]" with a
    /// stripped-down field set: Step X / Step Y + link toggle, plus
    /// OK / Cancel. Display style + multiplier + per-grid-color land
    /// in v0.18.11.x as the underlying canvas/grid system grows them.
    pub(in crate::app::view) fn view_grid_properties_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_grid_properties_dialog_body();
        self.wrap_dialog(
            crate::app::state::ModalId::GridProperties,
            dialog,
            super::sizes::GRID_PROPERTIES,
        )
    }

    pub(in crate::app::view) fn view_grid_properties_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.grid_properties else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = modal_title_header(
            "Cartesian Grid Editor [mm]",
            Message::GridProperties(GridPropertiesMsg::Close),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::GridProperties,
        );

        fn mk_input<'a>(
            placeholder: &'static str,
            value: &'a str,
            on_input: fn(String) -> Message,
        ) -> Element<'a, Message> {
            use signex_widgets::{raw, text_input};
            text_input(raw(placeholder), value)
                .size(TS.label)
                .padding(6)
                .width(Length::Fixed(140.0))
                .on_input(on_input)
                .on_submit(Message::GridProperties(GridPropertiesMsg::Apply))
                .boxed()
        }

        // Display style + multiplier rows.
        use crate::library::editor::footprint::state::GridDisplay as Gd;
        let mk_display_row =
            |label: &'static str, current: Gd, setter: fn(Gd) -> Message| -> Element<'_, Message> {
                let seg = move |label: &'static str, target: Gd, active: bool| {
                    grid_segment(label, active, setter(target), text_c, border_c)
                };
                row![
                    container(text(raw(label)).size(TS.body).color(text_muted))
                        .width(Length::Fixed(80.0)),
                    seg("Lines", Gd::Lines, current == Gd::Lines),
                    seg("Dots", Gd::Dots, current == Gd::Dots),
                    seg("Hidden", Gd::Hidden, current == Gd::Hidden),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed()
            };
        let mk_mult_row = |current: u32| -> Element<'_, Message> {
            let seg = move |label: &'static str, target: u32, active: bool| {
                let msg = Message::GridProperties(GridPropertiesMsg::SetMultiplier(target));
                grid_segment(label, active, msg, text_c, border_c)
            };
            row![
                container(text(raw("Multiplier")).size(TS.body).color(text_muted))
                    .width(Length::Fixed(80.0)),
                seg("1×", 1, current == 1),
                seg("2×", 2, current == 2),
                seg("5×", 5, current == 5),
                seg("10×", 10, current == 10),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed()
        };

        let body = column![
            row![
                container(text(raw("Step")).size(TS.body).color(text_muted))
                    .width(Length::Fixed(80.0)),
                mk_input("0.127", &st.step_mm, |v| Message::GridProperties(
                    GridPropertiesMsg::SetStep(v)
                )),
                text(raw("mm, both axes"))
                    .size(TS.caption)
                    .color(text_muted),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
            mk_display_row("Fine", st.fine_display, |d| {
                Message::GridProperties(GridPropertiesMsg::SetFineDisplay(d))
            }),
            mk_display_row("Coarse", st.coarse_display, |d| {
                Message::GridProperties(GridPropertiesMsg::SetCoarseDisplay(d))
            }),
            mk_mult_row(st.multiplier),
            text(raw(
                "The step applies to both axes (one grid step per footprint editor). \
                 Display: Lines / Dots / Hidden. \
                 Multiplier sets the coarse-grid stride (5× / 10× are typical Altium defaults)."
            ))
            .size(TS.caption)
            .color(text_muted),
        ]
        .spacing(10);

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        "Cancel",
                        DialogButtonKind::Secondary,
                        Some(Message::GridProperties(GridPropertiesMsg::Close)),
                        tokens
                    ),
                    dialog_button(
                        "Apply",
                        DialogButtonKind::Primary,
                        Some(Message::GridProperties(GridPropertiesMsg::Apply)),
                        tokens
                    ),
                ]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::GridProperties,
                super::sizes::GRID_PROPERTIES,
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
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let header_content: Element<'_, Message> = modal_title_header(
            "Selection Filter — Customize",
            Message::SelectionFilter(SelectionFilterMsg::CloseCustom),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::SelectionFilterCustom,
        );

        let mk_row = |label: &'static str, kind: K, on: bool| -> Element<'_, Message> {
            let glyph = if on { "[x]" } else { "[ ]" };
            signex_widgets::button(
                row![
                    text(raw(format!("{glyph}  {label}")))
                        .size(TS.body)
                        .color(text_c)
                        .width(Length::Fill),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .on_press(Message::SelectionFilter(
                SelectionFilterMsg::ToggleCustomKind(kind),
            ))
            .style(move |_: &iced::Theme, status| iced::widget::button::Style {
                background: match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                    )),
                    _ => Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                },
                border: iced::Border {
                    width: 0.0,
                    radius: 0.0.into(),
                    color: iced::Color::TRANSPARENT,
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            })
            .boxed()
        };

        let body = column![
            text(raw("Toggle which kinds the canvas hit-test will accept. \
                 Pads is the only kind functionally wired today; the \
                 rest store flags for forward compatibility."))
            .size(TS.caption)
            .color(text_muted),
            mk_row("Pads", K::Pads, st.pads),
            mk_row("Tracks", K::Tracks, st.tracks),
            mk_row("Arcs", K::Arcs, st.arcs),
            mk_row("Pours", K::Pours, st.pours),
            mk_row("3D Bodies", K::Bodies3d, st.bodies_3d),
            mk_row("Keepouts", K::Keepouts, st.keepouts),
            mk_row("Cutouts", K::Cutouts, st.cutouts),
            mk_row("Texts", K::Texts, st.texts),
        ]
        .spacing(SP.s);

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        "Cancel",
                        DialogButtonKind::Secondary,
                        Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom)),
                        tokens
                    ),
                    dialog_button(
                        "Apply",
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

/// One segment button of the Grid Properties display and multiplier rows:
/// tinted while `active`.
fn grid_segment<'a>(
    label: &'static str,
    active: bool,
    on_press: Message,
    text_c: iced::Color,
    border_c: iced::Color,
) -> Element<'a, Message> {
    let bg = if active {
        iced::Color::from_rgba(0.40, 0.70, 1.00, 0.20)
    } else {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    signex_widgets::button(text(raw(label)).size(TS.caption).color(text_c))
        .padding([3, 10])
        .on_press(on_press)
        .style(move |_: &iced::Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
                ..iced::Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .boxed()
}
