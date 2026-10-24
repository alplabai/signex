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

use std::path::{Path, PathBuf};

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

fn is_backslash(c: char) -> bool {
    c == '\\'
}

/// The server and share named by the leading segments of `rest`, lowercased.
///
/// `strict` is the verbatim `\\?\UNC\` form: Windows does not rewrite `/` there,
/// so only `\` separates segments, an empty segment is not skipped, and a
/// segment containing `/` is part of a name the OS will not see as the share
/// it looks like, so it is never trusted. The non-verbatim forms accept both
/// separators and skip empty segments, as Win32 does.
fn share_from(rest: &str, strict: bool) -> Option<Share> {
    let share = |server: &str, share: &str| {
        let ok = |s: &str| !s.is_empty() && !s.contains('/');
        (ok(server) && ok(share)).then(|| Share {
            server: server.to_lowercase(),
            share: share.to_lowercase(),
        })
    };
    if strict {
        let mut segments = rest.strip_prefix('\\')?.split(is_backslash);
        return share(segments.next()?, segments.next()?);
    }
    let mut segments = rest.split(is_separator).filter(|s| !s.is_empty());
    share(segments.next()?, segments.next()?)
}

/// `rest` after a leading `UNC` segment (`UNC\server\share` -> `\server\share`).
fn after_unc_segment(rest: &str, sep: fn(char) -> bool) -> Option<&str> {
    let head = rest.get(..3)?;
    let tail = &rest[3..];
    (head.eq_ignore_ascii_case("UNC") && tail.starts_with(sep)).then_some(tail)
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

/// Classify what follows a leading pair of separators; `backslashes` is true
/// when that pair was `\\`.
fn classify_double_separator(after: &str, backslashes: bool) -> ReferenceKind {
    let Some(marker) = after.chars().next().filter(|c| matches!(c, '.' | '?')) else {
        return ReferenceKind::Network(share_from(after, false));
    };
    let rest = &after[1..];
    if !rest.starts_with(is_separator) {
        return ReferenceKind::Network(share_from(after, false));
    }
    // `\\.\` is the device namespace; `\\?\` is verbatim, and only its UNC
    // spelling names a share. Windows treats the prefix as verbatim only when
    // it is spelled with backslashes alone, and then leaves `/` untouched in
    // the rest of the path; a mixed prefix is refused as a device.
    let strict = marker == '?' && backslashes && rest.starts_with('\\');
    if marker == '?' && !strict && (backslashes || rest.starts_with('\\')) {
        return ReferenceKind::Device;
    }
    let rest = if strict {
        &rest[1..]
    } else {
        rest.trim_start_matches(is_separator)
    };
    if marker == '?' && is_verbatim_disk(rest) {
        return ReferenceKind::Local;
    }
    let sep: fn(char) -> bool = if strict { is_backslash } else { is_separator };
    match after_unc_segment(rest, sep) {
        Some(unc) if marker == '?' => ReferenceKind::Network(share_from(unc, strict)),
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
        return classify_double_separator(&text[2..], a == '\\' && b == '\\');
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

/// Add the share of each project directory's real (canonical) spelling to
/// `trusted`. A project opened through a mapped drive, DFS or a symlink to a
/// share is spelled locally, but its children resolve to `\\?\UNC\srv\share`;
/// that share is the project's own. `canonical` does the IO and is only called
/// for a directory that is not itself refused.
pub(crate) fn trust_real_shares(
    trusted: &mut Vec<Share>,
    dirs: &[PathBuf],
    canonical: impl Fn(&Path) -> Option<PathBuf>,
) {
    let seed = trusted.clone();
    for dir in dirs.iter().filter(|d| !path_is_refused(d, &seed)) {
        if let Some(share) = canonical(dir).and_then(|real| share_of(&real))
            && !trusted.contains(&share)
        {
            trusted.push(share);
        }
    }
}

/// True when the walk must not follow `reference` or the `normalised` path it
/// resolves to: either spelling is a device or an untrusted network name.
/// A `..` cannot leave a share: `normalize_clamped` stops it at the prefix.
pub(crate) fn is_walk_refused(reference: &str, normalised: &Path, trusted: &[Share]) -> bool {
    is_refused(reference, trusted) || path_is_refused(normalised, trusted)
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
    fn the_normalised_path_is_classified_too() {
        let trusted = trusting(r"\\srv\share\proj\p.snxprj");
        // Raw text looks local; the normalised path is on another share.
        assert!(is_walk_refused(
            "a.snxsch",
            Path::new("//other/share/a.snxsch"),
            &trusted
        ));
        assert!(!is_walk_refused(
            "../b/a.snxsch",
            Path::new("//srv/share/b/a.snxsch"),
            &trusted
        ));
    }

    #[test]
    fn a_slash_in_a_verbatim_unc_server_or_share_is_never_trusted() {
        let trusted = trusting(r"\\trusted\share\proj\p.snxprj");

        for reference in [
            r"\\?\UNC\trusted/share\evil.snxsch",
            r"\\?\UNC\trusted\share/x\y",
            r"\\?\UNC\trusted/share/evil.snxsch",
            r"\\?\UNC\\trusted\share\x",
            // Mixed verbatim prefixes are not a spelling Windows normalises.
            r"\\?/UNC\trusted\share\x",
            r"//?\UNC\trusted\share\x",
        ] {
            assert!(is_refused(reference, &trusted), "{reference}");
        }
        for reference in [
            r"\\?\UNC\trusted\share\x",
            r"\\?\UNC\TRUSTED\Share\a\b.snxsch",
            // All-slash prefix: Win32 rewrites every `/` first.
            "//?/UNC/trusted/share/x",
        ] {
            assert!(!is_refused(reference, &trusted), "{reference}");
        }
    }

    #[test]
    fn a_project_spelled_locally_trusts_the_share_its_real_path_is_on() {
        // `Z:\proj` is a mapped drive; canonicalising it gives the verbatim UNC.
        let real = |dir: &Path| {
            (dir == Path::new(r"Z:\proj")).then(|| PathBuf::from(r"\\?\UNC\Srv\Share\proj"))
        };
        let mut trusted = Vec::new();
        trust_real_shares(&mut trusted, &[PathBuf::from(r"Z:\proj")], real);

        let child = Path::new(r"\\?\UNC\srv\share\proj\a.snxsch");
        assert!(!is_walk_refused("a.snxsch", child, &trusted));
        assert!(is_walk_refused(
            "a.snxsch",
            Path::new(r"\\?\UNC\evil\share\a.snxsch"),
            &trusted
        ));
        assert!(is_walk_refused(
            "a.snxsch",
            Path::new(r"\\?\UNC\srv\other\a.snxsch"),
            &trusted
        ));
    }

    #[test]
    fn a_refused_project_dir_is_never_canonicalised() {
        let mut trusted = Vec::new();
        let dirs = [PathBuf::from(r"\\attacker\share\proj")];
        trust_real_shares(&mut trusted, &dirs, |_| panic!("canonicalised"));
        assert!(trusted.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn std_agrees_on_what_a_verbatim_unc_share_is() {
        use std::path::{Component, Prefix};
        let prefix = |s: &str| match Path::new(s).components().next() {
            Some(Component::Prefix(p)) => match p.kind() {
                Prefix::VerbatimUNC(server, share) => Some((
                    server.to_string_lossy().into_owned(),
                    share.to_string_lossy().into_owned(),
                )),
                _ => None,
            },
            _ => None,
        };
        // std sees the whole `trusted/share` as the server, so it must not be trusted.
        assert_eq!(
            prefix(r"\\?\UNC\trusted/share\evil.snxsch").map(|(server, _)| server),
            Some("trusted/share".to_string())
        );
        assert_eq!(
            prefix(r"\\?\UNC\trusted\share/x\y").map(|(_, share)| share),
            Some("share/x".to_string())
        );
        assert_eq!(
            share_of(Path::new(r"\\?\UNC\trusted\share\x")),
            Some(Share {
                server: "trusted".into(),
                share: "share".into()
            })
        );
    }

    #[cfg(windows)]
    #[test]
    fn unc_dotdot_is_clamped_at_the_share_on_windows() {
        let trusted = trusting(r"\\trusted\share\proj\p.snxprj");
        let dir = Path::new(r"\\?\UNC\trusted\share\proj");
        let deep = format!("{}other\\share\\x", r"..\".repeat(30));
        for reference in [r"\\trusted\share\..\..\other\share\x", deep.as_str()] {
            let next =
                crate::app::project_sheets::ownership_reference(dir, reference).expect("non-empty");
            assert!(!is_walk_refused(reference, &next, &trusted), "{reference}");
            assert_eq!(share_of(&next), share_of(dir), "{reference}: {next:?}");
        }
    }
}
