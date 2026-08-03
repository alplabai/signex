//! v0.18.14 — Snap Options chrome — the 3-segment toggle row
//! (All Layers / Current Layer / Off).

use iced::widget::{Column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::super::{FootprintEditorPanelContext, PanelMsg};

/// v0.18.14.3 — Altium "Snapping" 3-segment toggle. `All Layers` is
/// the default behaviour (current pre-v0.18.14 functionality);
/// `Current Layer` is a placeholder for the v0.18.15 layer-aware
/// enforcement; `Off` short-circuits every snap priority in
/// `snap::snap_cursor` so the cursor returns the raw click.
pub(super) fn render_snapping_mode_row<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    _border_c: Color,
) -> Column<'a, PanelMsg> {
    use crate::library::editor::footprint::state::SnappingMode as M;
    let current = fp.snapping_mode;
    // v0.13 — Match the Grids/Guides/Axes pill chrome above. Mutex
    // semantics (clicking one selects only that mode), but the same
    // border / fill / padding so the row reads visually identical.
    let chip_border = Color::from_rgba8(0xE7, 0x8B, 0x2A, 1.0);
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let mk_pill =
        move |label: &'static str, target: M, active: bool| -> Element<'static, PanelMsg> {
            iced::widget::button(
                text(label)
                    .size(10)
                    .color(if active { primary } else { muted })
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .padding([3, 12])
            .on_press(PanelMsg::FpEditorSetSnappingMode(target))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => {
                        Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06)))
                    }
                    _ => Some(Background::Color(if active {
                        active_bg
                    } else {
                        inactive_bg
                    })),
                };
                iced::widget::button::Style {
                    background: bg,
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: chip_border,
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            .into()
        };
    col = col.push(
        container(text("Snap layers").size(10).color(muted))
            .padding([4, 8])
            .width(Length::Fill),
    );
    col = col.push(
        container(
            row![
                mk_pill("All Layers", M::AllLayers, current == M::AllLayers),
                mk_pill("Current Layer", M::CurrentLayer, current == M::CurrentLayer),
                mk_pill("Off", M::Off, current == M::Off),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );
    col
}
