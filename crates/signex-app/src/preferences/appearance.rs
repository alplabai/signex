//! Appearance section — theme picker cards, UI font, and the
//! schematic / symbol-editor render-style pick-lists.
//!
//! Moved verbatim from the former single-file `preferences` module —
//! pure view code, zero behaviour change.

use super::*;
use crate::fonts;
use crate::render_config::{GridStyle, PinSelectionMode};
use iced::widget::{Space, column, row};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{raw, text};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn content_appearance<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let PrefsView {
        draft_theme,
        draft_font,
        draft_grid_style,
        draft_pcb_gpu_render,
        draft_antialiasing,
        draft_symbol_grid_size_mm,
        draft_symbol_grid_style,
        draft_symbol_pin_selection,
        draft_zoom_step_percent,
        draft_trackpad_pan,
        draft_wheel_mode,
        draft_remember_window_geometry,
        draft_ui_scale_percent,
        draft_window_controls_placement,
        window_controls_restart_note,
        i18n,
        ..
    } = v;
    let mut col: iced::widget::Column<Element<'a, PrefMsg>> =
        column![].spacing(0).padding([SP.xl, 20.0]);

    // ── Section: Theme ──
    col = col.push(super::theme_cards::theme_section(
        draft_theme,
        v.panel_tokens,
        i18n,
    ));
    col = col.push(Space::new().height(10).boxed());

    // ── Divider ──
    col = col.push(Space::new().height(16).boxed());
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());

    // ── Section: UI Font ──
    col = col.push(section_title("Font"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_UI_FONT))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_UI_FONT_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            {
                let families = fonts::system_font_families();
                let current_owned = draft_font.to_string();
                super::graphite_fields::font_pick_list(
                    current_owned,
                    families.as_slice(),
                    v.panel_tokens,
                )
            },
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Window controls ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Window controls"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(window_controls_row(
        draft_window_controls_placement,
        window_controls_restart_note,
        i18n,
    ));
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Schematic Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Schematic Editor"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_GRID_STYLE))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(
                    i18n,
                    ids::PREFERENCES_APPEARANCE_SCHEMATIC_GRID_STYLE_HELP
                ))
                .size(TS.caption)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(Some(draft_grid_style), GridStyle::ALL, |v: &GridStyle| {
                v.to_string()
            })
            .graphite()
            .on_select(PrefMsg::DraftGridStyle)
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: PCB Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("PCB Editor"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_GPU_RENDER))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_GPU_RENDER_HELP))
                    .size(TS.caption)
                    .style(text_muted),
                // This used to carry a second line naming the known GPU gaps.
                // The last of them is closed: dashed lines, concave fills and
                // overlay z-order earlier, the faded hairline (#645), and the
                // fill-over-trace stacking (#653) — see the `scene::order`
                // parity tests. "experimental" stays because GPU parity is
                // still unconfirmed on hardware.
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_pcb_gpu_render)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftPcbGpuRender),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Rendering ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Rendering"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_ANTIALIASING))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_ANTIALIASING_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_antialiasing)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftAntialiasing),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Symbol Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Symbol Editor"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_SYMBOL_GRID_SIZE))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_SYMBOL_GRID_SIZE_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                crate::canvas::grid::GRID_SIZES_MM
                    .iter()
                    .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                    .find(|(sz, _)| (**sz - draft_symbol_grid_size_mm).abs() < 0.001)
                    .map(|(_, lbl)| *lbl),
                crate::canvas::grid::GRID_SIZE_LABELS,
                |lbl: &&'static str| lbl.to_string(),
            )
            .graphite()
            .on_select(|lbl: &'static str| {
                let mm = crate::canvas::grid::GRID_SIZES_MM
                    .iter()
                    .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                    .find(|(_, l)| **l == lbl)
                    .map(|(sz, _)| *sz)
                    .unwrap_or(1.27);
                PrefMsg::DraftSymbolGridSize(mm)
            })
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(16).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_GRID_STYLE))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_SYMBOL_GRID_STYLE_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                Some(draft_symbol_grid_style),
                GridStyle::ALL,
                |v: &GridStyle| v.to_string(),
            )
            .graphite()
            .on_select(PrefMsg::DraftSymbolGridStyle)
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(16).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_PIN_SELECTION))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_PIN_SELECTION_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                Some(draft_symbol_pin_selection),
                PinSelectionMode::ALL,
                |v: &PinSelectionMode| v.to_string(),
            )
            .graphite()
            .on_select(PrefMsg::DraftSymbolPinSelection)
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Interface scale ──
    col = col.push(section_title("Interface scale"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_UI_SCALE))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_UI_SCALE_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::slider(
                crate::ui_scale::UI_SCALE_MIN_PERCENT..=crate::ui_scale::UI_SCALE_MAX_PERCENT,
                draft_ui_scale_percent,
                PrefMsg::DraftUiScalePercent,
            )
            .step(crate::ui_scale::UI_SCALE_STEP_PERCENT)
            .width(140),
            text(raw(format!("{draft_ui_scale_percent}%")))
                .size(TS.label)
                .style(text_primary)
                .width(100)
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Navigation ──
    col = col.push(section_title("Navigation"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_ZOOM_SPEED))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_ZOOM_SPEED_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::slider(
                crate::canvas::Camera::ZOOM_STEP_PERCENT_MIN
                    ..=crate::canvas::Camera::ZOOM_STEP_PERCENT_MAX,
                draft_zoom_step_percent,
                PrefMsg::DraftZoomStepPercent,
            )
            .width(140),
            text(t!(
                i18n,
                ids::PREFERENCES_APPEARANCE_ZOOM_STEP_VALUE,
                "percent" => u32::from(draft_zoom_step_percent)
            ))
            .size(TS.label)
            .style(text_primary)
            .width(100)
            .align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(14).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_TRACKPAD_PAN))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_TRACKPAD_PAN_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(320),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_trackpad_pan)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftTrackpadPan),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(14).boxed());
    col = col.push(
        row![
            column![
                text(signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::PREFERENCES_WHEEL_MODE_LABEL
                ))
                .size(TS.label)
                .style(text_primary),
                text(signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::PREFERENCES_WHEEL_MODE_HELP
                ))
                .size(TS.caption)
                .style(text_muted),
            ]
            .spacing(3)
            .width(320),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                Some(draft_wheel_mode),
                crate::canvas::WheelMode::ALL,
                |v: &crate::canvas::WheelMode| v.to_string(),
            )
            .graphite()
            .on_select(PrefMsg::DraftWheelMode)
            .width(120),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Window ──
    col = col.push(section_title("Window"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_REMEMBER_GEOMETRY))
                    .size(TS.label)
                    .style(text_primary),
                text(t!(i18n, ids::PREFERENCES_APPEARANCE_REMEMBER_GEOMETRY_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(320),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_remember_window_geometry)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftRememberWindowGeometry),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    col.boxed()
}

