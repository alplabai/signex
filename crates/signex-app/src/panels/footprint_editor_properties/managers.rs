//! Grid Manager + Other section + grid_manager_btn — the per-editor
//! library Options surfaces below the Pad form.

use iced::widget::{Column, Space, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::super::{FootprintEditorPanelContext, PanelMsg, PanelPalette};

/// v0.18.21 — Grid Manager table. One row per `GridDef`. The active
/// row is highlighted; clicking another row activates it (mirrors its
/// step / display style onto `snap_options`). The footer's Add /
/// Properties / Delete operate on the active row.
pub(super) fn render_grid_manager<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    // Header row — Altium PCB Library editor columns: Prior / Name /
    // Color / Origin / Enabled. "Step" stays as a sub-row info line
    // since Altium puts it in the Properties dialog, not the grid row.
    col = col.push(
        container(
            row![
                text("Prior")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(40.0)),
                text("Name").size(10).color(muted).width(Length::Fill),
                text("Color")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(40.0)),
                text("Origin")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(60.0)),
                text("Enabled")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(50.0)),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );
    col = col.push(super::super::thin_sep(border_c));

    let active_idx = fp.active_grid_idx;
    if fp.grids.is_empty() {
        col = col.push(
            container(text("(no grids)").size(10).color(muted))
                .padding([4, 8])
                .width(Length::Fill),
        );
    } else {
        for (idx, g) in fp.grids.iter().enumerate() {
            let is_active = idx == active_idx;
            let row_bg = if is_active {
                iced::Color::from_rgba(0.30, 0.55, 0.95, 0.16)
            } else {
                iced::Color::TRANSPARENT
            };
            // Color swatch — placeholder using the theme accent until
            // GridDef.color lands. Click does nothing yet.
            let swatch = container(Space::new())
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(14.0))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba8(0xff, 0xff, 0xff, 1.0))),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                    },
                    ..Default::default()
                });
            // Enabled column — checkbox toggling active grid.
            let enabled_check = iced::widget::checkbox(is_active)
                .on_toggle(move |_| PanelMsg::FpEditorGridSetActive(idx))
                .size(12)
                .spacing(0);
            col = col.push(
                container(
                    row![
                        text(format!("{}", (idx + 1) * 10))
                            .size(10)
                            .color(if is_active { primary } else { muted })
                            .width(Length::Fixed(40.0)),
                        text(g.name.as_str())
                            .size(10)
                            .color(if is_active { primary } else { muted })
                            .width(Length::Fill),
                        container(swatch).width(Length::Fixed(40.0)).padding([0, 0]),
                        text("0,0").size(10).color(muted).width(Length::Fixed(60.0)),
                        container(enabled_check)
                            .width(Length::Fixed(50.0))
                            .center_x(Length::Shrink),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([3, 8])
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(row_bg)),
                    ..Default::default()
                }),
            );
        }
    }
    // Action footer — Add / Properties / Delete using primary
    // (orange-accent) buttons + a unicode trash glyph for Delete to
    // mirror Altium's icon-style footer.
    col = col.push(super::super::thin_sep(border_c));
    col = col.push(
        container(
            row![
                Space::new().width(Length::Fill),
                grid_manager_btn(
                    "Add",
                    Some(PanelMsg::FpEditorGridManagerAdd),
                    primary,
                    border_c,
                ),
                grid_manager_btn(
                    "Properties",
                    Some(PanelMsg::FpEditorGridManagerProperties),
                    primary,
                    border_c,
                ),
                grid_manager_btn(
                    "\u{1F5D1}",
                    if fp.grids.len() > 1 {
                        Some(PanelMsg::FpEditorGridManagerDelete)
                    } else {
                        None
                    },
                    primary,
                    border_c,
                ),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col
}

/// v0.18.13 — Other section. Today carries only a Units toggle;
/// future home for additional document-level options.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_other_section<'a>(
    mut col: Column<'a, PanelMsg>,
    _fp: &'a FootprintEditorPanelContext,
    palette: PanelPalette,
    unit: signex_types::coord::Unit,
) -> Column<'a, PanelMsg> {
    let PanelPalette {
        muted,
        primary,
        input_bg,
        input_bdr,
        seg_hover,
        ..
    } = palette;
    use signex_types::coord::Unit;
    // Units row — mm/mils segmented selector (Altium parity). Reuses
    // the schematic Properties panel's `seg_btn` widget so the chrome
    // matches byte-for-byte.
    col = col.push(super::super::form_label("Units", muted));
    col = col.push(
        container(
            row![
                super::super::seg_btn(
                    "mm",
                    unit == Unit::Mm,
                    PanelMsg::SetUnit(Unit::Mm),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr,
                ),
                super::super::seg_btn(
                    "mils",
                    unit == Unit::Mil,
                    PanelMsg::SetUnit(Unit::Mil),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr,
                ),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([2, 8]),
    );
    col
}

/// Shared button factory for the Grid / Guide Manager footers.
/// Uses iced's built-in `button::primary` (accent-filled) so the
/// chrome matches the "+ Add Filter" call-to-action button in the
/// Custom Selection Filters section above.
pub(super) fn grid_manager_btn<'a>(
    label: &'static str,
    on_press: Option<PanelMsg>,
    primary: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    let mut btn = iced::widget::button(text(label).size(10).color(primary))
        .padding([4, 10])
        .style(iced::widget::button::primary);
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn.into()
}
