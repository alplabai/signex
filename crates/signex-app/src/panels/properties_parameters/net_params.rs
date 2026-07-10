//! Net-attribute rows and the Parameters (Net) section chrome (tabs /
//! header / empty-state / add-bar) plus the two 3x3 justification-grid
//! pickers. Moved verbatim from the former single-file
//! `properties_parameters` module.

use super::super::*;

/// Net-attribute row: label | checkbox | text value | unit. Used for
/// "Power Net = 0.000 V" and "High Speed = 0.000 Hz".
pub fn net_numeric_row<'a>(
    label: &str,
    value: &str,
    unit: &str,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::checkbox(false)
                .on_toggle(|_| PanelMsg::Noop)
                .size(12)
                .spacing(4),
            container(text(value.to_string()).size(11).color(label_c),)
                .padding([3, 6])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(input_bg)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_bdr
                    },
                    ..container::Style::default()
                }),
            text(unit.to_string()).size(10).color(label_c),
        ]
        .spacing(6.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Parameters (Net) segmented tabs — All / Parameters / Rules / Classes.
pub fn net_params_tabs<'a>(
    primary: Color,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let tab = |label: &'static str, active: bool| -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        iced::widget::button(
            text(label.to_string())
                .size(11)
                .color(if active { fg_active } else { fg_inactive })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([3, 12])
        .on_press(PanelMsg::Noop)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(if active {
                    bg_active
                } else if hovered {
                    input_bdr
                } else {
                    input_bg
                })),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    let _ = label_c;
    container(
        row![
            tab("All", true),
            tab("Parameters", false),
            tab("Rules", false),
            tab("Classes", false),
        ]
        .spacing(4.0),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Two-column Name / Value header for the Parameters (Net) table.
pub fn net_params_header<'a>(label_c: Color, border_c: Color) -> Element<'a, PanelMsg> {
    container(
        row![
            text("Name".to_string())
                .size(10)
                .color(label_c)
                .width(Length::FillPortion(2)),
            text("Value".to_string())
                .size(10)
                .color(label_c)
                .width(Length::FillPortion(3)),
        ]
        .spacing(4.0),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
        },
        ..container::Style::default()
    })
    .into()
}

/// Empty-state row — centered muted text spanning the whole row.
pub fn empty_section_row<'a>(
    label: &str,
    label_c: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        container(text(label.to_string()).size(10).color(label_c))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
        },
        ..container::Style::default()
    })
    .into()
}

/// Add / edit / delete toolbar at the bottom of the Parameters table.
pub fn net_params_add_bar<'a>(
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let icon_btn = |label: &'static str| -> Element<'static, PanelMsg> {
        iced::widget::button(text(label.to_string()).size(11).color(label_c))
            .padding([4, 8])
            .on_press(PanelMsg::Noop)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(input_bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: label_c,
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    container(
        row![
            Space::new().width(Length::Fill),
            iced::widget::button(
                text("Add \u{25BE}".to_string())
                    .size(11)
                    .color(Color::WHITE)
            )
            .padding([4, 12])
            .on_press(PanelMsg::Noop)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(input_bdr)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr
                },
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            }),
            icon_btn("\u{270E}"),
            icon_btn("\u{1F5D1}"),
        ]
        .spacing(4.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Altium-style 3x3 justification picker with proper SVG arrow icons.
/// Only horizontal is wired to state for now; vertical slots toggle visually
/// but don't mutate the label.
pub fn justification_grid(
    id: uuid::Uuid,
    rotation_deg: f64,
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    muted: Color,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let _ = muted;

    // Cell size mimics Altium's compact 24×24 px anchor picker.
    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: iced::widget::svg::Handle,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget =
            iced::widget::svg(handle)
                .width(12.0)
                .height(12.0)
                .style(move |_: &Theme, _| iced::widget::svg::Style {
                    color: Some(if active { fg_active } else { fg_inactive }),
                });
        iced::widget::button(
            container(svg_widget)
                .width(Length::Fill)
                .height(Length::Fill)
                .center(Length::Fill),
        )
        .width(CELL_SIZE)
        .height(CELL_SIZE)
        .padding(0)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let bg = if active {
                bg_active
            } else if hovered {
                input_bdr
            } else {
                input_bg
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LabelDir {
        Left,
        Up,
        Right,
        Down,
    }

    let normalize_rot = |deg: f64| {
        let r = (deg.round() as i32) % 360;
        if r < 0 { r + 360 } else { r }
    };

    let current_dir = {
        match normalize_rot(rotation_deg) {
            90 => LabelDir::Up,
            270 => LabelDir::Down,
            180 => {
                if matches!(h, HAlign::Right) {
                    LabelDir::Right
                } else {
                    LabelDir::Left
                }
            }
            _ => {
                if matches!(h, HAlign::Right) {
                    LabelDir::Left
                } else {
                    LabelDir::Right
                }
            }
        }
    };

    let to_msg = |dir: LabelDir| -> PanelMsg {
        match dir {
            LabelDir::Right => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Left),
            LabelDir::Left => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Right),
            LabelDir::Up => PanelMsg::EditLabelDirection(id, 90.0, HAlign::Left),
            LabelDir::Down => PanelMsg::EditLabelDirection(id, 270.0, HAlign::Left),
        }
    };

    let hl = |dir: LabelDir| current_dir == dir;

    iced::widget::column![
        iced::widget::row![
            cell(
                crate::icons::icon_justify_tl(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
            cell(
                crate::icons::icon_justify_t(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
            cell(
                crate::icons::icon_justify_tr(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_l(theme),
                hl(LabelDir::Left),
                to_msg(LabelDir::Left)
            ),
            cell(
                crate::icons::icon_justify_c(theme),
                false,
                to_msg(current_dir)
            ),
            cell(
                crate::icons::icon_justify_r(theme),
                hl(LabelDir::Right),
                to_msg(LabelDir::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_bl(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
            cell(
                crate::icons::icon_justify_b(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
            cell(
                crate::icons::icon_justify_br(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
        ]
        .spacing(2),
    ]
    .spacing(2)
    .into()
}

/// Pre-placement 3x3 justification picker. Same visual grid as the
/// selection-aware `justification_grid` but dispatches to the
/// `SetPrePlacementJustifyH` message family (no UUID needed).
pub fn preplacement_justification_grid(
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    muted: Color,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let _ = muted;

    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: iced::widget::svg::Handle,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget =
            iced::widget::svg(handle)
                .width(12.0)
                .height(12.0)
                .style(move |_: &Theme, _| iced::widget::svg::Style {
                    color: Some(if active { fg_active } else { fg_inactive }),
                });
        iced::widget::button(
            container(svg_widget)
                .width(Length::Fill)
                .height(Length::Fill)
                .center(Length::Fill),
        )
        .width(CELL_SIZE)
        .height(CELL_SIZE)
        .padding(0)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let bg = if active {
                bg_active
            } else if hovered {
                input_bdr
            } else {
                input_bg
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    let hl_mid = |target: HAlign| -> bool { h == target };
    iced::widget::column![
        iced::widget::row![
            cell(
                crate::icons::icon_justify_tl(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_t(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_tr(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_l(theme),
                hl_mid(HAlign::Left),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_c(theme),
                hl_mid(HAlign::Center),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_r(theme),
                hl_mid(HAlign::Right),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_bl(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_b(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_br(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
    ]
    .spacing(2)
    .into()
}
