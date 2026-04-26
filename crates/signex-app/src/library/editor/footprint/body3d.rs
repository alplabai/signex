//! Body 3D editor pane.
//!
//! Sits in the right column of the Footprint tab (per
//! `v0.9-library-refactor-plan.md` §11 step F3). Edits the
//! [`signex_library::Body3D`] embedded on the active footprint
//! primitive — the procedural 3D render in `preview3d.rs` rebuilds off
//! these values on every frame.

use iced::widget::{Space, button, column, container, pick_list, row, text};
use iced::{Border, Element, Length, Theme};
use iced_aw::NumberInput;
use signex_library::{Body3D, BodyShape};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::messages::{EditorMsg, LibraryMessage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShapePick(BodyShape);

impl std::fmt::Display for ShapePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            BodyShape::Extrude => "Extrude",
            BodyShape::Dome => "Dome",
            BodyShape::Cylinder => "Cylinder",
            BodyShape::Custom => "Custom",
            other => return write!(f, "{other:?}"),
        };
        f.write_str(s)
    }
}

/// Render the Body 3D editor pane. `body` is borrowed from
/// `Footprint::body_3d`; messages mutate it through the dispatcher.
pub fn view<'a>(
    body: &Body3D,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let header = text("Body 3D").size(13).color(text_c);

    // Shape picker.
    let opts = [
        ShapePick(BodyShape::Extrude),
        ShapePick(BodyShape::Dome),
        ShapePick(BodyShape::Cylinder),
        ShapePick(BodyShape::Custom),
    ];
    let shape_picker = pick_list(
        opts,
        Some(ShapePick(body.shape)),
        move |ShapePick(s)| LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SetBodyShape(s),
        },
    )
    .text_size(11)
    .padding([4, 8]);

    // Numeric inputs — height_mm / offset_z_mm.
    let height_input = NumberInput::new(
        &body.height_mm,
        0.0_f32..=50.0_f32,
        move |v: f32| LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SetBodyHeight(v),
        },
    )
    .step(0.1_f32)
    .padding(4)
    .width(Length::Fixed(96.0));

    let offset_input = NumberInput::new(
        &body.offset_z_mm,
        -10.0_f32..=10.0_f32,
        move |v: f32| LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SetBodyOffsetZ(v),
        },
    )
    .step(0.1_f32)
    .padding(4)
    .width(Length::Fixed(96.0));

    // Color rows — three swatch buttons + RGBA hex display.
    let top_row = color_row(
        "Top color",
        body.top_color,
        muted,
        text_c,
        tokens,
        window_id,
        true,
    );
    let side_row = color_row(
        "Side color",
        body.side_color,
        muted,
        text_c,
        tokens,
        window_id,
        false,
    );

    let body_col = column![
        header,
        Space::new().height(8),
        labeled_field("Shape", shape_picker, muted),
        Space::new().height(6),
        labeled_field("Height (mm)", height_input, muted),
        Space::new().height(6),
        labeled_field("Offset Z (mm)", offset_input, muted),
        Space::new().height(8),
        top_row,
        Space::new().height(6),
        side_row,
    ]
    .spacing(0)
    .width(Length::Fill);

    container(body_col)
        .padding(12)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .into()
}

fn labeled_field<'a>(
    label: &'static str,
    field: impl Into<Element<'a, LibraryMessage>>,
    muted: iced::Color,
) -> Element<'a, LibraryMessage> {
    column![
        text(label).size(10).color(muted),
        Space::new().height(2),
        field.into(),
    ]
    .spacing(0)
    .into()
}

/// Render one color row — preview swatch + 4 numeric inputs (R/G/B/A
/// 0..1) + cycle button. We don't pull the full ColorPicker here
/// (would explode the panel height); the cycle preset gives the user
/// quick access to a dark/light/custom palette.
fn color_row<'a>(
    label: &'static str,
    rgba: [f32; 4],
    muted: iced::Color,
    text_c: iced::Color,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
    is_top: bool,
) -> Element<'a, LibraryMessage> {
    let border = theme_ext::border_color(tokens);
    let preview_color = iced::Color::from_rgba(rgba[0], rgba[1], rgba[2], 1.0);
    let preview_swatch = container(text("").size(11))
        .width(Length::Fixed(36.0))
        .height(Length::Fixed(20.0))
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(preview_color)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::container::Style::default()
        });

    // 5-step preset palette so the user can cycle without typing.
    let presets: [[f32; 4]; 5] = [
        [0.10, 0.10, 0.10, 1.0], // matte black
        [0.20, 0.20, 0.20, 1.0], // dark grey
        [0.85, 0.78, 0.30, 1.0], // gold
        [0.30, 0.45, 0.85, 1.0], // blue
        [0.70, 0.20, 0.20, 1.0], // red
    ];
    // Find current preset index (for cycling).
    let current_idx = presets.iter().position(|p| (*p) == rgba).unwrap_or(0);
    let next = presets[(current_idx + 1) % presets.len()];

    let msg_factory = move |c: [f32; 4]| -> EditorMsg {
        if is_top {
            EditorMsg::SetBodyTopColor(c)
        } else {
            EditorMsg::SetBodySideColor(c)
        }
    };

    let cycle_btn = button(container(text("Cycle").size(10).color(text_c)).padding([3, 8]))
        .on_press(LibraryMessage::EditorEvent {
            window_id,
            msg: msg_factory(next),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    let hex = format!(
        "#{:02x}{:02x}{:02x}",
        (rgba[0] * 255.0) as u8,
        (rgba[1] * 255.0) as u8,
        (rgba[2] * 255.0) as u8,
    );

    column![
        text(label).size(10).color(muted),
        Space::new().height(2),
        row![
            preview_swatch,
            Space::new().width(8),
            text(hex).size(11).color(text_c),
            Space::new().width(Length::Fill),
            cycle_btn,
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(0)
    .into()
}

#[cfg(test)]
mod tests {
    use signex_library::{Body3D, BodyShape};

    /// `Body3D::default()` should give us a sensible block: visible
    /// (non-zero alpha + non-zero height) and a defined extrude shape.
    #[test]
    fn body3d_default_is_a_sensible_block() {
        let b = Body3D::default();
        assert_eq!(b.shape, BodyShape::Extrude);
        assert!(b.height_mm > 0.0, "default height_mm should be > 0");
        assert!(
            b.top_color[3] > 0.0,
            "default top color should not be transparent"
        );
        assert!(
            b.side_color[3] > 0.0,
            "default side color should not be transparent"
        );
    }
}
