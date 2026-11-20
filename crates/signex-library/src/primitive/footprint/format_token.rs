//! The `.snxfpt` format version (#2242, decision w9b 5/9/10): the token a
//! save writes follows the content, as the schematic's does. A file whose
//! sketches hold no link writes `snxfpt/1.1` byte for byte.

use signex_types::format_version::{FormatVersion, MissingToken, NewerMinor};

use super::FootprintFile;

/// 1.1 (#2242): a pad may carry the `trapezoid:<dx>,<dy>` shape token.
pub(super) const SNXFPT_FORMAT_V1_1: &str = "snxfpt/1.1";

/// Written instead of [`SNXFPT_FORMAT_V1_1`] when a footprint's sketch holds
/// reference dimensions, pattern sources or mirrors, or per-instance pad
/// overrides: a 1.1 build would drop them on save, so it refuses the file
/// by name instead.
pub(super) const SNXFPT_FORMAT_SKETCH_LINKS: &str = "snxfpt/1.2";

/// `.snxfpt` version policy. A newer token, minor included, is refused by
/// name: an older build cannot tell which of its shapes it would misread.
pub(super) const FOOTPRINT_FILE_VERSION: FormatVersion = FormatVersion {
    current: SNXFPT_FORMAT_SKETCH_LINKS,
    readable: &["snxfpt/1", SNXFPT_FORMAT_V1_1, SNXFPT_FORMAT_SKETCH_LINKS],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

impl FootprintFile {
    /// The token a save of this file writes: [`SNXFPT_FORMAT_SKETCH_LINKS`]
    /// when any footprint's sketch needs it, else [`SNXFPT_FORMAT_V1_1`].
    /// From content, so a file whose last link is removed goes back to 1.1.
    pub fn format_token(&self) -> &'static str {
        let links = self
            .footprints
            .iter()
            .filter_map(|fp| fp.sketch.as_ref())
            .any(|sketch| sketch.needs_format_1_2());
        if links {
            SNXFPT_FORMAT_SKETCH_LINKS
        } else {
            SNXFPT_FORMAT_V1_1
        }
    }
}
