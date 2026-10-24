//! Which sheet references the ownership walk may follow (#771).
//!
//! References live inside sheet files nobody vouches for. A network name read
//! on Windows sends the user's NTLM credentials to the named host, and a
//! device name reaches hardware, so neither is followed unless the user has
//! already put themselves there: a UNC reference on the SAME server and share
//! as an open project is allowed (that is the project's own sheets and its
//! same-share references). A different host or share, and every device or
//! verbatim-device form, stays refused and is never read or canonicalised.
//!
//! The decision is on the text, so every host answers alike. That includes
//! Unix, where a leading `//host/share` is treated as UNC too (POSIX leaves
//! `//` implementation-defined): a `//`-rooted project there trusts its own
//! share by the same rule instead of being refused.

use std::path::Path;

/// A UNC server and share, compared without case.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Share {
    server: String,
    share: String,
}

/// What the text of a reference names.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReferenceKind {
    /// A drive, relative or rooted local path.
    Local,
    /// `\\server\share\..`, `//server/share/..` or `\\?\UNC\server\share\..`.
    /// `None` when the server or share is missing, which is never trusted.
    Network(Option<Share>),
    /// `\\.\`, `\\?\` (non-UNC, non-disk), `\??\`, or any other non-drive prefix.
    Device,
}

fn is_separator(c: char) -> bool {
    matches!(c, '/' | '\\')
}

/// The server and share named by the leading segments of `rest`, lowercased.
fn share_from(rest: &str) -> Option<Share> {
    let mut segments = rest.split(is_separator).filter(|s| !s.is_empty());
    Some(Share {
        server: segments.next()?.to_lowercase(),
        share: segments.next()?.to_lowercase(),
    })
}

/// `rest` after a leading `UNC` segment (`UNC\server\share` -> `\server\share`).
fn after_unc_segment(rest: &str) -> Option<&str> {
    let head = rest.get(..3)?;
    let tail = &rest[3..];
    (head.eq_ignore_ascii_case("UNC") && tail.starts_with(is_separator)).then_some(tail)
}

/// True for the verbatim-disk spelling `C:\..` (what `canonicalize` returns on
/// Windows once `\\?\` is stripped): the same local disk as `C:\..`, so it is
/// followed. A drive letter, a colon, then a separator or the end.
fn is_verbatim_disk(rest: &str) -> bool {
    let mut chars = rest.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(d), Some(':'), None) | (Some(d), Some(':'), Some('/' | '\\')) if d.is_ascii_alphabetic()
    )
}

/// Classify what follows a leading pair of separators.
fn classify_double_separator(after: &str) -> ReferenceKind {
    let Some(marker) = after.chars().next().filter(|c| matches!(c, '.' | '?')) else {
        return ReferenceKind::Network(share_from(after));
    };
    let rest = &after[1..];
    if !rest.starts_with(is_separator) {
        return ReferenceKind::Network(share_from(after));
    }
    // `\\.\` is the device namespace; `\\?\` is verbatim, and only its UNC
    // spelling names a share.
    let rest = rest.trim_start_matches(is_separator);
    if marker == '?' && is_verbatim_disk(rest) {
        return ReferenceKind::Local;
    }
    match after_unc_segment(rest) {
        Some(unc) if marker == '?' => ReferenceKind::Network(share_from(unc)),
        _ => ReferenceKind::Device,
    }
}

fn classify(reference: &str) -> ReferenceKind {
    let text = reference.trim();
    if text.starts_with("\\??\\") || text.starts_with("/??/") {
        return ReferenceKind::Device;
    }
    let mut chars = text.chars();
    if let (Some(a), Some(b)) = (chars.next(), chars.next())
        && is_separator(a)
        && is_separator(b)
    {
        return classify_double_separator(&text[2..]);
    }
    use std::path::{Component, Prefix};
    match Path::new(text).components().next() {
        Some(Component::Prefix(p)) if !matches!(p.kind(), Prefix::Disk(_)) => ReferenceKind::Device,
        _ => ReferenceKind::Local,
    }
}

