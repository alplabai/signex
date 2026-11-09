//! Grid Manager + Other section + grid_manager_btn — the per-editor
//! library Options surfaces below the Pad form.

use iced::widget::{Column, Space, container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_widgets::{raw, text};

use super::super::{FootprintEditorPanelContext, PanelMsg, PanelPalette};
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// v0.18.21 — Grid Manager table. One row per `GridDef`. The active
/// row is highlighted; clicking another row activates it (mirrors its
/// step / display style onto `snap_options`). The footer's Add /
/// Properties / Delete operate on the active row.
pub(super) fn render_grid_manager<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    // Header row — Altium PCB Library editor columns: Prior / Name /
    // Color / Origin / Enabled. "Step" stays as a sub-row info line
    // since Altium puts it in the Properties dialog, not the grid row.
    col = col.push(
        container(
            row![
                text(raw("Prior"))
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fixed(40.0)),
                text(raw("Name"))
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fill),
                text(raw("Color"))
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fixed(40.0)),
                text(raw("Origin"))
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fixed(60.0)),
                text(raw("Enabled"))
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fixed(50.0)),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed(),
    );
    col = col.push(super::super::thin_sep(border_c));

    let active_idx = fp.active_grid_idx;
    if fp.grids.is_empty() {
        col = col.push(
            container(text(raw("(no grids)")).size(TS.caption).color(muted))
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
    } else {
        for (idx, g) in fp.grids.iter().enumerate() {
            let is_active = idx == active_idx;
            let row_bg = if is_active {
                iced::Color::from_rgba(0.30, 0.55, 0.95, 0.16)
            } else {
                iced::Color::TRANSPARENT
            };
            // Color swatch — placeholder white until GridDef.color lands.
            // Click does nothing yet.
            let swatch = container(Space::new())
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(14.0))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba8(0xff, 0xff, 0xff, 1.0))),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                        ..Default::default()
                    },
                    ..Default::default()
                });
            // Enabled column — checkbox toggling active grid.
            let enabled_check = signex_widgets::checkbox(is_active)
                .on_toggle(move |_| PanelMsg::FpEditorGridSetActive(idx))
                .size(TS.label)
                .spacing(0);
            col = col.push(
                container(
                    row![
                        text(raw(format!("{}", (idx + 1) * 10)))
                            .size(TS.caption)
                            .color(if is_active { primary } else { muted })
                            .width(Length::Fixed(40.0)),
                        text(signex_i18n::UserText::new(g.name.as_str()))
                            .size(TS.caption)
                            .color(if is_active { primary } else { muted })
                            .width(Length::Fill),
                        container(swatch).width(Length::Fixed(40.0)).padding([0, 0]),
                        text(raw("0,0"))
                            .size(TS.caption)
                            .color(muted)
                            .width(Length::Fixed(60.0)),
                        container(enabled_check)
                            .width(Length::Fixed(50.0))
                            .center_x(Length::Shrink),
                    ]
                    .spacing(SP.s)
                    .align_y(iced::Alignment::Center),
                )
                .padding([3.0, SP.m])
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(row_bg)),
                    ..Default::default()
                })
                .boxed(),
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
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed(),
    );
    col
}

/// v0.18.13 — Other section. Today carries only a Units toggle;
/// future home for additional document-level options.
pub(super) fn render_other_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    _fp: &'a FootprintEditorPanelContext,
    palette: PanelPalette,
    unit: signex_types::coord::Unit,
) -> Column<Element<'a, PanelMsg>> {
    let PanelPalette {
        muted,
        primary,
        input_bg,
        ..
    } = palette;
    use signex_types::coord::Unit;
    // Units row — mm/mils segmented selector (Altium parity). Reuses
    // the schematic Properties panel's `seg_btn` widget so the chrome
    // matches byte-for-byte.
    col = col.push(super::super::form_label("Units"));
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
                ),
                super::super::seg_btn(
                    "mils",
                    unit == Unit::Mil,
                    PanelMsg::SetUnit(Unit::Mil),
                    input_bg,
                    primary,
                    muted,
                ),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([SP.xs, SP.m])
        .boxed(),
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
    let mut btn = signex_widgets::button(text(raw(label)).size(TS.caption).color(primary))
        .padding([SP.s, 10.0])
        .style(iced::widget::button::primary);
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn.boxed()
}
