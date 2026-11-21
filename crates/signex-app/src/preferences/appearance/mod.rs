//! Appearance page (mockup row 144): Look, Canvas, Navigation and Window. Each control
//! sends the draft message it sent before the page was regrouped; the theme
//! cards and the grid preview show the drafts live. The fast grids moved to
//! the Editing page (mockup row 147).

pub(in crate::preferences) mod grid_preview;

use super::pages_view::PreviewKind;
use super::pages_view::controls::{readout, segmented_choice, slider_style, toggle};
use super::setting_row::{
    SettingRow, help_text, page, page_heading, section_header, setting_row, tone_pill,
};
use super::{PrefMsg, PrefsView};
use crate::canvas::WheelMode;
use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZES_MM};
use crate::choice::{Choice, chosen};
use crate::chrome::controls::WindowControlsPlacement;
use crate::render_config::{GridStyle, PinSelectionMode};
use crate::styles::{self, PillTone, tokens};
use iced::widget::{column, pick_list, row, slider};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text;

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Width of the caption before each grid style picker (56 px).
const GRID_CAPTION_W: f32 = TS.body * 14.0 / 3.0;

pub(super) fn content_appearance<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    if v.pages_view.expanded_preview() == Some(PreviewKind::Grid) {
        return page(vec![grid_preview::grid_preview(v, true)]);
    }
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let section = |id: MessageId| section_header(t!(i18n, id), None, t);
    page(vec![
        page_heading(
            t!(i18n, ids::PREFERENCES_APPEARANCE_HEADING),
            t!(i18n, ids::PREFERENCES_THEME_INTRO),
            t,
        ),
        section(ids::PREFERENCES_APPEARANCE_SECTION_LOOK),
        super::theme_cards::theme_cards(v.draft_theme, t, i18n),
        font_row(v),
        ui_scale_row(v),
        selected_segment_row(v),
        section(ids::PREFERENCES_APPEARANCE_SECTION_CANVAS),
        grid_style_row(v),
        grid_preview::grid_preview(v, false),
        symbol_grid_row(v),
        pin_selection_row(v),
        gpu_render_row(v),
        antialiasing_row(v),
        section(ids::PREFERENCES_APPEARANCE_SECTION_NAVIGATION),
        wheel_mode_row(v),
        trackpad_pan_row(v),
        zoom_speed_row(v),
        layer_switcher_row(v),
        section(ids::PREFERENCES_APPEARANCE_SECTION_WINDOW),
        window_buttons_row(v),
        remember_geometry_row(v),
    ])
}

/// The catalog name of a grid style.
pub(super) fn grid_style_label(style: GridStyle) -> MessageId {
    match style {
        GridStyle::Dots => ids::PREFERENCES_GRID_STYLE_DOTS,
        GridStyle::Lines => ids::PREFERENCES_GRID_STYLE_LINES,
        GridStyle::SmallCrosses => ids::PREFERENCES_GRID_STYLE_SMALL_CROSSES,
    }
}

/// One setting row with a plain help text.
fn row_with<'a>(
    label: Localized,
    pills: Vec<Element<'a, PrefMsg>>,
    help: Localized,
    control: Element<'a, PrefMsg>,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    setting_row(
        SettingRow {
            label,
            pills,
            help: Some(help_text(help, t)),
            control,
            error: None,
        },
        t,
    )
}

/// The pill of a setting that applies at the next start.
fn restart_pill<'a>(t: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    tone_pill(t!(i18n, ids::PREFERENCES_PILL_RESTART), PillTone::Warn, t)
}

fn font_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let families: &'static [String] = crate::fonts::system_font_families().as_slice();
    let picker = pick_list(Some(v.draft_font.to_string()), families, |s: &String| {
        s.clone()
    })
    .graphite()
    .on_select(PrefMsg::DraftFont)
    .padding([SP.s, SP.m])
    .width(Length::Fill);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_UI_FONT),
        vec![restart_pill(t, i18n)],
        t!(i18n, ids::PREFERENCES_APPEARANCE_UI_FONT_SHORT_HELP),
        picker.boxed(),
        t,
    )
}

fn ui_scale_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let percent = v.draft_ui_scale_percent;
    let control = row![
        slider(
            crate::ui_scale::UI_SCALE_MIN_PERCENT..=crate::ui_scale::UI_SCALE_MAX_PERCENT,
            percent,
            PrefMsg::DraftUiScalePercent,
        )
        .step(crate::ui_scale::UI_SCALE_STEP_PERCENT)
        .width(Length::Fill)
        .style(slider_style(t)),
        readout(
            t!(i18n, ids::PREFERENCES_APPEARANCE_UI_SCALE_VALUE, "percent" => u32::from(percent)),
            t,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_UI_SCALE),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_APPEARANCE_UI_SCALE_HELP),
        control.boxed(),
        t,
    )
}

