//! 3D model tab — Phase 2 minimum-viable upload + offset / rotation
//! editor. Real wgpu rendering of the loaded model lands later
//! (`TODO(v0.9-phase-3)`).
//!
//! Layout:
//!  * Upload row — file-picker button gated to STEP / WRL / GLB / glTF.
//!  * Model card — filename, size, storage path, `Replace` / `Remove`.
//!  * Offset + rotation grid — 3-axis editable spinboxes wired to
//!    `PcbSide.model_3d.offset` / `.rotation`.
//!  * Placeholder "Pad-to-pin alignment preview" panel — wired to a
//!    real wgpu surface in Phase 3.

pub mod state;

use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use iced_aw::NumberInput;
use signex_library::ModelRef;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;

pub use state::{Model3dUploadInfo, hash_bytes_hex, is_supported_extension};

/// Render the 3D tab for `editor`.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    let upload_row = view_upload_row(editor, tokens, window_id);
    let model_card = view_model_card(editor, tokens, window_id);
    let transform_grid = view_transform_grid(editor, tokens, window_id);
    let preview_card = view_preview_placeholder(tokens);

    let body = column![
        upload_row,
        Space::new().height(12),
        model_card,
        Space::new().height(12),
        transform_grid,
        Space::new().height(12),
        preview_card,
        Space::new().height(8),
        text(
            "Real-time wgpu render of the uploaded model lands in Phase 3 \
             (TODO(v0.9-phase-3))."
        )
        .size(10)
        .color(muted),
    ]
    .spacing(0)
    .width(Length::Fill);

    container(body)
        .style(crate::styles::modal_card(tokens))
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ─────────────────────────────────────────────────────────────────────
// Sections
// ─────────────────────────────────────────────────────────────────────

fn view_upload_row<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let upload_label = if editor.draft.pcb.model_3d.is_some() {
        "Replace STEP / WRL / GLB"
    } else {
        "Upload STEP / WRL / GLB"
    };
    let upload_btn = button(
        container(text(upload_label).size(11).color(iced::Color::WHITE)).padding([4, 14]),
    )
    .on_press(LibraryMessage::EditorEvent {
        window_id,
        msg: EditorMsg::Model3dUploadDialog,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            0.00, 0.47, 0.84,
        ))),
        text_color: iced::Color::WHITE,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });

    let mut row = row![
        text("3D Model").size(13).color(text_c),
        Space::new().width(Length::Fill),
        upload_btn,
    ]
    .align_y(iced::Alignment::Center);

    if editor.draft.pcb.model_3d.is_some() {
        let remove_btn = button(
            container(text("Remove").size(11).color(text_c)).padding([4, 12]),
        )
        .on_press(LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::Model3dRemove,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: theme_ext::border_color(tokens),
            },
            ..iced::widget::button::Style::default()
        });
        row = row.push(Space::new().width(8));
        row = row.push(remove_btn);
    }

    let formats_hint = text("Accepted: *.step, *.stp, *.wrl, *.glb, *.gltf")
        .size(10)
        .color(muted);

    column![row, Space::new().height(4), formats_hint]
        .spacing(0)
        .into()
}

fn view_model_card<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    _window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let Some(model) = editor.draft.pcb.model_3d.as_ref() else {
        return container(
            text("No 3D model linked yet — click \"Upload\" to attach one.")
                .size(11)
                .color(muted),
        )
        .padding(12)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .into();
    };

    let info = editor.three_d_upload_info.as_ref();
    let filename_line = info
        .map(|i| i.filename.clone())
        .unwrap_or_else(|| derive_filename_from_path(&model.path));
    let size_line = info
        .map(|i| i.human_size())
        .unwrap_or_else(|| "(size — set on next upload)".to_string());

    let body = column![
        kv_row("Filename", filename_line, tokens),
        kv_row("Size", size_line, tokens),
        kv_row("Storage", model.path.clone(), tokens),
    ]
    .spacing(4);

    container(
        column![
            text("Linked Model").size(11).color(text_c),
            Space::new().height(6),
            body,
        ]
        .spacing(0),
    )
    .padding(12)
    .style(crate::styles::modal_card(tokens))
    .width(Length::Fill)
    .into()
}

fn view_transform_grid<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    // Default to a zero ModelRef so the grid is interactive even
    // when no upload has happened yet — pre-set values then ride along
    // with the next upload via `apply_inline_edit`.
    let zero_default = ModelRef {
        path: String::new(),
        offset: [0.0; 3],
        rotation: [0.0; 3],
    };
    let model = editor.draft.pcb.model_3d.as_ref().unwrap_or(&zero_default);

    let axis_label = ["X", "Y", "Z"];

    let mut offset_row = row![text("Offset (mm)")
        .size(11)
        .color(text_c)
        .width(Length::Fixed(96.0))]
    .align_y(iced::Alignment::Center)
    .spacing(8);
    for (axis, axis_name) in axis_label.iter().enumerate() {
        offset_row = offset_row.push(text(*axis_name).size(10).color(muted));
        offset_row = offset_row.push(
            NumberInput::new(
                &model.offset[axis],
                f64::MIN..=f64::MAX,
                move |v: f64| LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::Model3dSetOffset { axis, value: v },
                },
            )
            .step(0.1_f64)
            .padding(4)
            .width(Length::Fixed(96.0)),
        );
    }

    let mut rotation_row = row![text("Rotation (deg)")
        .size(11)
        .color(text_c)
        .width(Length::Fixed(96.0))]
    .align_y(iced::Alignment::Center)
    .spacing(8);
    for (axis, axis_name) in axis_label.iter().enumerate() {
        rotation_row = rotation_row.push(text(*axis_name).size(10).color(muted));
        rotation_row = rotation_row.push(
            NumberInput::new(
                &model.rotation[axis],
                -360.0_f64..=360.0_f64,
                move |v: f64| LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::Model3dSetRotation { axis, value: v },
                },
            )
            .step(1.0_f64)
            .padding(4)
            .width(Length::Fixed(96.0)),
        );
    }

    container(
        column![
            text("Transform").size(11).color(text_c),
            Space::new().height(6),
            offset_row,
            Space::new().height(6),
            rotation_row,
        ]
        .spacing(0),
    )
    .padding(12)
    .style(crate::styles::modal_card(tokens))
    .width(Length::Fill)
    .into()
}

fn view_preview_placeholder<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let body = column![
        text("Pad-to-pin alignment preview").size(11).color(text_c),
        Space::new().height(6),
        text(
            "TODO(v0.9-phase-3): wgpu render of the uploaded mesh \
             overlaid on the footprint pads."
        )
        .size(10)
        .color(muted),
    ]
    .spacing(0);

    container(body)
        .padding(12)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .height(Length::Fixed(96.0))
        .into()
}

// ─────────────────────────────────────────────────────────────────────
// Small helpers
// ─────────────────────────────────────────────────────────────────────

fn kv_row<'a>(
    label: &'static str,
    value: String,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    row![
        text(label).size(10).color(muted).width(Length::Fixed(96.0)),
        // Read-only display — `text_input` with `on_input` omitted so
        // it reads "selectable but not editable" in iced 0.14. Default
        // text-input chrome matches the rest of the editor.
        text_input("", value.as_str()).padding([4, 8]).size(11),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8)
    .into()
}

fn derive_filename_from_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(_, b)| b.to_string())
        .unwrap_or_else(|| path.to_string())
}
