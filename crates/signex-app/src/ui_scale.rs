//! User interface scale (#1228): one global multiplier laid on top of the OS
//! display scale, through iced's `scale_factor` hook.
//!
//! iced multiplies it into every window's viewport scale, so text, spacing,
//! borders and dialog sizes grow together, and the cursor, resize and
//! window events it hands the app are already in the scaled logical pixels.
//! The app does no physical-to-logical conversion of its own.
//!
//! The one place the OS factor and this one must be combined by hand is the
//! wordmark tier, because `iced::window::scale_factor` reports the OS factor
//! alone ([`render_scale`]).

/// Smallest interface scale, as a percent.
pub const UI_SCALE_MIN_PERCENT: u16 = 80;
/// Largest interface scale, as a percent.
pub const UI_SCALE_MAX_PERCENT: u16 = 200;
/// Distance between the choices in Preferences, as a percent.
pub const UI_SCALE_STEP_PERCENT: u16 = 10;
/// No extra scaling.
pub const UI_SCALE_DEFAULT: f32 = 1.0;

/// How a stored `ui_scale` value was read.
#[derive(Debug, Clone, PartialEq)]
pub struct UiScaleRead {
    /// The scale to use: always within 0.8..=2.0.
    pub scale: f32,
    /// Why the stored value was not used as written; `None` when it was.
    /// Meant for the Messages panel.
    pub problem: Option<String>,
}

/// Read the `ui_scale` value of `prefs.json`. A missing key is the default
/// and not a problem. A number outside 0.8..=2.0 is clamped; anything else
/// (text, null, NaN, zero, negative) falls back to 1.0. Both are reported.
pub fn parse_ui_scale(stored: Option<&serde_json::Value>) -> UiScaleRead {
    let Some(value) = stored else {
        return UiScaleRead {
            scale: UI_SCALE_DEFAULT,
            problem: None,
        };
    };
    let min = UI_SCALE_MIN_PERCENT as f64 / 100.0;
    let max = UI_SCALE_MAX_PERCENT as f64 / 100.0;
    match value.as_f64().filter(|v| v.is_finite() && *v > 0.0) {
        Some(v) if (min..=max).contains(&v) => UiScaleRead {
            scale: v as f32,
            problem: None,
        },
        Some(v) => {
            let clamped = v.clamp(min, max) as f32;
            UiScaleRead {
                scale: clamped,
                problem: Some(format!(
                    "ui_scale {v} in prefs.json is outside {min}..={max}; using {clamped}"
                )),
            }
        }
        None => UiScaleRead {
            scale: UI_SCALE_DEFAULT,
            problem: Some(format!(
                "ui_scale {value} in prefs.json is not a positive number; using {UI_SCALE_DEFAULT}"
            )),
        },
    }
}

/// The percent shown and edited in Preferences for a stored scale.
pub fn scale_to_percent(scale: f32) -> u16 {
    (scale * 100.0).round() as u16
}

/// The scale for a Preferences percent: snapped to the 10 % steps and
/// clamped to 80..=200 %, never stored raw.
pub fn percent_to_scale(percent: u16) -> f32 {
    snap_percent(percent) as f32 / 100.0
}

/// The scale Save commits. A draft still equal to the percent of the
/// current scale keeps the current scale as it is, so saving some other
/// preference never rewrites a hand-edited off-step value such as 1.25.
pub fn committed_scale(current: f32, draft_percent: u16) -> f32 {
    if scale_to_percent(current) == draft_percent {
        current
    } else {
        percent_to_scale(draft_percent)
    }
}

/// The value written to `prefs.json`: rounded to two decimals so an f32
/// does not land as `1.100000023841858`.
pub fn scale_for_file(scale: f32) -> f64 {
    (scale as f64 * 100.0).round() / 100.0
}

/// `percent` snapped to the nearest step within the allowed range.
pub fn snap_percent(percent: u16) -> u16 {
    let step = UI_SCALE_STEP_PERCENT;
    let snapped = (percent + step / 2) / step * step;
    snapped.clamp(UI_SCALE_MIN_PERCENT, UI_SCALE_MAX_PERCENT)
}