/// Where the minimize / maximize / close buttons go, with a restart note
/// when the drafted choice changes how macOS creates the window.
fn window_controls_row<'a>(
    draft: crate::chrome::controls::WindowControlsPlacement,
    restart_note: bool,
    i18n: &'a I18n,
) -> Element<'a, PrefMsg> {
    let mut help = column![
        text(t!(i18n, ids::PREFERENCES_APPEARANCE_WINDOW_BUTTONS))
            .size(TS.label)
            .style(text_primary),
        text(t!(i18n, ids::PREFERENCES_APPEARANCE_WINDOW_BUTTONS_HELP))
            .size(TS.caption)
            .style(text_muted),
    ]
    .spacing(3)
    .width(200);
    if restart_note {
        help = help.push(
            text(t!(i18n, ids::PREFERENCES_APPEARANCE_RESTART_NOTE))
                .size(TS.caption)
                .style(text_muted)
                .boxed(),
        );
    }
    row![
        help,
        Space::new().width(Length::Fill),
        iced::widget::pick_list(
            Some(draft),
            crate::chrome::controls::WindowControlsPlacement::ALL,
            |v: &crate::chrome::controls::WindowControlsPlacement| v.to_string(),
        )
        .graphite()
        .on_select(PrefMsg::DraftWindowControlsPlacement)
        .width(200),
    ]
    .align_y(iced::Alignment::Center)
    .boxed()
}
