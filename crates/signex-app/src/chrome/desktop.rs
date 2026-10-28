//! The window-button arrangement a Linux desktop asks for: GNOME's
//! `button-layout` setting and KDE's `kwinrc` decoration keys. The parsers
//! are pure; [`detect`] is the blocking IO shell around them.

use super::controls::WindowButton;

/// Which buttons the desktop lists on each side of the title bar. The first
/// entry of each list is nearest the window's left edge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopButtonLayout {
    pub leading: Vec<WindowButton>,
    pub trailing: Vec<WindowButton>,
}

impl DesktopButtonLayout {
    pub fn is_empty(&self) -> bool {
        self.leading.is_empty() && self.trailing.is_empty()
    }

    /// `None` when the desktop lists none of the buttons we draw.
    fn non_empty(self) -> Option<Self> {
        (!self.is_empty()).then_some(self)
    }
}

/// Parse GNOME's `org.gnome.desktop.wm.preferences button-layout`, e.g.
/// `'appmenu:minimize,maximize,close'`. Tokens other than the three buttons
/// (`appmenu`, `menu`, `icon`, `spacer`, ...) are ignored. Without a `:`
/// every button is on the left.
pub fn parse_gnome_button_layout(raw: &str) -> Option<DesktopButtonLayout> {
    let value = raw.trim().trim_matches(|c| c == '\'' || c == '"');
    let (left, right) = value.split_once(':').unwrap_or((value, ""));
    let side = |s: &str| -> Vec<WindowButton> {
        s.split(',')
            .filter_map(|token| match token.trim() {
                "close" => Some(WindowButton::Close),
                "minimize" => Some(WindowButton::Minimize),
                "maximize" => Some(WindowButton::Maximize),
                _ => None,
            })
            .collect()
    };
    DesktopButtonLayout {
        leading: side(left),
        trailing: side(right),
    }
    .non_empty()
}

const KDE_SECTION: &str = "org.kde.kdecoration2";
const KDE_DEFAULT_LEFT: &str = "MS";
const KDE_DEFAULT_RIGHT: &str = "HIAX";

/// Parse the `[org.kde.kdecoration2]` section of `kwinrc`. `X` is close,
/// `I` minimize, `A` maximize; every other letter is ignored. Absent keys
/// take KDE's defaults (`MS` left, `HIAX` right).
pub fn parse_kwinrc(text: &str) -> Option<DesktopButtonLayout> {
    let mut in_section = false;
    let mut left = None;
    let mut right = None;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = name == KDE_SECTION;
        } else if in_section && let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "ButtonsOnLeft" => left = Some(value.trim()),
                "ButtonsOnRight" => right = Some(value.trim()),
                _ => {}
            }
        }
    }
    let side = |codes: &str| -> Vec<WindowButton> {
        codes
            .chars()
            .filter_map(|c| match c {
                'X' => Some(WindowButton::Close),
                'I' => Some(WindowButton::Minimize),
                'A' => Some(WindowButton::Maximize),
                _ => None,
            })
            .collect()
    };
    DesktopButtonLayout {
        leading: side(left.unwrap_or(KDE_DEFAULT_LEFT)),
        trailing: side(right.unwrap_or(KDE_DEFAULT_RIGHT)),
    }
    .non_empty()
}

/// Which desktop's settings describe the title-bar buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopFamily {
    Kde,
    /// Desktops whose window manager honours GNOME's `button-layout`.
    GnomeLike,
    Other,
}

/// Classify `XDG_CURRENT_DESKTOP` (colon-separated, case-insensitive).
/// `gsettings` alone cannot tell desktops apart: it answers GNOME's schema
/// default on KDE and others too, so the desktop decides which source counts.
pub fn desktop_family(xdg_current_desktop: &str) -> DesktopFamily {
    let has = |wanted: &[&str]| {
        xdg_current_desktop
            .split(':')
            .any(|d| wanted.iter().any(|w| d.trim().eq_ignore_ascii_case(w)))
    };
    if has(&["KDE"]) {
        DesktopFamily::Kde
    } else if has(&["GNOME", "Unity", "Budgie", "Pantheon"]) {
        DesktopFamily::GnomeLike
    } else {
        DesktopFamily::Other
    }
}

