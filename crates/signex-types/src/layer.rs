use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Layer ID (0..63 in KiCad convention)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerId(pub u8);

// ---------------------------------------------------------------------------
// Layer kind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKind {
    Signal,
    Power,
    Mixed,
    User,
    Mechanical,
}

// ---------------------------------------------------------------------------
// Well-known layer IDs (KiCad numbering)
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

// ---------------------------------------------------------------------------
// Default layer colours (Altium-inspired RGBA)
// ---------------------------------------------------------------------------

/// Default layer colors as `(LayerId, [r, g, b, a])` tuples.
pub const DEFAULT_LAYER_COLORS: &[(LayerId, [u8; 4])] = &[
    (F_CU, [0xC8, 0x00, 0x00, 0xFF]),      // red
    (B_CU, [0x00, 0x00, 0xC8, 0xFF]),      // blue
    (F_SILKS, [0xC8, 0xC8, 0x00, 0xFF]),   // yellow
    (B_SILKS, [0x80, 0x00, 0x80, 0xFF]),   // purple
    (F_MASK, [0xC8, 0x00, 0xC8, 0x80]),    // magenta semi-transparent
    (B_MASK, [0x00, 0xC8, 0xC8, 0x80]),    // cyan semi-transparent
    (F_PASTE, [0x80, 0x80, 0x00, 0xC0]),   // dark yellow
    (B_PASTE, [0x00, 0x80, 0x80, 0xC0]),   // teal
    (F_FAB, [0x80, 0x80, 0x80, 0xFF]),     // grey
    (B_FAB, [0x60, 0x60, 0x60, 0xFF]),     // dark grey
    (F_CRTYD, [0xC0, 0xC0, 0xC0, 0xFF]),   // light grey
    (B_CRTYD, [0xA0, 0xA0, 0xA0, 0xFF]),   // mid grey
    (EDGE_CUTS, [0xFF, 0xFF, 0x00, 0xFF]), // bright yellow
    (MARGIN, [0xFF, 0x00, 0xFF, 0xFF]),    // bright magenta
    (DWGS_USER, [0x60, 0x60, 0xC8, 0xFF]), // slate blue
    (CMTS_USER, [0x60, 0xC8, 0x60, 0xFF]), // slate green
];
