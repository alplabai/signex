//! The role picker's wording (#2518): one catalog entry per role. The
//! list itself is the host's (`SketchHost::roles`).

use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::sketch::{SketchHost, SketchRole};

/// The catalog id naming `role` in the picker on `host`. A cutout reads
/// "Board Cutout" in a footprint, where the board is elsewhere, and
/// "Cutout" on the board itself.
pub fn role_label_id(role: SketchRole, host: SketchHost) -> MessageId {
    use SketchRole as R;
    match role {
        R::Unassigned => ids::EDITORS_SKETCH_ROLE_UNASSIGNED,
        R::Pad => ids::EDITORS_SKETCH_ROLE_PAD,
        R::SilkTop => ids::EDITORS_SKETCH_ROLE_SILK_TOP,
        R::SilkBottom => ids::EDITORS_SKETCH_ROLE_SILK_BOTTOM,
        R::Courtyard => ids::EDITORS_SKETCH_ROLE_COURTYARD,
        R::Keepout => ids::EDITORS_SKETCH_ROLE_KEEPOUT,
        R::Cutout if host == SketchHost::Board => ids::EDITORS_SKETCH_ROLE_CUTOUT,
        R::Cutout => ids::EDITORS_SKETCH_ROLE_BOARD_CUTOUT,
        R::MaskOpeningTop => ids::EDITORS_SKETCH_ROLE_MASK_OPENING_TOP,
        R::MaskOpeningBottom => ids::EDITORS_SKETCH_ROLE_MASK_OPENING_BOTTOM,
        R::MaskExcludeTop => ids::EDITORS_SKETCH_ROLE_MASK_EXCLUDE_TOP,
        R::MaskExcludeBottom => ids::EDITORS_SKETCH_ROLE_MASK_EXCLUDE_BOTTOM,
        R::PourTop => ids::EDITORS_SKETCH_ROLE_POUR_TOP,
        R::PourBottom => ids::EDITORS_SKETCH_ROLE_POUR_BOTTOM,
        R::PasteApertureTop => ids::EDITORS_SKETCH_ROLE_PASTE_TOP,
        R::PasteApertureBottom => ids::EDITORS_SKETCH_ROLE_PASTE_BOTTOM,
        R::BoardOutline => ids::EDITORS_SKETCH_ROLE_BOARD_OUTLINE,
        R::MountingHole => ids::EDITORS_SKETCH_ROLE_MOUNTING_HOLE,
        R::VScore => ids::EDITORS_SKETCH_ROLE_V_SCORE,
        R::Slot => ids::EDITORS_SKETCH_ROLE_SLOT,
        R::BoardPour => ids::EDITORS_SKETCH_ROLE_BOARD_POUR,
        R::MechanicalLayer => ids::EDITORS_SKETCH_ROLE_MECHANICAL_LAYER,
        R::ComponentAnchor => ids::EDITORS_SKETCH_ROLE_COMPONENT_ANCHOR,
    }
}

/// `role` as the picker on `host` shows it, in the UI language.
pub fn role_label(role: SketchRole, host: SketchHost, i18n: &I18n) -> String {
    t!(i18n, role_label_id(role, host)).into_string()
}

/// Every role the picker on `host` offers with its label, in order.
pub fn role_labels(host: SketchHost, i18n: &I18n) -> Vec<(SketchRole, String)> {
    host.roles()
        .iter()
        .map(|&role| (role, role_label(role, host, i18n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en_us() -> I18n {
        I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()))
    }

    /// The board picker reads in the board sketch design's words.
    #[test]
    fn the_board_picker_names_its_five_targets() {
        let labels: Vec<String> = role_labels(SketchHost::Board, &en_us())
            .into_iter()
            .map(|(_, label)| label.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert_eq!(
            labels,
            [
                "Unassigned",
                "Board outline",
                "Cutout",
                "Slot",
                "Keepout",
                "Pour",
                "V-score",
                "Mechanical layer",
                "Component anchor",
                "Mounting hole"
            ]
        );
    }

    #[test]
    fn the_footprint_picker_keeps_its_wording() {
        let i18n = en_us();
        let labels = role_labels(SketchHost::Footprint, &i18n);
        assert_eq!(labels.len(), 15);
        assert_eq!(labels[6].1, "Board Cutout");
        assert_eq!(labels[14].1, "Paste · Bottom");
    }
}
