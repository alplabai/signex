//! The `.snxfpt` format version (#2242, Caner 2026-11-20, decisions 5, 9 and 10): the token a
//! save writes follows the content, as the schematic's does. A file whose
//! sketches hold no link and whose footprints generate nothing writes
//! `snxfpt/1.1` byte for byte.

use signex_types::format_version::{FormatVersion, MissingToken, NewerMinor};

use super::{Footprint, FootprintFile};

/// 1.1 (#2242): a pad may carry the `trapezoid:<dx>,<dy>` shape token.
pub(super) const SNXFPT_FORMAT_V1_1: &str = "snxfpt/1.1";

/// Written instead of [`SNXFPT_FORMAT_V1_1`] when a footprint's sketch holds
/// reference dimensions, pattern sources or mirrors, or per-instance pad
/// overrides: a 1.1 build would drop them on save, so it refuses the file
/// by name instead.
pub(super) const SNXFPT_FORMAT_SKETCH_LINKS: &str = "snxfpt/1.2";

/// Written when a footprint has generated features (mockup row 124, #2834)
/// or a pattern with a non-default numbering sweep (mockup row 126): a 1.2
/// build would drop them on save, so it refuses the file by name.
pub(super) const SNXFPT_FORMAT_V1_3: &str = "snxfpt/1.3";

/// Written when a pad or a sketch pad sets its own thermal relief values
/// (#2593): a 1.3 build refuses the pad columns as a schema mismatch and
/// would drop the sketch pad's on save, so it refuses the file by name.
pub(super) const SNXFPT_FORMAT_V1_4: &str = "snxfpt/1.4";

/// `.snxfpt` version policy. A newer token, minor included, is refused by
/// name: an older build cannot tell which of its shapes it would misread.
pub(super) const FOOTPRINT_FILE_VERSION: FormatVersion = FormatVersion {
    current: SNXFPT_FORMAT_V1_4,
    readable: &[
        "snxfpt/1",
        SNXFPT_FORMAT_V1_1,
        SNXFPT_FORMAT_SKETCH_LINKS,
        SNXFPT_FORMAT_V1_3,
        SNXFPT_FORMAT_V1_4,
    ],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

impl FootprintFile {
    /// The lowest token that holds this file's content. From content, so a
    /// file whose last link, sweep or generated feature goes drops back down.
    pub fn format_token(&self) -> &'static str {
        if self.footprints.iter().any(has_pad_thermal) {
            return SNXFPT_FORMAT_V1_4;
        }
        let needs_1_3 = |fp: &Footprint| {
            !fp.generated.is_default()
                || fp.sketch.as_ref().is_some_and(|s| s.has_numbering_sweep())
        };
        if self.footprints.iter().any(needs_1_3) {
            return SNXFPT_FORMAT_V1_3;
        }
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

/// Whether a pad or a sketch pad of `fp` sets its own thermal relief values.
fn has_pad_thermal(fp: &Footprint) -> bool {
    let mut sketch_pads = fp
        .sketch
        .iter()
        .flat_map(|sketch| sketch.entities.iter())
        .filter_map(|entity| entity.pad.as_ref());
    fp.pads.iter().any(|pad| !pad.thermal.is_rule())
        || sketch_pads.any(|attr| !attr.stack.thermal.is_rule())
}
