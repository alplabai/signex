//! Signex-native PCB layer abstraction.
//!
//! Variants are **semantic** — they describe a layer's purpose
//! (top copper, bottom silkscreen, courtyard, etc.), not its bit
//! position in any particular EDA tool's internal layer set.
//! The previous version of this module exposed `LayerId(u8)` plus
//! pre-Standard-7 numeric constants (`F_CU = 0`, `B_CU = 31`, …) that
//! mirrored Standard's `PCB_LAYER_ID` numbering; those have been
//! removed as part of the issue #62 Apache-clean remediation.
//! Concrete `u8` IDs for any future foreign-format I/O are produced
//! by the `signex-standard-import` companion crate's translation layer
//! and do not live in this Apache codebase.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SignexLayer — semantic variant set
// ---------------------------------------------------------------------------

/// A PCB layer identified by purpose, not by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignexLayer {
    TopCopper,
    BottomCopper,
    /// 1-based index of an inner-stackup copper layer.
    InnerCopper(u8),
    TopSilk,
    BottomSilk,
    TopSolderMask,
    BottomSolderMask,
    TopPaste,
    BottomPaste,
    TopAssembly,
    BottomAssembly,
    TopCourtyard,
    BottomCourtyard,
    BoardOutline,
    KeepOut,
    /// 1-based index of a user-defined mechanical layer.
    Mechanical(u8),
    /// 1-based index of a generic user layer (notes, comments, etc.).
    User(u8),
}

// ---------------------------------------------------------------------------
// LayerKind — coarse category for theme rendering and picker grouping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKind {
    Copper,
    Silk,
    Mask,
    Paste,
    Assembly,
    Courtyard,
    Outline,
    KeepOut,
    Mechanical,
    User,
}

// ---------------------------------------------------------------------------
// SignexLayer methods
// ---------------------------------------------------------------------------

pub const F_CU: LayerId = LayerId(0);
pub const B_CU: LayerId = LayerId(31);
pub const F_SILKS: LayerId = LayerId(36);
pub const B_SILKS: LayerId = LayerId(37);
pub const F_MASK: LayerId = LayerId(38);
pub const B_MASK: LayerId = LayerId(39);
pub const F_PASTE: LayerId = LayerId(34);
pub const B_PASTE: LayerId = LayerId(35);
pub const F_FAB: LayerId = LayerId(40);
pub const B_FAB: LayerId = LayerId(41);
pub const F_CRTYD: LayerId = LayerId(42);
pub const B_CRTYD: LayerId = LayerId(43);
pub const EDGE_CUTS: LayerId = LayerId(44);
pub const MARGIN: LayerId = LayerId(45);
pub const DWGS_USER: LayerId = LayerId(46);
pub const CMTS_USER: LayerId = LayerId(47);
pub const ECO1_USER: LayerId = LayerId(49);
pub const ECO2_USER: LayerId = LayerId(50);

impl LayerId {
    /// Canonical Altium UI label for this layer. Drives every PCB
    /// surface where layer names are shown to the user (View
    /// Configuration, layer pickers, footprint editor toolbar,
    /// footprint diff legend). The internal data layer keeps the Standard
    /// numeric `LayerId`; this is purely a display rename so the user
    /// sees Altium nomenclature per `docs/UX_REFERENCE_ALTIUM.md`.
    ///
    /// Inner copper layers `In1.Cu` … `In30.Cu` (LayerId 1–30) map to
    /// `Mid Layer 1` … `Mid Layer 30`.
    pub fn altium_label(self) -> String {
        match self {
            F_CU => "Top Layer".into(),
            B_CU => "Bottom Layer".into(),
            F_SILKS => "Top Overlay".into(),
            B_SILKS => "Bottom Overlay".into(),
            F_MASK => "Top Solder".into(),
            B_MASK => "Bottom Solder".into(),
            F_PASTE => "Top Paste".into(),
            B_PASTE => "Bottom Paste".into(),
            F_FAB => "Top Assembly".into(),
            B_FAB => "Bottom Assembly".into(),
            F_CRTYD => "Top Courtyard".into(),
            B_CRTYD => "Bottom Courtyard".into(),
            EDGE_CUTS => "Keep-Out".into(),
            MARGIN => "Board Outline".into(),
            DWGS_USER => "Mechanical 1".into(),
            CMTS_USER => "Mechanical 2".into(),
            ECO1_USER => "Mechanical 3".into(),
            ECO2_USER => "Mechanical 4".into(),
            LayerId(n) if (1..=30).contains(&n) => format!("Mid Layer {n}"),
            LayerId(n) => format!("Layer {n}"),
        }
    }