/// Grid style: a captioned picker per editor, one above the other.
fn grid_style_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let choices = Choice::named(GridStyle::ALL, grid_style_label, i18n);
    let control = column![
        style_picker(
            t!(i18n, ids::PREFERENCES_APPEARANCE_GRID_STYLE_SCHEMATIC),
            v.draft_grid_style,
            choices.clone(),
            PrefMsg::DraftGridStyle,
            t,
        ),
        style_picker(
            t!(i18n, ids::PREFERENCES_APPEARANCE_GRID_STYLE_SYMBOL),
            v.draft_symbol_grid_style,
            choices,
            PrefMsg::DraftSymbolGridStyle,
            t,
        ),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_GRID_STYLE_LABEL),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_APPEARANCE_GRID_STYLE_HELP),
        control.boxed(),
        t,
    )
}

fn style_picker<'a>(
    caption: Localized,
    current: GridStyle,
    choices: Vec<Choice<GridStyle>>,
    on_select: fn(GridStyle) -> PrefMsg,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let ink4 = tokens::resolve(t).ink4;
    row![
        text(caption)
            .size(TS.caption)
            .color(ink4)
            .width(Length::Fixed(GRID_CAPTION_W)),
        pick_list(
            chosen(&choices, &current),
            choices,
            |choice: &Choice<GridStyle>| choice.label.clone(),
        )
        .graphite()
        .on_select(move |choice: Choice<GridStyle>| on_select(choice.value))
        .width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The symbol editor's default grid: the grid presets, picked by label.
fn symbol_grid_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let current = GRID_SIZES_MM
        .iter()
        .zip(GRID_SIZE_LABELS.iter())
        .find(|(mm, _)| (**mm - v.draft_symbol_grid_size_mm).abs() < 0.001)
        .map(|(_, label)| *label);
    let picker = pick_list(current, GRID_SIZE_LABELS, |label: &&'static str| {
        label.to_string()
    })
    .graphite()
    .on_select(|label: &'static str| {
        let mm = GRID_SIZES_MM
            .iter()
            .zip(GRID_SIZE_LABELS.iter())
            .find(|(_, l)| **l == label)
            .map(|(mm, _)| *mm)
            .unwrap_or(1.27);
        PrefMsg::DraftSymbolGridSize(mm)
    })
    .width(Length::Fill);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_SYMBOL_GRID_LABEL),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_APPEARANCE_SYMBOL_GRID_HELP),
        picker.boxed(),
        t,
    )
}

fn pin_selection_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let control = segmented_choice(
        vec![
            (
                PinSelectionMode::PinOnly,
                t!(i18n, ids::PREFERENCES_PIN_SELECTION_PIN_ONLY),
            ),
            (
                PinSelectionMode::TextAndPin,
                t!(i18n, ids::PREFERENCES_PIN_SELECTION_TEXT_AND_PIN),
            ),
        ],
        v.draft_symbol_pin_selection,
        t,
        PrefMsg::DraftSymbolPinSelection,
    );
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_PIN_SELECTION_LABEL),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_APPEARANCE_PIN_SELECTION_SHORT_HELP),
        control,
        t,
    )
}

fn gpu_render_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    // "Experimental" stays while GPU parity is unconfirmed on hardware; the
    // known gaps are closed (see the `scene::order` parity tests).
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_GPU_RENDER_LABEL),
        vec![tone_pill(
            t!(i18n, ids::PREFERENCES_PILL_EXPERIMENTAL),
            PillTone::Info,
            t,
        )],
        t!(i18n, ids::PREFERENCES_APPEARANCE_GPU_RENDER_HELP),
        toggle(v.draft_pcb_gpu_render, t, PrefMsg::DraftPcbGpuRender),
        t,
    )
}

fn antialiasing_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_ANTIALIASING),
        vec![restart_pill(t, i18n)],
        t!(i18n, ids::PREFERENCES_APPEARANCE_ANTIALIASING_SHORT_HELP),
        toggle(v.draft_antialiasing, t, PrefMsg::DraftAntialiasing),
        t,
    )
}

fn wheel_mode_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let control = segmented_choice(
        vec![
            (WheelMode::Zoom, t!(i18n, ids::PREFERENCES_WHEEL_MODE_ZOOM)),
            (
                WheelMode::Scroll,
                t!(i18n, ids::PREFERENCES_WHEEL_MODE_SCROLL),
            ),
        ],
        v.draft_wheel_mode,
        t,
        PrefMsg::DraftWheelMode,
    );
    row_with(
        t!(i18n, ids::PREFERENCES_WHEEL_MODE_LABEL),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_WHEEL_MODE_SHORT_HELP),
        control,
        t,
    )
}

