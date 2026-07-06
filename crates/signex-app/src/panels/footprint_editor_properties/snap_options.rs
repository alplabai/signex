//! v0.18.14 — Snap Options chrome — the 3-segment toggle row
//! (All Layers / Current Layer / Off) and the sub-tab pill row
//! (Grids / Guides / Axes).

use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
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
    let mk_pill = move |label: &'static str, target: M, active: bool| -> Element<'static, PanelMsg> {
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
                iced::widget::button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
                _ => Some(Background::Color(if active { active_bg } else { inactive_bg })),
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

/// v0.18.14.2 — Snap Options sub-tab strip (Grids / Guides / Axes).
/// Mirrors the schematic Properties tab-row visual rhythm. The
/// active sub-tab paints with the accent background; clicking a
/// pill sets `state.snap_subtab` via `FpEditorSetSnapSubTab`.
pub(super) fn render_snap_subtab_row<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    use crate::library::editor::footprint::state::SnapSubTab as T;
    let current = fp.snap_subtab;
    let mk_pill =
        move |label: &'static str, target: T, active: bool| -> Element<'static, PanelMsg> {
            let bg = if active {
                iced::Color::from_rgba(0.40, 0.70, 1.00, 0.18)
            } else {
                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };
            let txt = if active { primary } else { muted };
            iced::widget::button(container(text(label).size(10).color(txt)).padding([2, 8]))
                .padding(0)
                .on_press(PanelMsg::FpEditorSetSnapSubTab(target))
                .style(move |_: &Theme, _| iced::widget::button::Style {
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
    col = col.push(
        container(
            row![
                mk_pill("Grids", T::Grids, current == T::Grids),
                mk_pill("Guides", T::Guides, current == T::Guides),
                mk_pill("Axes", T::Axes, current == T::Axes),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );
    col
}