/// The UNC share `path` is on, or `None` for a local path.
pub(crate) fn share_of(path: &Path) -> Option<Share> {
    match classify(&path.to_string_lossy()) {
        ReferenceKind::Network(share) => share,
        ReferenceKind::Local | ReferenceKind::Device => None,
    }
}

/// True when `reference` must not be canonicalised or read: a device, or a
/// network name outside every share in `trusted`.
pub(crate) fn is_refused(reference: &str, trusted: &[Share]) -> bool {
    match classify(reference) {
        ReferenceKind::Local => false,
        ReferenceKind::Device => true,
        ReferenceKind::Network(share) => share.is_none_or(|s| !trusted.contains(&s)),
    }
}

/// [`is_refused`] for a path that has not been normalised yet.
///
/// On Windows a reserved device name is refused too (#999): the classifier
/// calls `C:\proj\NUL` local, but `GetFullPathNameW` maps it to `\\.\NUL`, so
/// a canonicalise or read would open the device.
pub(crate) fn path_is_refused(path: &Path, trusted: &[Share]) -> bool {
    is_refused(&path.to_string_lossy(), trusted) || names_windows_device(path)
}

/// True when `name` is a Windows reserved device name: `NUL`, `CON`, `PRN`,
/// `AUX`, `COM1`-`COM9`, `LPT1`-`LPT9` (also with a superscript digit),
/// `CONIN$`, `CONOUT$`. Windows ignores case, any extension (everything from
/// the first dot) and spaces before it, so `nul.snxsch` and `aux .snxsch` are
/// the devices. Host independent: a pure name test.
pub(crate) fn is_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end_matches(' ');
    let upper = stem.to_uppercase();
    match upper.as_str() {
        "NUL" | "CON" | "PRN" | "AUX" | "CONIN$" | "CONOUT$" => true,
        _ => {
            let mut chars = upper.chars();
            let head: String = chars.by_ref().take(3).collect();
            matches!(head.as_str(), "COM" | "LPT")
                && matches!(
                    chars.next(),
                    Some('1'..='9' | '\u{00B9}' | '\u{00B2}' | '\u{00B3}')
                )
                && chars.next().is_none()
        }
    }
}

/// True when `path` would open a Windows device: its lexical absolute form
/// (no IO) is a `\\.\` device path, or its last component is a reserved
/// device name. Always false off Windows, where these are ordinary files.
#[cfg(windows)]
fn names_windows_device(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    let device_prefix = std::path::absolute(path).is_ok_and(|abs| {
        matches!(
            abs.components().next(),
            Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::DeviceNS(_))
        )
    });
    device_prefix
        || path
            .file_name()
            .is_some_and(|n| is_reserved_device_name(&n.to_string_lossy()))
}

#[cfg(not(windows))]
fn names_windows_device(_path: &Path) -> bool {
    false
}

/// True when `text` names a path on a UNC share whose `..` segments climb
/// above the share root. The OS clamps such a `..` at the root, but a reader
/// that resolved it differently could land on another share, so the walk
/// refuses the reference instead of trusting the clamp.
fn climbs_above_share(text: &str) -> bool {
    if !matches!(classify(text), ReferenceKind::Network(Some(_))) {
        return false;
    }
    let mut segments = text.trim().split(is_separator).filter(|s| !s.is_empty());
    let mut head = segments.next();
    if head == Some("?") {
        segments.next(); // the `UNC` marker
        head = segments.next();
    }
    // `head` is the server, the next segment the share.
    if head.is_none() || segments.next().is_none() {
        return false;
    }
    let mut depth = 0usize;
    for segment in segments {
        match segment {
            "." => {}
            ".." => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return true,
            },
            _ => depth += 1,
        }
    }
    false
}

