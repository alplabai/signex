//! Project & document modals — Rename, Enable Version Control, Grid
//! Properties, and the Custom Selection Filter.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container, row, text};
use iced::{Color, Element, Length};

use super::widgets::{close_x_button, primary_button, secondary_button, wrap_modal};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    // ────────────────────────────────────────────────────────────────
    // Small Altium-style modals (Rename / Remove-from-Project / Close-
    // Tab Confirm). All three render through the same chrome used by
    // Annotate / ERC: `draggable_header` + `close_x_button` on the
    // right + `wrap_modal` for absolute positioning with drag-offset
    // persistence in `ui_state.modal_offsets`.
    // ────────────────────────────────────────────────────────────────

    pub(in crate::app::view) fn view_rename_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_rename_dialog_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::RenameDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (420.0, 200.0))
    }

    pub(in crate::app::view) fn view_rename_dialog_body(&self) -> Element<'_, Message> {
        use iced::widget::text_input;

        let Some(ref st) = self.ui_state.rename_dialog else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let error_c = Color::from_rgb(0.90, 0.35, 0.30);

        let theme_id = self.ui_state.theme_id;
        let title = if st.is_project_rename {
            "Rename Project"
        } else {
            "Rename File"
        };
        let header_content: Element<'_, Message> = container(
            row![
                text(title).size(MODAL_HEADER_TITLE_SIZE).color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::Rename(RenameMsg::Close), theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
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

        let mut body: iced::widget::Column<'_, Message> = column![
            text(prompt).size(TS.body).color(text_muted),
            text_input(placeholder, &st.buffer)
                .on_input(|s| Message::Rename(RenameMsg::BufferChanged(s)))
                .on_submit(Message::Rename(RenameMsg::Submit))
                .size(TS.label)
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(SP.m);

        if let Some(ref err) = st.error {
            body = body.push(text(err.clone()).size(TS.body).color(error_c));
        }

        let dialog = container(
            column![
                header,
                container(body).padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::Rename(RenameMsg::Close),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Rename",
                            Some(Message::Rename(RenameMsg::Submit)),
                            border_c
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::RenameDialog, 420.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(in crate::app::view) fn view_enable_version_control_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_enable_version_control_dialog_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::EnableVersionControl)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (520.0, 320.0))
    }

    pub(in crate::app::view) fn view_enable_version_control_dialog_body(
        &self,
    ) -> Element<'_, Message> {
        use iced::widget::checkbox;
        let Some(ref st) = self.ui_state.enable_version_control else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let theme_id = self.ui_state.theme_id;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = container(
            row![
                text("Enable Version Control")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::EnableVersionControl(EnableVersionControlMsg::Close),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::EnableVersionControl,
        );

        let scope = st.scope;
        let intro = text(st.intro_text.as_str()).size(TS.body).color(text_muted);
        let summary_label = match scope {
            crate::app::VersionControlScope::Project => "Project",
            crate::app::VersionControlScope::Library => "Library",
        };
        let summary = text(format!("{summary_label}: {}", st.project_name))
            .size(TS.label)
            .color(text_c);

        // Per-row track toggles. Surfaced as a column (no scrollable
        // — typical project / library layouts have <20 entries) so
        // the modal sizing stays predictable.
        let items_text_c = text_c;
        let items_text_muted = text_muted;
        let mut items_col = column![].spacing(SP.s);
        if !st.items.is_empty() {
            items_col = items_col.push(
                text("Track in repository")
                    .size(TS.body)
                    .color(items_text_muted),
            );
            for (idx, item) in st.items.iter().enumerate() {
                let cb: Element<'_, Message> = checkbox(item.tracked)
                    .size(TS.title)
                    .on_toggle(move |_| {
                        Message::EnableVersionControl(EnableVersionControlMsg::ToggleItem(idx))
                    })
                    .into();
                let row_widget = row![
                    cb,
                    Space::new().width(8),
                    text(item.relative.clone())
                        .size(TS.body)
                        .color(items_text_c),
                    Space::new().width(Length::Fill),
                    text(item.label.clone())
                        .size(TS.caption)
                        .color(items_text_muted),
                ]
                .align_y(iced::Alignment::Center);
                items_col = items_col.push(row_widget);
            }
        }

        let lfs_check: Element<'_, Message> = checkbox(st.use_lfs)
            .size(TS.heading)
            .on_toggle(|_| Message::EnableVersionControl(EnableVersionControlMsg::ToggleLfs))
            .into();
        let lfs_row = row![
            lfs_check,
            Space::new().width(8),
            column![
                text("Track binary 3D models via Git LFS")
                    .size(TS.label)
                    .color(text_c),
                text("Routes `*.step / *.stp / *.wrl / *.iges` through Git LFS. Requires `git lfs` locally.")
                    .size(TS.caption)
                    .color(text_muted),
            ]
            .spacing(SP.xs),
        ]
        .align_y(iced::Alignment::Start);

        let mut body_col = column![summary, intro].spacing(10);
        if !st.items.is_empty() {
            body_col = body_col.push(Space::new().height(2));
            body_col = body_col.push(items_col);
        }
        body_col = body_col.push(Space::new().height(6));
        body_col = body_col.push(lfs_row);
        if let Some(err) = st.error.as_ref() {
            body_col = body_col.push(
                text(err.clone())
                    .size(TS.body)
                    .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
            );
        }

        let dialog = container(
            column![
                header,
                container(body_col).padding([14.0, SP.xl]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::EnableVersionControl(EnableVersionControlMsg::Close),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            if st.working { "Enabling…" } else { "Enable" },
                            (!st.working && !st.scanning).then_some(Message::EnableVersionControl(
                                EnableVersionControlMsg::Confirm
                            )),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::EnableVersionControl,
                520.0,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    /// v0.18.11 — Cartesian Grid Editor modal (Ctrl+G in a footprint
    /// editor). Mirrors Altium's "Cartesian Grid Editor [mm]" with a
    /// stripped-down field set: Step X / Step Y + link toggle, plus
    /// OK / Cancel. Display style + multiplier + per-grid-color land
    /// in v0.18.11.x as the underlying canvas/grid system grows them.
    pub(in crate::app::view) fn view_grid_properties_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_grid_properties_dialog_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::GridProperties)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (480.0, 280.0))
    }

    pub(in crate::app::view) fn view_grid_properties_dialog_body(&self) -> Element<'_, Message> {
        use iced::widget::text_input;

        let Some(ref st) = self.ui_state.grid_properties else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let theme_id = self.ui_state.theme_id;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = container(
            row![
                text("Cartesian Grid Editor [mm]")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::GridProperties(GridPropertiesMsg::Close),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::GridProperties,
        );

        let mk_input = |placeholder: &'static str,
                        value: &str,
                        on_input: fn(String) -> Message|
         -> Element<'_, Message> {
            text_input(placeholder, value)
                .size(TS.label)
                .padding(6)
                .width(Length::Fixed(140.0))
                .on_input(on_input)
                .on_submit(Message::GridProperties(GridPropertiesMsg::Apply))
                .into()
        };

        // Display style + multiplier rows.
        use crate::library::editor::footprint::state::GridDisplay as Gd;
        let mk_display_row =
            |label: &'static str, current: Gd, setter: fn(Gd) -> Message| -> Element<'_, Message> {
                let seg =
                    move |label: &'static str, target: Gd, active: bool| -> Element<'_, Message> {
                        let bg = if active {
                            iced::Color::from_rgba(0.40, 0.70, 1.00, 0.20)
                        } else {
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                        };
                        iced::widget::button(text(label).size(TS.caption).color(text_c))
                            .padding([3, 10])
                            .on_press(setter(target))
                            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                                background: Some(iced::Background::Color(bg)),
                                border: iced::Border {
                                    width: 1.0,
                                    radius: 3.0.into(),
                                    color: border_c,
                                },
                                ..iced::widget::button::Style::default()
                            })
                            .into()
                    };
                row![
                    container(text(label).size(TS.body).color(text_muted))
                        .width(Length::Fixed(80.0)),
                    seg("Lines", Gd::Lines, current == Gd::Lines),
                    seg("Dots", Gd::Dots, current == Gd::Dots),
                    seg("Hidden", Gd::Hidden, current == Gd::Hidden),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .into()
            };
        let mk_mult_row = |current: u32| -> Element<'_, Message> {
            let seg =
                move |label: &'static str, target: u32, active: bool| -> Element<'_, Message> {
                    let bg = if active {
                        iced::Color::from_rgba(0.40, 0.70, 1.00, 0.20)
                    } else {
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                    };
                    iced::widget::button(text(label).size(TS.caption).color(text_c))
                        .padding([3, 10])
                        .on_press(Message::GridProperties(GridPropertiesMsg::SetMultiplier(
                            target,
                        )))
                        .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                            background: Some(iced::Background::Color(bg)),
                            border: iced::Border {
                                width: 1.0,
                                radius: 3.0.into(),
                                color: border_c,
                            },
                            ..iced::widget::button::Style::default()
                        })
                        .into()
                };
            row![
                container(text("Multiplier").size(TS.body).color(text_muted))
                    .width(Length::Fixed(80.0)),
                seg("1×", 1, current == 1),
                seg("2×", 2, current == 2),
                seg("5×", 5, current == 5),
                seg("10×", 10, current == 10),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .into()
        };

        let body = column![
            row![
                container(text("Step").size(TS.body).color(text_muted)).width(Length::Fixed(80.0)),
                mk_input("0.127", &st.step_mm, |v| Message::GridProperties(
                    GridPropertiesMsg::SetStep(v)
                )),
                text("mm, both axes").size(TS.caption).color(text_muted),
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
            text(
                "The step applies to both axes (one grid step per footprint editor). \
                 Display: Lines / Dots / Hidden. \
                 Multiplier sets the coarse-grid stride (5× / 10× are typical Altium defaults)."
            )
            .size(TS.caption)
            .color(text_muted),
        ]
        .spacing(10);

        let dialog = container(
            column![
                header,
                container(body).padding([14.0, SP.xl]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::GridProperties(GridPropertiesMsg::Close),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Apply",
                            Some(Message::GridProperties(GridPropertiesMsg::Apply)),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::GridProperties, 480.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    /// v0.18.14.1 — Custom Selection Filter modal. 8 rows of
    /// per-kind checkboxes (Pads / Tracks / Arcs / Pours / 3D Bodies
    /// / Keepouts / Cutouts / Texts) + Apply / Cancel. Apply writes
    /// the draft into `editor.state.selection_filter`.
    pub(in crate::app::view) fn view_selection_filter_custom_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_selection_filter_custom_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::SelectionFilterCustom)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (440.0, 380.0))
    }

    pub(in crate::app::view) fn view_selection_filter_custom_body(&self) -> Element<'_, Message> {
        use crate::library::editor::footprint::state::SelectionFilterKind as K;

        let Some(ref st) = self.ui_state.selection_filter_custom else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let theme_id = self.ui_state.theme_id;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = container(
            row![
                text("Selection Filter — Customize")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::SelectionFilter(SelectionFilterMsg::CloseCustom),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::SelectionFilterCustom,
        );

        let mk_row = |label: &'static str, kind: K, on: bool| -> Element<'_, Message> {
            let glyph = if on { "[x]" } else { "[ ]" };
            iced::widget::button(
                row![
                    text(format!("{glyph}  {label}"))
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
                },
                ..iced::widget::button::Style::default()
            })
            .into()
        };

        let body = column![
            text(
                "Toggle which kinds the canvas hit-test will accept. \
                 Pads is the only kind functionally wired today; the \
                 rest store flags for forward compatibility."
            )
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
                container(body).padding([SP.l, 14.0]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::SelectionFilter(SelectionFilterMsg::CloseCustom),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Apply",
                            Some(Message::SelectionFilter(SelectionFilterMsg::ApplyCustom)),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::SelectionFilterCustom,
                440.0,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}