fn layer_switcher_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    use crate::layer_strip_prefs::LayerSwitcher;
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let control = segmented_choice(
        vec![
            (
                LayerSwitcher::Strip,
                t!(i18n, ids::PREFERENCES_LAYER_SWITCHER_STRIP),
            ),
            (
                LayerSwitcher::Dropdown,
                t!(i18n, ids::PREFERENCES_LAYER_SWITCHER_DROPDOWN),
            ),
        ],
        v.draft_layer_switcher,
        t,
        PrefMsg::DraftLayerSwitcher,
    );
    row_with(
        t!(i18n, ids::PREFERENCES_LAYER_SWITCHER_LABEL),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_LAYER_SWITCHER_HELP),
        control,
        t,
    )
}

fn selected_segment_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    use signex_types::theme::SelectedSegmentLook;
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let control = segmented_choice(
        vec![
            (
                SelectedSegmentLook::Neutral,
                t!(i18n, ids::PREFERENCES_SELECTED_SEGMENT_NEUTRAL),
            ),
            (
                SelectedSegmentLook::Accent,
                t!(i18n, ids::PREFERENCES_SELECTED_SEGMENT_ACCENT),
            ),
        ],
        v.draft_selected_segment,
        t,
        PrefMsg::DraftSelectedSegment,
    );
    row_with(
        t!(i18n, ids::PREFERENCES_SELECTED_SEGMENT_LABEL),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_SELECTED_SEGMENT_HELP),
        control,
        t,
    )
}

fn trackpad_pan_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_TRACKPAD_PAN),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_APPEARANCE_TRACKPAD_PAN_SHORT_HELP),
        toggle(v.draft_trackpad_pan, t, PrefMsg::DraftTrackpadPan),
        t,
    )
}

fn zoom_speed_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let percent = v.draft_zoom_step_percent;
    let control = row![
        slider(
            crate::canvas::Camera::ZOOM_STEP_PERCENT_MIN
                ..=crate::canvas::Camera::ZOOM_STEP_PERCENT_MAX,
            percent,
            PrefMsg::DraftZoomStepPercent,
        )
        .width(Length::Fill)
        .style(slider_style(t)),
        readout(
            t!(
                i18n,
                ids::PREFERENCES_APPEARANCE_ZOOM_STEP_VALUE,
                "percent" => u32::from(percent)
            ),
            t,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_ZOOM_SPEED),
        Vec::new(),
        t!(i18n, ids::PREFERENCES_APPEARANCE_ZOOM_SPEED_HELP),
        control.boxed(),
        t,
    )
}

/// Where the minimize / maximize / close buttons go, with the restart note
/// when the drafted choice changes how macOS creates the window.
fn window_buttons_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let mut help = column![help_text(
        t!(i18n, ids::PREFERENCES_APPEARANCE_WINDOW_BUTTONS_HELP),
        t
    )]
    .spacing(SP.xs);
    if v.window_controls_restart_note {
        help = help.push(help_text(
            t!(i18n, ids::PREFERENCES_APPEARANCE_RESTART_NOTE),
            t,
        ));
    }
    let picker = pick_list(
        Some(v.draft_window_controls_placement),
        WindowControlsPlacement::ALL,
        |placement: &WindowControlsPlacement| placement.to_string(),
    )
    .graphite()
    .on_select(PrefMsg::DraftWindowControlsPlacement)
    .width(Length::Fill);
    setting_row(
        SettingRow {
            label: t!(i18n, ids::PREFERENCES_APPEARANCE_WINDOW_BUTTONS),
            pills: Vec::new(),
            help: Some(help.boxed()),
            control: picker.boxed(),
            error: None,
        },
        t,
    )
}

fn remember_geometry_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    row_with(
        t!(i18n, ids::PREFERENCES_APPEARANCE_REMEMBER_GEOMETRY),
        Vec::new(),
        t!(
            i18n,
            ids::PREFERENCES_APPEARANCE_REMEMBER_GEOMETRY_SHORT_HELP
        ),
        toggle(
            v.draft_remember_window_geometry,
            t,
            PrefMsg::DraftRememberWindowGeometry,
        ),
        t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid style names read as the `Display` names the pickers showed.
    #[test]
    fn grid_style_names_keep_their_english() {
        let en = I18n::en_us();
        let names: Vec<String> = GridStyle::ALL
            .iter()
            .map(|&style| en.message_plain(grid_style_label(style), &[]))
            .collect();
        let shown: Vec<String> = GridStyle::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(names, shown);
    }
}
