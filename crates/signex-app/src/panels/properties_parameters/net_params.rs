//! The Net Parameters chrome of a power port (the All / Parameters / Rules /
//! Classes tabs, the empty card and its Add button) plus the two 3x3
//! justification-grid pickers. Moved from the former single-file
//! `properties_parameters` module.

use super::super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Net Parameters tabs — All / Parameters / Rules / Classes, as a segmented
/// control. A net holds no parameters, rules or classes here yet, so All
/// stays on and every tab shows the same empty card.
pub fn net_params_tabs<'a>(
    i18n: &signex_i18n::I18n,
    primary: Color,
    muted: Color,
    input_bg: Color,
) -> Element<'a, PanelMsg> {
    let tab = |label: MessageId, active: bool| {
        super::seg_btn(
            signex_i18n::t!(i18n, label),
            active,
            PanelMsg::Noop,
            input_bg,
            primary,
            muted,
        )
    };
    container(
        container(
            row![
                tab(ids::PROPERTIES_TAB_ALL, true),
                tab(ids::PROPERTIES_TAB_PARAMETERS, false),
                tab(ids::PROPERTIES_TAB_RULES, false),
                tab(ids::PROPERTIES_TAB_CLASSES, false),
            ]
            .spacing(0)
            .width(Length::Fill),
        )
        .padding(SP.xs)
        .style(crate::panels::form_look::segmented_frame),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Empty-state row — centered muted text spanning the whole row.
pub fn empty_section_row<'a>(
    label: signex_i18n::Localized,
    label_c: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        container(text(label).size(TS.caption).color(label_c))
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
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The Add ▾ button under the Net Parameters card.
pub fn net_params_add_bar<'a>(i18n: &signex_i18n::I18n) -> Element<'a, PanelMsg> {
    container(
        row![
            Space::new().width(Length::Fill),
            signex_widgets::button(
                text(signex_i18n::t!(i18n, ids::PROPERTIES_NET_PARAMS_ADD)).size(TS.body)
            )
            .padding([SP.s, SP.l])
            .on_press(PanelMsg::Noop)
            .style(crate::panels::form_look::ghost_button),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Which way a label's text runs from its anchor: the four directions the
/// 3x3 grid maps its nine cells onto.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelDir {
    Left,
    Up,
    Right,
    Down,
}

/// The direction a label turned `rotation_deg` and justified `h` reads in.
fn label_direction(rotation_deg: f64, h: signex_types::schematic::HAlign) -> LabelDir {
    use signex_types::schematic::HAlign;
    let turn = (rotation_deg.round() as i32).rem_euclid(360);
    // Right-justified text runs left of its anchor, unless turned upside down.
    let runs_right = (h == HAlign::Right) == (turn == 180);
    match turn {
        90 => LabelDir::Up,
        270 => LabelDir::Down,
        _ if runs_right => LabelDir::Right,
        _ => LabelDir::Left,
    }
}

/// The word beside the justification grid that names the label's anchor:
/// where on its text the anchor sits.
pub fn label_anchor(rotation_deg: f64, h: signex_types::schematic::HAlign) -> MessageId {
    match label_direction(rotation_deg, h) {
        LabelDir::Right => ids::PROPERTIES_ANCHOR_BOTTOM_LEFT,
        LabelDir::Left => ids::PROPERTIES_ANCHOR_BOTTOM_RIGHT,
        LabelDir::Up => ids::PROPERTIES_ANCHOR_BOTTOM,
        LabelDir::Down => ids::PROPERTIES_ANCHOR_TOP,
    }
}

/// The 3x3 justification picker of a selected label, with SVG arrow icons.
/// A label reads in four directions, so the nine cells map onto them
/// (corners follow their edge; the centre re-sends the current one).
pub fn justification_grid(
    id: uuid::Uuid,
    rotation_deg: f64,
    h: signex_types::schematic::HAlign,
    palette: PanelPalette,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let PanelPalette {
        input_bg,
        input_bdr,
        primary,
        ..
    } = palette;

    // A compact 24 x 24 px anchor cell.
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
                    ..Default::default()
                });
        signex_widgets::button(
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
                    ..Border::default()
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
    };
    let current_dir = label_direction(rotation_deg, h);

    let to_msg = |dir: LabelDir| -> PanelMsg {
        match dir {
            LabelDir::Right => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Left),
            LabelDir::Left => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Right),
            LabelDir::Up => PanelMsg::EditLabelDirection(id, 90.0, HAlign::Left),
            LabelDir::Down => PanelMsg::EditLabelDirection(id, 270.0, HAlign::Left),
        }
    };

    // A label has only four anchor directions, so the nine cells map onto
    // them (corners follow their edge; centre re-sends the current one).
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
        .spacing(SP.xs),
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
        .spacing(SP.xs),
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
        .spacing(SP.xs),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// Pre-placement 3x3 justification picker. Same visual grid as the
/// selection-aware `justification_grid` but dispatches to the
/// `SetPrePlacementJustifyH` message family (no UUID needed).
pub fn preplacement_justification_grid(
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;

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
                    ..Default::default()
                });
        signex_widgets::button(
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
                    ..Border::default()
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
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
        .spacing(SP.xs),
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
        .spacing(SP.xs),
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
        .spacing(SP.xs),
    ]
    .spacing(SP.xs)
    .boxed()
}