/// Ask the running desktop (blocking: spawns `gsettings`, reads a file).
/// `None` on desktops with no known source, which is normal.
pub fn detect() -> Option<DesktopButtonLayout> {
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    match desktop_family(&current) {
        DesktopFamily::Kde => detect_kde(),
        DesktopFamily::GnomeLike => detect_gnome(),
        DesktopFamily::Other => None,
    }
}

fn detect_gnome() -> Option<DesktopButtonLayout> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_gnome_button_layout(&String::from_utf8_lossy(&output.stdout))
}

fn detect_kde() -> Option<DesktopButtonLayout> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
    let text = std::fs::read_to_string(base.join("kwinrc")).ok()?;
    parse_kwinrc(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use WindowButton::{Close, Maximize, Minimize};

    fn layout(leading: &[WindowButton], trailing: &[WindowButton]) -> Option<DesktopButtonLayout> {
        Some(DesktopButtonLayout {
            leading: leading.to_vec(),
            trailing: trailing.to_vec(),
        })
    }

    #[test]
    fn desktop_family_reads_the_colon_separated_list_case_insensitively() {
        use DesktopFamily::{GnomeLike, Kde, Other};
        for (raw, want) in [
            ("KDE", Kde),
            ("kde", Kde),
            ("GNOME", GnomeLike),
            ("ubuntu:GNOME", GnomeLike),
            ("Unity", GnomeLike),
            ("Budgie:GNOME", GnomeLike),
            ("Pantheon", GnomeLike),
            ("XFCE", Other),
            ("X-Cinnamon", Other),
            ("", Other),
        ] {
            assert_eq!(desktop_family(raw), want, "{raw:?}");
        }
    }

    #[test]
    fn gnome_appmenu_prefix_is_ignored() {
        assert_eq!(
            parse_gnome_button_layout("'appmenu:minimize,maximize,close'\n"),
            layout(&[], &[Minimize, Maximize, Close])
        );
    }

    #[test]
    fn gnome_left_side_buttons_keep_their_order() {
        assert_eq!(
            parse_gnome_button_layout("'close,minimize,maximize:'"),
            layout(&[Close, Minimize, Maximize], &[])
        );
    }

    #[test]
    fn gnome_close_only_layout_lists_just_close() {
        assert_eq!(parse_gnome_button_layout("':close'"), layout(&[], &[Close]));
        assert_eq!(
            parse_gnome_button_layout("\"icon,spacer:close\""),
            layout(&[], &[Close])
        );
    }

    #[test]
    fn gnome_without_a_colon_is_all_leading() {
        assert_eq!(
            parse_gnome_button_layout("close,maximize"),
            layout(&[Close, Maximize], &[])
        );
    }

    #[test]
    fn gnome_empty_or_garbage_is_none() {
        for raw in [
            "",
            "''",
            "':'",
            "appmenu:",
            "garbage",
            "'menu:spacer'",
            "\u{0}",
        ] {
            assert_eq!(parse_gnome_button_layout(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn kwinrc_letter_codes_map_to_buttons_and_others_are_ignored() {
        let rc = "[General]\nButtonsOnLeft=XYZ\n\n[org.kde.kdecoration2]\nButtonsOnLeft=MS\nButtonsOnRight=HIAX\n";
        assert_eq!(parse_kwinrc(rc), layout(&[], &[Minimize, Maximize, Close]));
        let swapped = "[org.kde.kdecoration2]\nButtonsOnLeft=XIA\nButtonsOnRight=\n";
        assert_eq!(
            parse_kwinrc(swapped),
            layout(&[Close, Minimize, Maximize], &[])
        );
    }

    #[test]
    fn kwinrc_absent_keys_take_kde_defaults() {
        let expected = layout(&[], &[Minimize, Maximize, Close]);
        assert_eq!(parse_kwinrc(""), expected);
        assert_eq!(
            parse_kwinrc("[org.kde.kdecoration2]\nBorderSize=Normal\n"),
            expected
        );
        let only_left = "[org.kde.kdecoration2]\nButtonsOnLeft=X\n";
        assert_eq!(
            parse_kwinrc(only_left),
            layout(&[Close], &[Minimize, Maximize, Close])
        );
    }

    #[test]
    fn kwinrc_with_no_known_buttons_is_none() {
        let rc = "[org.kde.kdecoration2]\nButtonsOnLeft=MS\nButtonsOnRight=H\n";
        assert_eq!(parse_kwinrc(rc), None);
    }
}
