//! How a path is turned into a comparison key: which case rule and which
//! separator rule apply (#694).
//!
//! The two rules are independent facts. Whether `A` and `a` are one name is a
//! property of the VOLUME (probed, see `volume_case`); whether `\` separates
//! path components is a property of the HOST OS (a backslash is an ordinary
//! filename character on macOS and Linux, even on a volume that folds case).
//!
//! Tests: `app/state/case_fold_tests.rs`, next to `path_key_with`.

/// The case and separator rules `app::state::scope::path_key_with` applies.
///
/// An injectable input rather than a `cfg!(windows)` read inside the key
/// function, so every combination runs on every host, including the
/// Linux-only `cargo test` CI job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaseFold {
    /// `A.snxsch` and `a.snxsch` are one file on this volume.
    pub fold_case: bool,
    /// `\` separates path components (the Windows host rule).
    pub backslash_is_separator: bool,
    /// `é` as one code point (NFC) and as `e` + a combining accent (NFD) are
    /// one name on this volume (APFS). NTFS keeps them apart, so this is off
    /// on a Windows host and on a volume that preserves case.
    pub normalize_unicode: bool,
}

impl Default for CaseFold {
    /// The host default, for a project whose volume is not probed yet.
    fn default() -> Self {
        Self::host_default()
    }
}

impl CaseFold {
    /// The Windows-host rules: case folded, `\` is a separator.
    #[cfg(test)]
    pub(crate) const FOLD: Self = Self {
        fold_case: true,
        backslash_is_separator: true,
        normalize_unicode: false,
    };
    /// The POSIX-host rules on a case-sensitive volume: the path verbatim.
    #[cfg(test)]
    pub(crate) const PRESERVE: Self = Self {
        fold_case: false,
        backslash_is_separator: false,
        normalize_unicode: false,
    };

    /// What the host family implies when the volume has not been probed (or
    /// cannot be): `FOLD` on Windows, `PRESERVE`
    /// elsewhere.
    pub(crate) const fn host_default() -> Self {
        Self::from_probe(cfg!(windows))
    }

    /// The rules for a volume whose probe answered `ignores_case`. Only the
    /// case rule follows the probe; the separator rule stays host-derived.
    /// Unicode normalisation is enabled on a macOS host only, where the
    /// case-insensitive volume is APFS / HFS+ (both normalise); FAT / exFAT
    /// volumes (Linux, or mounted on macOS) keep NFC and NFD apart, and the
    /// probe cannot tell them from APFS, so the conservative answer is off.
    pub(crate) const fn from_probe(ignores_case: bool) -> Self {
        Self {
            fold_case: ignores_case,
            backslash_is_separator: cfg!(windows),
            normalize_unicode: ignores_case && cfg!(target_os = "macos"),
        }
    }

    /// Case folded, backslash an ordinary character: the APFS shape.
    #[cfg(test)]
    pub(crate) const fn case_only() -> Self {
        Self {
            fold_case: true,
            backslash_is_separator: false,
            normalize_unicode: false,
        }
    }

    /// Case folded and Unicode-normalised, backslash ordinary: APFS as the
    /// probe reports it on a macOS host.
    #[cfg(test)]
    pub(crate) const fn apfs() -> Self {
        Self {
            fold_case: true,
            backslash_is_separator: false,
            normalize_unicode: true,
        }
    }
}
