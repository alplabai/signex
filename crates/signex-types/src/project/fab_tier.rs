//! What a generic fab tier can build (#2311): the minimums of every base
//! DRC rule a maker's capability page lists, and where the numbers come
//! from. The tiers are generic starting points; no fabricator is named.
//! See `docs/DRC_RULES.md`, "Fab profiles".

use super::rule_profile::FabProfileInfo;
use super::setup_profile::FabTemplate;

/// The limits of one tier, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FabCapabilities {
    pub clearance: f64,
    pub track_width: f64,
    pub via_diameter: f64,
    pub via_drill: f64,
    /// The smallest hole drilled, pad or via.
    pub hole: f64,
    pub annular_ring: f64,
    /// Hole edge to hole edge.
    pub hole_to_hole: f64,
    /// Copper to the board outline.
    pub edge_clearance: f64,
    /// The narrowest solder mask strip between two openings.
    pub mask_sliver: f64,
    /// Silkscreen to an exposed pad.
    pub silk_to_pad: f64,
    /// The router bit that cuts the board outline and its openings: an
    /// inside corner tighter than half of it cannot be cut (mockup row 179d).
    pub router_tool_diameter: f64,
    /// The narrowest slot the maker routes, across (mockup row 179d).
    pub min_slot: f64,
}

/// When the tier numbers were last checked.
const TIERS_REVISED: &str = "2026-11-19";

impl FabTemplate {
    /// The tier's limits: the one tier table. A tier card's clearance and
    /// via come from here (`SetupProfile::rule_values`).
    pub const fn capabilities(self) -> FabCapabilities {
        match self {
            Self::Standard => FabCapabilities {
                clearance: 0.15,
                track_width: 0.15,
                via_diameter: 0.5,
                via_drill: 0.25,
                hole: 0.25,
                annular_ring: 0.125,
                hole_to_hole: 0.5,
                edge_clearance: 0.3,
                mask_sliver: 0.1,
                silk_to_pad: 0.15,
                router_tool_diameter: 2.0,
                min_slot: 0.8,
            },
            Self::Advanced => FabCapabilities {
                clearance: 0.1,
                track_width: 0.1,
                via_diameter: 0.4,
                via_drill: 0.2,
                hole: 0.2,
                annular_ring: 0.1,
                hole_to_hole: 0.4,
                edge_clearance: 0.25,
                mask_sliver: 0.075,
                silk_to_pad: 0.1,
                router_tool_diameter: 1.6,
                min_slot: 0.6,
            },
            Self::Hdi => FabCapabilities {
                clearance: 0.075,
                track_width: 0.075,
                via_diameter: 0.25,
                via_drill: 0.1,
                hole: 0.1,
                annular_ring: 0.075,
                hole_to_hole: 0.3,
                edge_clearance: 0.2,
                mask_sliver: 0.06,
                silk_to_pad: 0.075,
                router_tool_diameter: 1.0,
                min_slot: 0.5,
            },
        }
    }

    /// Where the tier's numbers come from, as a fab profile names its own.
    pub fn info(self) -> FabProfileInfo {
        let process = match self {
            Self::Standard => "standard process: through vias, common spacing",
            Self::Advanced => "fine-line process: smaller vias, tighter spacing",
            Self::Hdi => "high-density process: micro vias, the finest spacing",
        };
        FabProfileInfo {
            manufacturer: String::new(),
            process: process.to_string(),
            source: "generic tier shipped with Signex; check your maker's published limits"
                .to_string(),
            published: TIERS_REVISED.to_string(),
            router_tool_diameter: Some(self.capabilities().router_tool_diameter),
            min_slot: Some(self.capabilities().min_slot),
        }
    }
}