/// Device pixels per logical pixel in a window: the OS factor times the
/// interface scale. `iced::window::scale_factor` returns the OS factor only,
/// so the wordmark's image tier has to be picked from this product.
pub fn render_scale(os_scale: f32, ui_scale: f32) -> f32 {
    os_scale * ui_scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_value_is_the_default_without_a_report() {
        let read = parse_ui_scale(None);
        assert_eq!(read.scale, 1.0);
        assert_eq!(read.problem, None);
    }

    #[test]
    fn an_in_range_value_is_kept() {
        let read = parse_ui_scale(Some(&json!(1.5)));
        assert_eq!((read.scale, read.problem), (1.5, None));
    }

    #[test]
    fn an_out_of_range_value_is_clamped_and_reported() {
        let low = parse_ui_scale(Some(&json!(0.1)));
        let high = parse_ui_scale(Some(&json!(9)));
        assert_eq!(low.scale, 0.8);
        assert_eq!(high.scale, 2.0);
        assert!(low.problem.is_some() && high.problem.is_some());
    }

    #[test]
    fn a_malformed_value_falls_back_to_one_and_is_reported() {
        for bad in [json!("big"), json!(null), json!(0), json!(-1.0), json!([1])] {
            let read = parse_ui_scale(Some(&bad));
            assert_eq!(read.scale, 1.0, "{bad}");
            assert!(read.problem.is_some(), "{bad}");
        }
    }

    #[test]
    fn percent_snaps_to_steps_inside_the_range() {
        assert_eq!(snap_percent(0), 80);
        assert_eq!(snap_percent(104), 100);
        assert_eq!(snap_percent(105), 110);
        assert_eq!(snap_percent(999), 200);
        assert_eq!(scale_to_percent(percent_to_scale(150)), 150);
    }

    #[test]
    fn an_unchanged_draft_keeps_an_off_step_scale() {
        // Hand-edited 1.25 shows as draft 125; saving something else must
        // not snap it to 1.3.
        assert_eq!(committed_scale(1.25, 125), 1.25);
        assert_eq!(committed_scale(1.25, 130), 1.3);
        assert_eq!(committed_scale(1.0, 150), 1.5);
    }

    #[test]
    fn the_file_value_has_no_float_noise() {
        assert_eq!(scale_for_file(1.1_f32), 1.1_f64);
        assert_eq!(serde_json::json!(scale_for_file(1.7)).to_string(), "1.7");
    }

    #[test]
    fn main_registers_the_scale_factor_hook_on_the_daemon() {
        // The daemon builder lives in `main`, which no test can call; this
        // pins that the registration is still there.
        assert!(include_str!("main.rs").contains(".scale_factor(Signex::scale_factor)"));
    }

    #[test]
    fn the_wordmark_scale_is_the_product_of_both_factors() {
        // 100 % OS scale at 200 % interface scale draws at 2 device px per
        // logical px, which is the 2x wordmark tier, not the 1x one.
        assert_eq!(render_scale(1.0, 2.0), 2.0);
        assert_eq!(crate::menu_bar::wordmark_tier(render_scale(1.0, 2.0)), 2);
        assert_eq!(crate::menu_bar::wordmark_tier(1.0), 1);
        assert_eq!(crate::menu_bar::wordmark_tier(render_scale(1.5, 1.5)), 3);
    }
}

#[cfg(test)]
mod prefs_file_tests {
    use crate::fonts::{read_ui_scale_pref_at, write_ui_scale_pref_at};

    fn prefs_in(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("prefs.json")
    }

    #[test]
    fn the_scale_round_trips_through_prefs_json() {
        let dir = tempfile::Builder::new()
            .prefix("snx-1228-rt")
            .tempdir()
            .unwrap();
        let path = prefs_in(&dir);
        write_ui_scale_pref_at(&path, 1.7);
        assert_eq!(read_ui_scale_pref_at(&path), 1.7);
    }

    #[test]
    fn a_missing_file_is_the_default_and_silent() {
        let dir = tempfile::Builder::new()
            .prefix("snx-1228-none")
            .tempdir()
            .unwrap();
        let path = prefs_in(&dir);
        let before = crate::diagnostics::logged_count("snx-1228-none");
        assert_eq!(read_ui_scale_pref_at(&path), 1.0);
        assert_eq!(crate::diagnostics::logged_count("snx-1228-none"), before);
    }

    #[test]
    fn an_out_of_range_value_clamps_and_reaches_the_messages_panel() {
        let dir = tempfile::Builder::new()
            .prefix("snx-1228-hi")
            .tempdir()
            .unwrap();
        let path = prefs_in(&dir);
        std::fs::write(&path, br#"{"ui_scale": 7.5}"#).unwrap();
        let before = crate::diagnostics::logged_count("snx-1228-hi");
        assert_eq!(read_ui_scale_pref_at(&path), 2.0);
        assert_eq!(crate::diagnostics::logged_count("snx-1228-hi"), before + 1);
    }

    #[test]
    fn a_malformed_value_falls_back_to_one_and_reaches_the_messages_panel() {
        let dir = tempfile::Builder::new()
            .prefix("snx-1228-bad")
            .tempdir()
            .unwrap();
        let path = prefs_in(&dir);
        std::fs::write(&path, br#"{"ui_scale": "huge"}"#).unwrap();
        let before = crate::diagnostics::logged_count("snx-1228-bad");
        assert_eq!(read_ui_scale_pref_at(&path), 1.0);
        assert_eq!(crate::diagnostics::logged_count("snx-1228-bad"), before + 1);
    }
}
