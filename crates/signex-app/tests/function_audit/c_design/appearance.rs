//! Row 144: Preferences > Appearance. Every control waits for Apply, Cancel
//! puts it back, and the theme and symbol grid style preview while drafted.

use signex_app::canvas::WheelMode;
use signex_app::chrome::controls::WindowControlsPlacement;
use signex_app::layer_strip_prefs::LayerSwitcher;
use signex_app::preferences::{PrefMsg, PrefNav};
use signex_app::render_config::{GridStyle, PinSelectionMode};
use signex_types::theme::{Color, SelectedSegmentLook, ThemeId, theme_tokens};

use super::support::{Prefs, opened};

fn other_theme(theme: ThemeId) -> ThemeId {
    match theme {
        ThemeId::GraphiteLight => ThemeId::GraphiteDark,
        ThemeId::GraphiteDark => ThemeId::GraphiteLight,
    }
}

fn other_grid(style: GridStyle) -> GridStyle {
    match style {
        GridStyle::Dots => GridStyle::Lines,
        _ => GridStyle::Dots,
    }
}

/// The saved value of every control on the page, in one comparable row each.
fn saved(prefs: &Prefs) -> Vec<String> {
    let ui = &prefs.app.ui_state;
    vec![
        format!("{:?}", ui.theme_id),
        ui.ui_font_name.clone(),
        format!("{:?}", ui.ui_scale),
        format!("{:?}", ui.selected_segment),
        format!("{:?}", ui.grid_style),
        format!("{:?}", ui.symbol_grid_style),
        format!("{:?}", ui.symbol_grid_size_mm),
        format!("{:?}", ui.symbol_pin_selection),
        format!("{:?}", ui.layer_switcher),
        format!("{:?}", ui.pcb_gpu_render),
        format!("{:?}", ui.antialiasing),
        format!("{:?}", ui.wheel_mode),
        format!("{:?}", ui.trackpad_pan),
        format!("{:?}", ui.zoom_step_percent),
        format!("{:?}", ui.window_controls_placement),
        format!("{:?}", ui.remember_window_geometry),
    ]
}

/// Move every control on the page to a value other than its saved one.
fn change_every_control(prefs: &mut Prefs) {
    let ui = &prefs.app.ui_state;
    let edits = vec![
        PrefMsg::DraftTheme(other_theme(ui.theme_id)),
        PrefMsg::DraftFont(format!("{} Audit", ui.ui_font_name)),
        PrefMsg::DraftUiScalePercent(
            if signex_app::ui_scale::scale_to_percent(ui.ui_scale) == 100 {
                110
            } else {
                100
            },
        ),
        PrefMsg::DraftSelectedSegment(match ui.selected_segment {
            SelectedSegmentLook::Neutral => SelectedSegmentLook::Accent,
            SelectedSegmentLook::Accent => SelectedSegmentLook::Neutral,
        }),
        PrefMsg::DraftGridStyle(other_grid(ui.grid_style)),
        PrefMsg::DraftSymbolGridStyle(other_grid(ui.symbol_grid_style)),
        PrefMsg::DraftSymbolGridSize(if ui.symbol_grid_size_mm == 1.27 {
            2.54
        } else {
            1.27
        }),
        PrefMsg::DraftSymbolPinSelection(match ui.symbol_pin_selection {
            PinSelectionMode::PinOnly => PinSelectionMode::TextAndPin,
            PinSelectionMode::TextAndPin => PinSelectionMode::PinOnly,
        }),
        PrefMsg::DraftLayerSwitcher(match ui.layer_switcher {
            LayerSwitcher::Strip => LayerSwitcher::Dropdown,
            LayerSwitcher::Dropdown => LayerSwitcher::Strip,
        }),
        PrefMsg::DraftPcbGpuRender(!ui.pcb_gpu_render),
        PrefMsg::DraftAntialiasing(!ui.antialiasing),
        PrefMsg::DraftWheelMode(match ui.wheel_mode {
            WheelMode::Zoom => WheelMode::Scroll,
            WheelMode::Scroll => WheelMode::Zoom,
        }),
        PrefMsg::DraftTrackpadPan(!ui.trackpad_pan),
        PrefMsg::DraftZoomStepPercent(if ui.zoom_step_percent == 25 { 10 } else { 25 }),
        PrefMsg::DraftWindowControlsPlacement(match ui.window_controls_placement {
            WindowControlsPlacement::Leading => WindowControlsPlacement::Trailing,
            _ => WindowControlsPlacement::Leading,
        }),
        PrefMsg::DraftRememberWindowGeometry(!ui.remember_window_geometry),
    ];
    for edit in edits {
        prefs.send(edit);
    }
}

#[test]
fn every_appearance_control_waits_for_apply_and_marks_only_its_page() {
    let mut prefs = opened();
    let before = saved(&prefs);

    change_every_control(&mut prefs);

    assert_eq!(saved(&prefs), before, "nothing is committed yet");
    assert_eq!(prefs.dirty_pages(), [PrefNav::Appearance]);
}

#[test]
fn apply_commits_every_appearance_control() {
    let mut prefs = opened();
    let before = saved(&prefs);
    change_every_control(&mut prefs);

    prefs.apply();

    let after = saved(&prefs);
    for (index, (old, new)) in before.iter().zip(&after).enumerate() {
        assert_ne!(old, new, "control {index} was not committed");
    }
    assert!(!prefs.is_dirty());
}

