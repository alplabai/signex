//! Which window-control buttons the main window's strip shows, where, and in
//! which style. Pure: the platform, the saved placement and the detected
//! desktop layout go in, a [`WindowControlsLayout`] comes out.

use super::desktop::DesktopButtonLayout;

/// One window-control button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowButton {
    Close,
    Minimize,
    Maximize,
}

/// The persisted "Window controls" preference (`window_controls_placement`
/// in `prefs.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowControlsPlacement {
    /// Follow the platform (macOS left, Windows right, Linux per desktop).
    #[default]
    Auto,
    Leading,
    Trailing,
}

impl WindowControlsPlacement {
    pub const ALL: &'static [WindowControlsPlacement] = &[
        WindowControlsPlacement::Auto,
        WindowControlsPlacement::Leading,
        WindowControlsPlacement::Trailing,
    ];

    /// The `prefs.json` token.
    pub fn token(self) -> &'static str {
        match self {
            WindowControlsPlacement::Auto => "auto",
            WindowControlsPlacement::Leading => "leading",
            WindowControlsPlacement::Trailing => "trailing",
        }
    }

    /// Parse a `prefs.json` token; anything unknown is [`Self::Auto`].
    pub fn from_token(token: &str) -> Self {
        match token.trim().to_ascii_lowercase().as_str() {
            "leading" => WindowControlsPlacement::Leading,
            "trailing" => WindowControlsPlacement::Trailing,
            _ => WindowControlsPlacement::Auto,
        }
    }
}

impl std::fmt::Display for WindowControlsPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowControlsPlacement::Auto => write!(f, "Automatic (follow the system)"),
            WindowControlsPlacement::Leading => write!(f, "Left"),
            WindowControlsPlacement::Trailing => write!(f, "Right"),
        }
    }
}

/// How the buttons are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlsStyle {
    /// The OS draws the traffic lights (macOS, decorations on); the strip
    /// only reserves room for them.
    MacNative,
    /// Traffic-light circles drawn by the strip (macOS, controls on the right).
    MacDrawn,
    /// 46 px rectangular buttons with a red close hover.
    Windows,
    /// Small round buttons in the libadwaita manner.
    Linux,
}

/// The OS family the layout is resolved for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromePlatform {
    MacOs,
    Windows,
    Linux,
}

impl ChromePlatform {
    /// The platform this binary runs on; every target that is neither macOS
    /// nor Windows counts as Linux (the BSDs follow the same desktops).
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            ChromePlatform::MacOs
        } else if cfg!(target_os = "windows") {
            ChromePlatform::Windows
        } else {
            ChromePlatform::Linux
        }
    }
}

/// The resolved strip layout. In both lists the first entry is the one
/// nearest the window's left edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowControlsLayout {
    pub style: WindowControlsStyle,
    pub leading: Vec<WindowButton>,
    pub trailing: Vec<WindowButton>,
}

impl WindowControlsLayout {
    fn leading(style: WindowControlsStyle, buttons: &[WindowButton]) -> Self {
        Self {
            style,
            leading: buttons.to_vec(),
            trailing: Vec::new(),
        }
    }

    fn trailing(style: WindowControlsStyle, buttons: &[WindowButton]) -> Self {
        Self {
            style,
            leading: Vec::new(),
            trailing: buttons.to_vec(),
        }
    }

    /// Whether the OS decorates the window (and draws the buttons itself).
    pub fn is_native(&self) -> bool {
        self.style == WindowControlsStyle::MacNative
    }
}

use WindowButton::{Close, Maximize, Minimize};

/// Close sits at the window corner whichever side the buttons are on.
const TRAILING_ALL: [WindowButton; 3] = [Minimize, Maximize, Close];
const LEADING_WINDOWS: [WindowButton; 3] = [Close, Maximize, Minimize];
const LEADING_LINUX: [WindowButton; 3] = [Close, Minimize, Maximize];

/// Resolve the layout. `detected` only matters for [`ChromePlatform::Linux`]
/// with [`WindowControlsPlacement::Auto`].
pub fn resolve(
    platform: ChromePlatform,
    placement: WindowControlsPlacement,
    detected: Option<&DesktopButtonLayout>,
) -> WindowControlsLayout {
    use WindowControlsPlacement::{Auto, Leading, Trailing};
    match (platform, placement) {
        (ChromePlatform::MacOs, Auto | Leading) => {
            WindowControlsLayout::leading(WindowControlsStyle::MacNative, &[])
        }
        (ChromePlatform::MacOs, Trailing) => {
            WindowControlsLayout::trailing(WindowControlsStyle::MacDrawn, &TRAILING_ALL)
        }
        (ChromePlatform::Windows, Auto | Trailing) => {
            WindowControlsLayout::trailing(WindowControlsStyle::Windows, &TRAILING_ALL)
        }
        (ChromePlatform::Windows, Leading) => {
            WindowControlsLayout::leading(WindowControlsStyle::Windows, &LEADING_WINDOWS)
        }
        (ChromePlatform::Linux, Auto) => match detected.filter(|d| !d.is_empty()) {
            Some(d) => WindowControlsLayout {
                style: WindowControlsStyle::Linux,
                leading: d.leading.clone(),
                trailing: d.trailing.clone(),
            },
            None => WindowControlsLayout::trailing(WindowControlsStyle::Linux, &TRAILING_ALL),
        },
        (ChromePlatform::Linux, Leading) => {
            WindowControlsLayout::leading(WindowControlsStyle::Linux, &LEADING_LINUX)
        }
        (ChromePlatform::Linux, Trailing) => {
            WindowControlsLayout::trailing(WindowControlsStyle::Linux, &TRAILING_ALL)
        }
    }
}

