//! "Mouse wheel" preference: what a plain wheel or two-finger scroll does.

use iced::mouse::ScrollDelta;

use super::{Camera, ScrollIntent};

/// Preferences "Mouse wheel": the wheel zooms, or it scrolls the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelMode {
    /// The wheel zooms about the cursor (the default).
    #[default]
    Zoom,
    /// Plain wheel / two-finger scroll pans vertically, Shift+wheel pans
    /// horizontally, Ctrl+wheel and pinch zoom.
    Scroll,
}

impl WheelMode {
    /// Every mode, in the order the Preferences picker lists them.
    pub const ALL: [WheelMode; 2] = [WheelMode::Zoom, WheelMode::Scroll];

    /// Value stored under `mouse_wheel` in the preferences file.
    pub fn as_pref(self) -> &'static str {
        match self {
            WheelMode::Zoom => "zoom",
            WheelMode::Scroll => "scroll",
        }
    }

    /// Parse a stored value; anything unknown falls back to [`WheelMode::Zoom`].
    pub fn from_pref(value: &str) -> Self {
        match value {
            "scroll" => WheelMode::Scroll,
            _ => WheelMode::Zoom,
        }
    }
}

impl std::fmt::Display for WheelMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WheelMode::Zoom => "Zoom",
            WheelMode::Scroll => "Scroll",
        })
    }
}

/// Modifier keys that steer a wheel event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WheelKeys {
    /// Ctrl or Cmd held: the scroll zooms.
    pub zoom: bool,
    /// Shift held: in scroll mode the scroll pans horizontally.
    pub shift: bool,
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

/// Decide what a scroll event does under the "Mouse wheel" preference.
/// `Zoom` keeps [`Camera::scroll_intent`]. In `Scroll` mode, Ctrl/Cmd zooms
/// like `Zoom` mode; otherwise the event pans (a line delta is
/// `pixels_per_notch` pixels per notch), vertically, or horizontally with
/// Shift. Non-finite deltas become 0.
pub fn wheel_intent(
    delta: &ScrollDelta,
    pixels_per_notch: f32,
    trackpad_pan: bool,
    mode: WheelMode,
    keys: WheelKeys,
) -> ScrollIntent {
    if mode == WheelMode::Zoom || keys.zoom {
        return Camera::scroll_intent(delta, pixels_per_notch, trackpad_pan, keys.zoom);
    }
    let (x, y) = match *delta {
        ScrollDelta::Lines { x, y } => (finite(x) * pixels_per_notch, finite(y) * pixels_per_notch),
        ScrollDelta::Pixels { x, y } => (finite(x), finite(y)),
    };
    if keys.shift {
        ScrollIntent::Pan { dx: x + y, dy: 0.0 }
    } else {
        ScrollIntent::Pan { dx: x, dy: y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: WheelKeys = WheelKeys {
        zoom: false,
        shift: false,
    };

    #[test]
    fn zoom_mode_matches_scroll_intent() {
        let lines = ScrollDelta::Lines { x: 0.0, y: 1.0 };
        assert_eq!(
            wheel_intent(&lines, 50.0, false, WheelMode::Zoom, NONE),
            ScrollIntent::Zoom { notches: 1.0 }
        );
        let shift = WheelKeys {
            zoom: false,
            shift: true,
        };
        assert_eq!(
            wheel_intent(&lines, 50.0, false, WheelMode::Zoom, shift),
            ScrollIntent::Zoom { notches: 1.0 }
        );
    }

    #[test]
    fn scroll_mode_pans_vertically_and_shift_pans_horizontally() {
        let lines = ScrollDelta::Lines { x: 0.0, y: 2.0 };
        assert_eq!(
            wheel_intent(&lines, 50.0, false, WheelMode::Scroll, NONE),
            ScrollIntent::Pan { dx: 0.0, dy: 100.0 }
        );
        let shift = WheelKeys {
            zoom: false,
            shift: true,
        };
        assert_eq!(
            wheel_intent(&lines, 50.0, false, WheelMode::Scroll, shift),
            ScrollIntent::Pan { dx: 100.0, dy: 0.0 }
        );
        let pixels = ScrollDelta::Pixels { x: 3.0, y: -7.0 };
        assert_eq!(
            wheel_intent(&pixels, 50.0, false, WheelMode::Scroll, NONE),
            ScrollIntent::Pan { dx: 3.0, dy: -7.0 }
        );
    }

    #[test]
    fn scroll_mode_ctrl_wheel_still_zooms() {
        let keys = WheelKeys {
            zoom: true,
            shift: false,
        };
        let lines = ScrollDelta::Lines { x: 0.0, y: 1.0 };
        assert_eq!(
            wheel_intent(&lines, 50.0, false, WheelMode::Scroll, keys),
            ScrollIntent::Zoom { notches: 1.0 }
        );
        let pixels = ScrollDelta::Pixels { x: 0.0, y: 50.0 };
        assert_eq!(
            wheel_intent(&pixels, 50.0, false, WheelMode::Scroll, keys),
            ScrollIntent::Zoom { notches: 1.0 }
        );
    }

    #[test]
    fn scroll_mode_ignores_non_finite_deltas() {
        let pixels = ScrollDelta::Pixels {
            x: f32::NAN,
            y: f32::INFINITY,
        };
        assert_eq!(
            wheel_intent(&pixels, 50.0, false, WheelMode::Scroll, NONE),
            ScrollIntent::Pan { dx: 0.0, dy: 0.0 }
        );
    }

    #[test]
    fn the_pref_value_round_trips_and_unknown_means_zoom() {
        for mode in WheelMode::ALL {
            assert_eq!(WheelMode::from_pref(mode.as_pref()), mode);
        }
        assert_eq!(WheelMode::from_pref("sideways"), WheelMode::Zoom);
    }
}
