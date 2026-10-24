//! Format-version policy for `.snxsch` / `.snxpcb` tokens (`<kind>/<major>[.<minor>]`).
//!
//! * Same major, any minor: the file loads. A minor bump may only add
//!   keys / columns / blocks, which the unknown-key and unknown-column
//!   machinery carries through a save.
//! * Newer minor than this build: loads, but [`is_newer_minor`] is true so
//!   the caller can warn. The file's own (newer) token is kept on save --
//!   never stamped down to this build's minor -- so a save cannot make a
//!   newer file claim a schema it was not written in.
//! * Older minor: loads; the next save writes this build's token, because
//!   the content may now use features of the newer minor.
//! * Different major (or a different kind): refused with both versions named.

use super::FormatError;

/// `(major, minor)` of `token` if it is `<kind>/<major>[.<minor>]`.
fn parse(token: &str, kind: &str) -> Option<(u32, u32)> {
    let rest = token.strip_prefix(kind)?.strip_prefix('/')?;
    let (major, minor) = rest.split_once('.').unwrap_or((rest, "0"));
    Some((major.parse().ok()?, minor.parse().ok()?))
}

fn kind_of(current: &str) -> &str {
    current.split('/').next().unwrap_or(current)
}

/// Refuse a token whose kind or major differs from `current`.
pub(super) fn check(found: &str, current: &str) -> Result<(), FormatError> {
    let kind = kind_of(current);
    match (parse(found, kind), parse(current, kind)) {
        (Some((fm, _)), Some((cm, _))) if fm == cm => Ok(()),
        _ => Err(FormatError::UnsupportedVersion {
            found: found.to_string(),
            expected: current.to_string(),
        }),
    }
}

/// True when `found` is the same major as `current` with a newer minor.
pub(super) fn is_newer_minor(found: &str, current: &str) -> bool {
    let kind = kind_of(current);
    match (parse(found, kind), parse(current, kind)) {
        (Some((fmaj, fmin)), Some((cmaj, cmin))) => fmaj == cmaj && fmin > cmin,
        _ => false,
    }
}

/// The token to write: the file's own if newer, otherwise `current`.
pub(super) fn token_to_write(found: &str, current: &str) -> String {
    if is_newer_minor(found, current) {
        found.to_string()
    } else {
        current.to_string()
    }
}
