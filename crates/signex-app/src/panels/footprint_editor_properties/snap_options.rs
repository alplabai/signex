//! v0.18.14 — Snap Options chrome — the 3-segment toggle row
//! (All Layers / Current Layer / Off).

use iced::widget::{Column, container, row};
use iced::{Color, Element, Length, Theme, Widget};
use signex_widgets::{raw, text};

use super::super::{FootprintEditorPanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// v0.18.14.3 — Altium "Snapping" 3-segment toggle. `All Layers` is
/// the default behaviour (current pre-v0.18.14 functionality);
/// `Current Layer` is a placeholder for the v0.18.15 layer-aware
/// enforcement; `Off` short-circuits every snap priority in
/// `snap::snap_cursor` so the cursor returns the raw click.
pub(super) fn render_snapping_mode_row<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    _border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    use crate::library::editor::footprint::state::SnappingMode as M;
    let current = fp.snapping_mode;
    // v0.13 — Match the Grids/Guides/Axes pill chrome above. Mutex
    // semantics (clicking one selects only that mode), but the same
    // border / fill / padding so the row reads visually identical.
    let mk_pill =
        move |label: &'static str, target: M, active: bool| -> Element<'static, PanelMsg> {
            signex_widgets::button(
                text(raw(label))
                    .size(TS.caption)
                    .color(if active { primary } else { muted })
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .padding([3.0, SP.l])
            .on_press(PanelMsg::FpEditorSetSnappingMode(target))
            .style(move |theme: &Theme, status: iced::widget::button::Status| {
                let hovered = matches!(status, iced::widget::button::Status::Hovered);
                crate::panels::form_look::segment_cell(theme, active, hovered)
            })
            .boxed()
        };
    col = col.push(
        container(text(raw("Snap layers")).size(TS.caption).color(muted))
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    col = col.push(
        container(
            row![
                mk_pill("All Layers", M::AllLayers, current == M::AllLayers),
                mk_pill("Current Layer", M::CurrentLayer, current == M::CurrentLayer),
                mk_pill("Off", M::Off, current == M::Off),
            ]
            .spacing(SP.s)
            .align_y(iced::alignment::Vertical::Center),
        )
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .style(crate::panels::form_look::segmented_frame)
        .boxed(),
    );
    col
}
