//! Which footprint fields the sketch owns.
//!
//! The sketch bakes into footprint fields that can also be authored
//! elsewhere (an imported or hand-edited `.snxfpt`, silk drawn in Pads
//! mode). The sketch owns a kind when it holds an entity tagged for it, or
//! when an earlier bake produced that kind and recorded it in
//! [`SketchData::baked_kinds`]. Only an owned kind is cleared by an empty
//! bake. See `docs/FOOTPRINT_SKETCH.md`, "Bake ownership".

use signex_types::layer::SignexLayer;

use crate::sketch::SketchData;

/// One bakeable footprint field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BakedKind {
    SilkF,
    SilkB,
    Courtyard,
    MaskOpenings,
    MaskExcludes,
    PasteApertures,
    Pours,
    Keepouts,
    Cutouts,
    VScores,
}

impl BakedKind {
    /// The stored token. Never reuse or rename one: a file written by a
    /// newer build must keep meaning the same thing to an older one.
    pub const fn token(self) -> &'static str {
        match self {
            Self::SilkF => "silk_f",
            Self::SilkB => "silk_b",
            Self::Courtyard => "courtyard",
            Self::MaskOpenings => "mask_openings",
            Self::MaskExcludes => "mask_excludes",
            Self::PasteApertures => "paste_apertures",
            Self::Pours => "pours",
            Self::Keepouts => "keepouts",
            Self::Cutouts => "cutouts",
            Self::VScores => "v_scores",
        }
    }

    /// Name for messages.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SilkF => "top silk",
            Self::SilkB => "bottom silk",
            Self::Courtyard => "courtyard",
            Self::MaskOpenings => "mask openings",
            Self::MaskExcludes => "mask excludes",
            Self::PasteApertures => "paste apertures",
            Self::Pours => "pours",
            Self::Keepouts => "keepouts",
            Self::Cutouts => "cutouts",
            Self::VScores => "V-scores",
        }
    }
}

impl SketchData {
    /// Whether an entity is tagged for `kind`.
    pub fn tags_kind(&self, kind: BakedKind) -> bool {
        self.entities.iter().any(|e| match kind {
            BakedKind::SilkF => e
                .silk
                .as_ref()
                .is_some_and(|a| a.layer == SignexLayer::TopSilk),
            BakedKind::SilkB => e
                .silk
                .as_ref()
                .is_some_and(|a| a.layer == SignexLayer::BottomSilk),
            BakedKind::Courtyard => e.courtyard.is_some(),
            BakedKind::MaskOpenings => e.mask_opening.is_some(),
            BakedKind::MaskExcludes => e.mask_exclude.is_some(),
            BakedKind::PasteApertures => e.paste_aperture.is_some(),
            BakedKind::Pours => e.pour.is_some(),
            BakedKind::Keepouts => e.keepout.is_some(),
            BakedKind::Cutouts => e.board_cutout.is_some(),
            BakedKind::VScores => e.v_score.is_some(),
        })
    }

    /// Whether a bake produced `kind` earlier (stored in the file).
    pub fn was_baked(&self, kind: BakedKind) -> bool {
        self.baked_kinds.iter().any(|t| t == kind.token())
    }

    /// Whether the sketch owns `kind`: tagged, or baked before.
    pub fn owns_kind(&self, kind: BakedKind) -> bool {
        self.tags_kind(kind) || self.was_baked(kind)
    }

    /// Record that a bake did (`true`) or no longer does (`false`) produce
    /// `kind`. Tokens this build does not know are left alone.
    pub fn record_baked(&mut self, kind: BakedKind, baked: bool) {
        match (baked, self.was_baked(kind)) {
            (true, false) => self.baked_kinds.push(kind.token().to_string()),
            (false, true) => self.baked_kinds.retain(|t| t != kind.token()),
            _ => {}
        }
    }
}
