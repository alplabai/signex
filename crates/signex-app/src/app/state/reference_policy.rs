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
    /// `\\.\`, `\\?\` (non-UNC), `\??\`, or any other non-drive prefix.
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
pub(crate) fn path_is_refused(path: &Path, trusted: &[Share]) -> bool {
    is_refused(&path.to_string_lossy(), trusted)
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
            r"\\?\C:\x.snxsch",
            r"\\?\srv\share\x",
            r"\??\C:\x.snxsch",
            "/??/C:/x.snxsch",
        ] {
            assert!(is_refused(reference, &trusted), "{reference}");
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
}