/// The placement the running window actually follows. macOS decides between
/// native and drawn buttons when the window is created, so a changed
/// preference only shows after a restart; elsewhere it applies at once.
pub fn effective_placement(
    platform: ChromePlatform,
    saved: WindowControlsPlacement,
    at_boot: WindowControlsPlacement,
) -> WindowControlsPlacement {
    match platform {
        ChromePlatform::MacOs => at_boot,
        ChromePlatform::Windows | ChromePlatform::Linux => saved,
    }
}

/// Whether switching `shown` (what the window runs with) to `draft` needs a
/// restart on `platform`.
pub fn needs_restart(
    platform: ChromePlatform,
    shown: WindowControlsPlacement,
    draft: WindowControlsPlacement,
) -> bool {
    let style = |p| resolve(platform, p, None).style;
    platform == ChromePlatform::MacOs && style(shown) != style(draft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ChromePlatform::{Linux, MacOs, Windows};
    use WindowControlsPlacement::{Auto, Leading, Trailing};

    fn layout(
        style: WindowControlsStyle,
        leading: &[WindowButton],
        trailing: &[WindowButton],
    ) -> WindowControlsLayout {
        WindowControlsLayout {
            style,
            leading: leading.to_vec(),
            trailing: trailing.to_vec(),
        }
    }

    #[test]
    fn macos_auto_and_leading_use_the_native_lights() {
        for p in [Auto, Leading] {
            let r = resolve(MacOs, p, None);
            assert_eq!(r, layout(WindowControlsStyle::MacNative, &[], &[]));
            assert!(r.is_native());
        }
    }

    #[test]
    fn macos_trailing_draws_the_lights_on_the_right() {
        let r = resolve(MacOs, Trailing, None);
        assert_eq!(r, layout(WindowControlsStyle::MacDrawn, &[], &TRAILING_ALL));
        assert!(!r.is_native());
    }

    #[test]
    fn windows_auto_and_trailing_put_close_at_the_right_corner() {
        for p in [Auto, Trailing] {
            assert_eq!(
                resolve(Windows, p, None),
                layout(
                    WindowControlsStyle::Windows,
                    &[],
                    &[Minimize, Maximize, Close]
                )
            );
        }
    }

    #[test]
    fn windows_leading_puts_close_at_the_left_corner() {
        assert_eq!(
            resolve(Windows, Leading, None),
            layout(
                WindowControlsStyle::Windows,
                &[Close, Maximize, Minimize],
                &[]
            )
        );
    }

    #[test]
    fn linux_auto_respects_exactly_what_the_desktop_lists() {
        let gnome_default = DesktopButtonLayout {
            leading: vec![],
            trailing: vec![Close],
        };
        assert_eq!(
            resolve(Linux, Auto, Some(&gnome_default)),
            layout(WindowControlsStyle::Linux, &[], &[Close])
        );
        let split = DesktopButtonLayout {
            leading: vec![Close, Minimize],
            trailing: vec![Maximize],
        };
        assert_eq!(
            resolve(Linux, Auto, Some(&split)),
            layout(WindowControlsStyle::Linux, &[Close, Minimize], &[Maximize])
        );
    }

    #[test]
    fn linux_auto_falls_back_to_the_right_when_detection_gave_nothing() {
        let fallback = layout(WindowControlsStyle::Linux, &[], &TRAILING_ALL);
        assert_eq!(resolve(Linux, Auto, None), fallback);
        let empty = DesktopButtonLayout::default();
        assert_eq!(resolve(Linux, Auto, Some(&empty)), fallback);
    }

    #[test]
    fn linux_overrides_ignore_the_detected_layout() {
        let detected = DesktopButtonLayout {
            leading: vec![Close],
            trailing: vec![],
        };
        assert_eq!(
            resolve(Linux, Trailing, Some(&detected)),
            layout(WindowControlsStyle::Linux, &[], &TRAILING_ALL)
        );
        assert_eq!(
            resolve(Linux, Leading, None),
            layout(
                WindowControlsStyle::Linux,
                &[Close, Minimize, Maximize],
                &[]
            )
        );
    }

    #[test]
    fn only_macos_style_changes_need_a_restart() {
        assert!(needs_restart(MacOs, Auto, Trailing));
        assert!(needs_restart(MacOs, Trailing, Leading));
        assert!(!needs_restart(MacOs, Auto, Leading));
        for p in [Windows, Linux] {
            assert!(!needs_restart(p, Auto, Trailing));
        }
    }

    #[test]
    fn macos_follows_the_boot_placement_elsewhere_the_saved_one() {
        assert_eq!(effective_placement(MacOs, Trailing, Auto), Auto);
        assert_eq!(effective_placement(Windows, Trailing, Auto), Trailing);
        assert_eq!(effective_placement(Linux, Leading, Auto), Leading);
    }

    #[test]
    fn placement_tokens_round_trip_and_unknown_means_auto() {
        for &p in WindowControlsPlacement::ALL {
            assert_eq!(WindowControlsPlacement::from_token(p.token()), p);
        }
        assert_eq!(WindowControlsPlacement::from_token("LEADING"), Leading);
        assert_eq!(WindowControlsPlacement::from_token("sideways"), Auto);
        assert_eq!(WindowControlsPlacement::from_token(""), Auto);
    }
}
