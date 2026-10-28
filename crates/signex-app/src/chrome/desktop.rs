//! The window-button arrangement a Linux desktop asks for: GNOME's
//! `button-layout` setting and KDE's `kwinrc` decoration keys. The parsers
//! are pure; [`detect`] is the blocking IO shell around them, and it bounds
//! the `gsettings` child so a dead D-Bus session cannot hang it.

use super::controls::WindowButton;
use crate::ignore::IgnoreResult as _;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

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

/// How long `gsettings` may take before it is killed. It answers in
/// milliseconds; only a dead D-Bus session makes it stall.
const GSETTINGS_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// The probe child did not finish in time and was killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeTimedOut;

/// The running desktop's family, from `XDG_CURRENT_DESKTOP`.
pub fn current_family() -> DesktopFamily {
    desktop_family(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

/// Ask the running desktop (blocking: spawns `gsettings`, reads a file).
/// `Ok(None)` on desktops with no known source, which is normal.
pub fn detect() -> Result<Option<DesktopButtonLayout>, ProbeTimedOut> {
    match current_family() {
        DesktopFamily::Kde => Ok(detect_kde()),
        DesktopFamily::GnomeLike => detect_gnome(),
        DesktopFamily::Other => Ok(None),
    }
}

fn detect_gnome() -> Result<Option<DesktopButtonLayout>, ProbeTimedOut> {
    let mut command = Command::new("gsettings");
    command.args(["get", "org.gnome.desktop.wm.preferences", "button-layout"]);
    let Some(output) = output_within(command, GSETTINGS_TIMEOUT)? else {
        return Ok(None);
    };
    if !output.status.success() {
        return Ok(None);
    }
    Ok(parse_gnome_button_layout(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Run `command` and collect its stdout, killing it after `limit`.
/// `Ok(None)` when it could not be started or waited on, which callers treat
/// like any other unreadable source. Only for small-output children: `limit`
/// bounds the child's exit, not reading its stdout, so a grandchild that
/// inherits the pipe could hold the read open.
fn output_within(mut command: Command, limit: Duration) -> Result<Option<Output>, ProbeTimedOut> {
    let Ok(mut child) = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Ok(None);
    };
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(child.wait_with_output().ok()),
            Ok(None) if Instant::now() >= deadline => {
                kill_and_reap(&mut child);
                return Err(ProbeTimedOut);
            }
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(_) => {
                kill_and_reap(&mut child);
                return Ok(None);
            }
        }
    }
}

/// Best effort: the child may have exited since the last poll.
fn kill_and_reap(child: &mut Child) {
    child.kill().ignore("the probe child may already be gone");
    child.wait().ignore("reaping the killed probe child");
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

    #[cfg(unix)]
    #[test]
    fn a_stalled_probe_child_is_killed_at_the_deadline() {
        let mut stalled = Command::new("sleep");
        stalled.arg("30");
        let started = Instant::now();
        let result = output_within(stalled, Duration::from_millis(100));
        assert_eq!(result.map(|o| o.is_some()), Err(ProbeTimedOut));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn a_quick_probe_child_returns_its_output() {
        let mut quick = Command::new("echo");
        quick.arg("hi");
        let output = output_within(quick, Duration::from_secs(10))
            .expect("no timeout")
            .expect("echo runs");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hi");
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
