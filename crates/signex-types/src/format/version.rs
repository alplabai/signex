//! Format-version policy for `.snxsch` / `.snxpcb` tokens (`<kind>/<major>[.<minor>]`).
//!
//! * Same major, any minor: the file loads (for viewing).
//! * Newer minor than this build: loads, but the document remembers its
//!   source token and `write_string` refuses to save it -- enum parsers fall
//!   back silently and many nested structs have no unknown-field carrier,
//!   so a save would drop content. [`is_newer_minor`] lets callers warn.
//! * Older minor: loads; the next save writes this build's token.
//! * Different major (or a different kind): refused with both versions named.

use super::FormatError;

/// `(major, minor)` of `token` if it is `<kind>/<major>[.<minor>]`.
fn parse(token: &str, kind: &str) -> Option<(u32, u32)> {
    let rest = token.strip_prefix(kind)?.strip_prefix('/')?;
    let (major, minor) = rest.split_once('.').unwrap_or((rest, "0"));
    Some((number(major)?, number(minor)?))
}

/// ASCII digits only, no leading zero except "0" (no `+`, no `007`).
fn number(s: &str) -> Option<u32> {
    let plain =
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'));
    if plain { s.parse().ok() } else { None }
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

/// Refuse to save a document read from a newer minor than this build.
pub(super) fn refuse_save(source: &str, current: &str) -> Result<(), FormatError> {
    if is_newer_minor(source, current) {
        return Err(FormatError::NewerMinorReadOnly {
            found: source.to_string(),
            current: current.to_string(),
        });
    }
    Ok(())
}
