//! The `.snxprj` format tokens and the rule that picks the token a save
//! writes. Policy: `docs/FORMAT_VERSIONS.md`.

use super::ProjectData;
use crate::format_version::{FormatVersion, FormatVersionError, MissingToken, NewerMinor};

/// The `.snxprj` token of a project without variant settings. A file
/// without a token predates it and reads as this.
pub const SNXPRJ_FORMAT_V1: &str = "snxprj/1";

/// Written instead of [`SNXPRJ_FORMAT_V1`] when the project has variant
/// settings (`variant_settings`, mockup row 32), DECISIONS 2026-11-25: a
/// build that knows only `snxprj/1` would keep the section but not follow
/// a variant rename or removal, so it opens 1.1 read-only. A project
/// without them stays `snxprj/1`, byte for byte.
pub const SNXPRJ_FORMAT_V1_1: &str = "snxprj/1.1";

/// `.snxprj` version policy: a newer minor opens read-only, a newer major
/// is refused, a missing token reads as `snxprj/1`. A save writes the
/// lowest token its content needs ([`snxprj_token`]).
pub const SNXPRJ_VERSION: FormatVersion = FormatVersion {
    current: SNXPRJ_FORMAT_V1_1,
    readable: &[SNXPRJ_FORMAT_V1, SNXPRJ_FORMAT_V1_1],
    newer_minor: NewerMinor::ReadOnly,
    missing: MissingToken::ReadAs(SNXPRJ_FORMAT_V1),
};

/// The token a save of `data` writes: [`SNXPRJ_FORMAT_V1_1`] when it has
/// variant settings, [`SNXPRJ_FORMAT_V1`] otherwise.
pub fn snxprj_token(data: &ProjectData) -> &'static str {
    if data.variant_settings.is_empty() {
        SNXPRJ_FORMAT_V1
    } else {
        SNXPRJ_FORMAT_V1_1
    }
}

/// True when a project read as `token` opens read-only: a newer minor.
pub fn snxprj_opens_read_only(token: &str) -> bool {
    SNXPRJ_VERSION.is_newer_minor(token)
}

impl ProjectData {
    /// Read from a newer minor: it opens for viewing and every save of it
    /// is refused.
    pub fn is_from_newer_minor(&self) -> bool {
        self.unknown.newer_minor_source.is_some()
    }

    /// [`FormatVersion::refuse_save`] for the token the project was read as.
    pub fn refuse_save(&self) -> Result<(), FormatVersionError> {
        match &self.unknown.newer_minor_source {
            Some(source) => SNXPRJ_VERSION.refuse_save(source),
            None => Ok(()),
        }
    }
}