/// True when the walk must not follow `reference` (read from a sheet in
/// `dir`) or the `normalised` path it resolves to: either spelling is a
/// device or an untrusted network name, or the join climbs above a share.
pub(crate) fn is_walk_refused(
    dir: &Path,
    reference: &str,
    normalised: &Path,
    trusted: &[Share],
) -> bool {
    is_refused(reference, trusted)
        || path_is_refused(normalised, trusted)
        || climbs_above_share(&dir.join(reference.trim()).to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trusting(project: &str) -> Vec<Share> {
        vec![share_of(Path::new(project)).expect("a UNC project path")]
    }

    #[test]
    fn a_reference_on_the_projects_own_share_is_allowed_without_regard_to_case() {
        let trusted = trusting(r"\\Srv\Share\proj\p.snxprj");

        for reference in [
            r"\\srv\share\proj\a.snxsch",
            r"\\SRV\SHARE\other\b.snxsch",
            "//srv/share/c.snxsch",
            r"\\?\UNC\srv\share\d.snxsch",
            r"  \\srv\share\padded.snxsch  ",
        ] {
            assert!(!is_refused(reference, &trusted), "{reference}");
        }
    }

    #[test]
    fn another_host_or_another_share_stays_refused() {
        let trusted = trusting(r"\\srv\share\proj\p.snxprj");

        for reference in [
            r"\\attacker\share\x.snxsch",
            r"\\srv\other\x.snxsch",
            "//attacker/share/x.snxsch",
            r"\\?\UNC\attacker\share\x.snxsch",
            r"\\srv",
            r"\\srv\share2\x",
        ] {
            assert!(is_refused(reference, &trusted), "{reference}");
        }
    }

    #[test]
    fn a_network_name_is_refused_when_no_open_project_is_on_a_share() {
        assert!(is_refused(r"\\srv\share\x.snxsch", &[]));
        assert!(is_refused("//srv/share/x.snxsch", &[]));
    }

    #[test]
    fn device_and_verbatim_device_forms_are_refused_even_on_a_trusted_share() {
        let trusted = trusting(r"\\srv\share\proj\p.snxprj");

        for reference in [
            r"\\.\COM1",
            r"\\.\srv\share\x",
            r"\\?\GLOBALROOT\Device\x",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\x",
            r"\\?\C",
            r"\\?\C:x",
            r"\\?\srv\share\x",
            r"\??\C:\x.snxsch",
            "/??/C:/x.snxsch",
        ] {
            assert!(is_refused(reference, &trusted), "{reference}");
        }
    }

    #[test]
    fn a_verbatim_disk_path_is_the_same_local_disk_and_is_followed() {
        let trusted = trusting(r"\\srv\share\proj\p.snxprj");

        for reference in [
            r"\\?\C:\proj\p.snxprj",
            r"\\?\c:\x.snxsch",
            "//?/C:/x.snxsch",
            r"\\?\C:",
            r"  \\?\D:\padded.snxsch ",
        ] {
            assert!(!is_refused(reference, &[]), "{reference}");
            assert!(!is_refused(reference, &trusted), "{reference}");
            assert_eq!(share_of(Path::new(reference.trim())), None, "{reference}");
        }
    }

    #[test]
    fn local_relative_and_absolute_references_are_followed() {
        for reference in [
            "child.snxsch",
            "../other/x.snxsch",
            "/work/x.snxsch",
            "./x.snxsch",
            r"C:\work\x.snxsch",
            "C:/work/x.snxsch",
            "/dev/zero",
        ] {
            assert!(!is_refused(reference, &[]), "{reference}");
        }
    }

    #[test]
    fn a_local_path_has_no_share() {
        assert_eq!(share_of(Path::new("/work/p.snxprj")), None);
        assert_eq!(share_of(Path::new(r"C:\work\p.snxprj")), None);
    }

    #[test]
    fn a_double_slash_rooted_project_trusts_its_own_share_on_every_host() {
        let trusted = trusting("//srv/share/proj/p.snxprj");

        assert_eq!(
            share_of(Path::new("//srv/share/proj/p.snxprj")),
            share_of(Path::new(r"\\SRV\Share\x")),
        );
        assert!(!path_is_refused(
            Path::new("//srv/share/proj/a.snxsch"),
            &trusted
        ));
        assert!(path_is_refused(
            Path::new("//srv/other/proj/a.snxsch"),
            &trusted
        ));
    }

    #[test]
    fn a_dotdot_climbing_above_the_share_root_is_detected() {
        assert!(climbs_above_share(r"\\srv\share\a\..\..\x"));
        assert!(climbs_above_share("//srv/share/../x"));
        assert!(climbs_above_share(r"\\?\UNC\srv\share\..\x"));
        assert!(!climbs_above_share(r"\\srv\share\a\..\x"));
        assert!(!climbs_above_share(r"\\srv\share"));
        assert!(!climbs_above_share("a/../../x"));
    }

    #[test]
    fn the_normalised_path_is_classified_too() {
        let trusted = trusting(r"\\srv\share\proj\p.snxprj");
        let dir = Path::new("//srv/share/proj");
        // Raw text looks local; the normalised path is on another share.
        assert!(is_walk_refused(
            dir,
            "a.snxsch",
            Path::new("//other/share/a.snxsch"),
            &trusted
        ));
        // A `..` inside the share is fine.
        assert!(!is_walk_refused(
            dir,
            "../b/a.snxsch",
            Path::new("//srv/share/b/a.snxsch"),
            &trusted
        ));
        // A `..` above the share root is refused although clamping would keep
        // the result on the trusted share.
        assert!(is_walk_refused(
            dir,
            "../../../other/share/x",
            Path::new("//srv/share/other/share/x"),
            &trusted
        ));
    }

    #[cfg(windows)]
    #[test]
    fn unc_dotdot_cannot_escape_to_another_share_on_windows() {
        let trusted = trusting(r"\\trusted\share\proj\p.snxprj");
        let dir = Path::new(r"\\trusted\share\proj");
        for reference in [
            r"\\trusted\share\..\..\other\share\x",
            r"..\..\..\other\share\x",
        ] {
            let next =
                crate::app::project_sheets::ownership_reference(dir, reference).expect("non-empty");
            assert!(
                is_walk_refused(dir, reference, &next, &trusted),
                "{reference}"
            );
        }
        let inside = crate::app::project_sheets::ownership_reference(dir, r"..\b\x.snxsch")
            .expect("non-empty");
        assert!(!is_walk_refused(dir, r"..\b\x.snxsch", &inside, &trusted));
    }

    #[test]
    fn the_device_name_matcher_ignores_case_extension_and_trailing_spaces() {
        for name in [
            "NUL",
            "nul",
            "nul.snxsch",
            "COM1.txt",
            "com9",
            "aux .snxsch",
            "LPT9.snxsch",
            "CON",
            "prn.",
            "CONIN$",
            "COM\u{00B9}",
            "NUL.tar.gz",
        ] {
            assert!(is_reserved_device_name(name), "{name}");
        }
        for name in [
            "sheet.snxsch",
            "COM0",
            "COM10",
            "LPT",
            "COM",
            "nulls.snxsch",
            "xnul",
            "aux1",
            "",
            ".nul",
        ] {
            assert!(!is_reserved_device_name(name), "{name}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_device_names_are_refused_before_any_io() {
        for path in [
            r"C:\proj\NUL",
            "NUL",
            r"C:\proj\nul.snxsch",
            r"C:\proj\COM1.txt",
            r"C:\proj\aux .snxsch",
            r"C:\proj\folder\LPT9.snxsch",
            "COM1",
        ] {
            assert!(path_is_refused(Path::new(path), &[]), "{path}");
        }
        for path in [
            r"C:\proj\sheet.snxsch",
            r"C:\proj\NUL\sheet.snxsch",
            "main.snxsch",
        ] {
            assert!(!path_is_refused(Path::new(path), &[]), "{path}");
        }
    }
}