#[test]
fn cancel_puts_every_appearance_control_back() {
    let mut prefs = opened();
    let before = saved(&prefs);
    change_every_control(&mut prefs);

    prefs.cancel();

    assert_eq!(saved(&prefs), before);
    assert!(!prefs.is_dirty());
    let ui = &prefs.app.ui_state;
    assert_eq!(ui.preferences_draft_theme, ui.theme_id);
    assert_eq!(ui.preferences_draft_grid_style, ui.grid_style);
    assert_eq!(ui.preferences_draft_wheel_mode, ui.wheel_mode);
    assert_eq!(ui.preferences_draft_layer_switcher, ui.layer_switcher);
}

#[test]
fn a_theme_card_previews_at_once_and_cancel_takes_the_preview_back() {
    let mut prefs = opened();
    let saved_theme = prefs.app.ui_state.theme_id;
    let drafted = other_theme(saved_theme);

    prefs.send(PrefMsg::DraftTheme(drafted));

    assert_eq!(prefs.app.document_state.panel_ctx.theme_id, drafted);
    assert_eq!(prefs.app.ui_state.theme_id, saved_theme, "not committed");

    prefs.cancel();

    assert_eq!(prefs.app.document_state.panel_ctx.theme_id, saved_theme);
}

#[test]
fn the_symbol_grid_style_previews_at_once_and_cancel_takes_it_back() {
    let mut prefs = opened();
    let saved_style = prefs.app.ui_state.symbol_grid_style;
    let drafted = other_grid(saved_style);

    prefs.send(PrefMsg::DraftSymbolGridStyle(drafted));

    assert_eq!(
        prefs.app.document_state.panel_ctx.symbol_grid_style,
        drafted
    );

    prefs.cancel();

    assert_eq!(
        prefs.app.document_state.panel_ctx.symbol_grid_style,
        saved_style
    );
}

#[test]
fn selected_option_accent_recolours_the_chip_in_graphite_dark_after_apply() {
    let mut prefs = opened();
    prefs.send(PrefMsg::DraftTheme(ThemeId::GraphiteDark));
    prefs.send(PrefMsg::DraftSelectedSegment(SelectedSegmentLook::Neutral));
    prefs.apply();
    let chip = |prefs: &Prefs| {
        prefs
            .app
            .document_state
            .panel_ctx
            .tokens
            .extended
            .selected_segment
    };
    assert_eq!(chip(&prefs), Some(Color::from_hex("#3D4047")));

    prefs.send(PrefMsg::DraftSelectedSegment(SelectedSegmentLook::Accent));
    prefs.apply();

    assert_eq!(chip(&prefs), Some(Color::from_hex("#C2552D")));
}

#[test]
fn selected_option_leaves_graphite_light_unchanged() {
    let mut prefs = opened();
    prefs.send(PrefMsg::DraftTheme(ThemeId::GraphiteLight));
    let light = theme_tokens(ThemeId::GraphiteLight)
        .extended
        .selected_segment;

    for look in [SelectedSegmentLook::Accent, SelectedSegmentLook::Neutral] {
        prefs.send(PrefMsg::DraftSelectedSegment(look));
        prefs.apply();

        assert_eq!(
            prefs
                .app
                .document_state
                .panel_ctx
                .tokens
                .extended
                .selected_segment,
            light,
            "{look:?}"
        );
    }
}

#[test]
fn zoom_speed_stays_between_5_and_50_percent_per_notch() {
    let mut prefs = opened();

    prefs.send(PrefMsg::DraftZoomStepPercent(200));
    assert_eq!(prefs.app.ui_state.preferences_draft_zoom_step_percent, 50);

    prefs.send(PrefMsg::DraftZoomStepPercent(1));
    assert_eq!(prefs.app.ui_state.preferences_draft_zoom_step_percent, 5);
}

#[test]
fn interface_scale_keeps_to_80_to_200_percent_in_steps_of_10() {
    let mut prefs = opened();
    let draft = |prefs: &Prefs| prefs.app.ui_state.preferences_draft_ui_scale_percent;

    prefs.send(PrefMsg::DraftUiScalePercent(123));
    assert_eq!(draft(&prefs), 120);

    prefs.send(PrefMsg::DraftUiScalePercent(500));
    assert_eq!(draft(&prefs), 200);

    prefs.send(PrefMsg::DraftUiScalePercent(10));
    assert_eq!(draft(&prefs), 80);
}

#[test]
fn interface_scale_applies_to_the_window_on_apply() {
    let mut prefs = opened();
    let before = signex_app::ui_scale::scale_to_percent(prefs.app.ui_state.ui_scale);
    let wanted = if before == 150 { 130 } else { 150 };

    prefs.send(PrefMsg::DraftUiScalePercent(wanted));
    assert_eq!(
        signex_app::ui_scale::scale_to_percent(prefs.app.ui_state.ui_scale),
        before,
        "not before Apply"
    );
    prefs.apply();

    assert_eq!(
        signex_app::ui_scale::scale_to_percent(prefs.app.ui_state.ui_scale),
        wanted
    );
}