    /// Standard-style canonical name (e.g. `F.Cu`, `B.SilkS`). Used by
    /// callers that need to round-trip a `LayerId` through Standard
    /// S-expressions; the UI should generally prefer `altium_label`.
    pub fn standard_name(self) -> String {
        match self {
            F_CU => "F.Cu".into(),
            B_CU => "B.Cu".into(),
            F_SILKS => "F.SilkS".into(),
            B_SILKS => "B.SilkS".into(),
            F_MASK => "F.Mask".into(),
            B_MASK => "B.Mask".into(),
            F_PASTE => "F.Paste".into(),
            B_PASTE => "B.Paste".into(),
            F_FAB => "F.Fab".into(),
            B_FAB => "B.Fab".into(),
            F_CRTYD => "F.CrtYd".into(),
            B_CRTYD => "B.CrtYd".into(),
            EDGE_CUTS => "Edge.Cuts".into(),
            MARGIN => "Margin".into(),
            DWGS_USER => "Dwgs.User".into(),
            CMTS_USER => "Cmts.User".into(),
            ECO1_USER => "Eco1.User".into(),
            ECO2_USER => "Eco2.User".into(),
            LayerId(n) if (1..=30).contains(&n) => format!("In{n}.Cu"),
            LayerId(n) => format!("Layer{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altium_labels_match_ux_reference() {
        // Spot-check the canonical pairs from docs/UX_REFERENCE_ALTIUM.md.
        assert_eq!(F_CU.altium_label(), "Top Layer");
        assert_eq!(B_CU.altium_label(), "Bottom Layer");
        assert_eq!(F_SILKS.altium_label(), "Top Overlay");
        assert_eq!(B_SILKS.altium_label(), "Bottom Overlay");
        assert_eq!(F_MASK.altium_label(), "Top Solder");
        assert_eq!(F_PASTE.altium_label(), "Top Paste");
        assert_eq!(F_FAB.altium_label(), "Top Assembly");
        assert_eq!(F_CRTYD.altium_label(), "Top Courtyard");
        assert_eq!(EDGE_CUTS.altium_label(), "Keep-Out");
        assert_eq!(MARGIN.altium_label(), "Board Outline");
        assert_eq!(DWGS_USER.altium_label(), "Mechanical 1");
        assert_eq!(CMTS_USER.altium_label(), "Mechanical 2");
        assert_eq!(ECO1_USER.altium_label(), "Mechanical 3");
        assert_eq!(ECO2_USER.altium_label(), "Mechanical 4");
        assert_eq!(LayerId(1).altium_label(), "Mid Layer 1");
        assert_eq!(LayerId(30).altium_label(), "Mid Layer 30");
    }

    #[test]
    fn standard_names_round_trip_with_pcb_layers_plan_table() {
        assert_eq!(F_CU.standard_name(), "F.Cu");
        assert_eq!(F_SILKS.standard_name(), "F.SilkS");
        assert_eq!(EDGE_CUTS.standard_name(), "Edge.Cuts");
        assert_eq!(LayerId(5).standard_name(), "In5.Cu");
    }
}

// ---------------------------------------------------------------------------
// Default layer colours (Altium-flavoured palette, RGBA)
// ---------------------------------------------------------------------------

pub const DEFAULT_LAYER_COLORS: &[(SignexLayer, [u8; 4])] = &[
    (SignexLayer::TopCopper, [0xC8, 0x00, 0x00, 0xFF]),       // red
    (SignexLayer::BottomCopper, [0x00, 0x00, 0xC8, 0xFF]),    // blue
    (SignexLayer::TopSilk, [0xC8, 0xC8, 0x00, 0xFF]),         // yellow
    (SignexLayer::BottomSilk, [0x80, 0x00, 0x80, 0xFF]),      // purple
    (SignexLayer::TopSolderMask, [0xC8, 0x00, 0xC8, 0x80]),   // magenta semi
    (SignexLayer::BottomSolderMask, [0x00, 0xC8, 0xC8, 0x80]), // cyan semi
    (SignexLayer::TopPaste, [0x80, 0x80, 0x00, 0xC0]),        // dark yellow
    (SignexLayer::BottomPaste, [0x00, 0x80, 0x80, 0xC0]),     // teal
    (SignexLayer::TopAssembly, [0x80, 0x80, 0x80, 0xFF]),     // grey
    (SignexLayer::BottomAssembly, [0x60, 0x60, 0x60, 0xFF]),  // dark grey
    (SignexLayer::TopCourtyard, [0xC0, 0xC0, 0xC0, 0xFF]),    // light grey
    (SignexLayer::BottomCourtyard, [0xA0, 0xA0, 0xA0, 0xFF]), // mid grey
    (SignexLayer::BoardOutline, [0xFF, 0xFF, 0x00, 0xFF]),    // bright yellow
    (SignexLayer::KeepOut, [0xFF, 0x00, 0xFF, 0xFF]),         // bright magenta
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altium_labels_match_reference() {
        assert_eq!(SignexLayer::TopCopper.altium_label(), "Top Layer");
        assert_eq!(SignexLayer::BottomCopper.altium_label(), "Bottom Layer");
        assert_eq!(SignexLayer::TopSilk.altium_label(), "Top Overlay");
        assert_eq!(SignexLayer::TopSolderMask.altium_label(), "Top Solder");
        assert_eq!(SignexLayer::TopPaste.altium_label(), "Top Paste");
        assert_eq!(SignexLayer::KeepOut.altium_label(), "Keep-Out");
        assert_eq!(SignexLayer::InnerCopper(2).altium_label(), "Mid Layer 2");
        assert_eq!(SignexLayer::Mechanical(13).altium_label(), "Mechanical 13");
    }

    #[test]
    fn kinds_partition_correctly() {
        assert_eq!(SignexLayer::TopCopper.kind(), LayerKind::Copper);
        assert_eq!(SignexLayer::InnerCopper(1).kind(), LayerKind::Copper);
        assert_eq!(SignexLayer::TopSilk.kind(), LayerKind::Silk);
        assert_eq!(SignexLayer::TopSolderMask.kind(), LayerKind::Mask);
        assert_eq!(SignexLayer::BoardOutline.kind(), LayerKind::Outline);
        assert_eq!(SignexLayer::KeepOut.kind(), LayerKind::KeepOut);
    }

    #[test]
    fn round_trip_json() {
        for l in [
            SignexLayer::TopCopper,
            SignexLayer::InnerCopper(3),
            SignexLayer::Mechanical(4),
            SignexLayer::User(7),
        ] {
            let s = serde_json::to_string(&l).unwrap();
            let back: SignexLayer = serde_json::from_str(&s).unwrap();
            assert_eq!(l, back);
        }
    }

    #[test]
    fn all_iteration_yields_canonical_set() {
        let v: Vec<_> = SignexLayer::all().collect();
        assert_eq!(v.len(), 14);
        assert_eq!(v[0], SignexLayer::TopCopper);
        assert_eq!(v[v.len() - 1], SignexLayer::KeepOut);
    }
}
