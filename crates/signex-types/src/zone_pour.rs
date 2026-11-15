//! A polygon pour (#2216): a copper zone whose `fill_type` is [`POUR_FILL`].
//! Its outline only bounds the copper; the copper itself is the fill
//! `pcb_geom::zone_fills` computes, clear of other nets' copper by the
//! pour's clearance. The fill is not stored: see `docs/PCB_TOOLS.md`,
//! "Polygon Pour".

use crate::pcb::{PcbBoard, Zone};

/// The `fill_type` of a polygon pour.
pub const POUR_FILL: &str = "pour";

impl Zone {
    /// Whether this zone is a polygon pour, whose copper is computed.
    pub fn is_pour(&self) -> bool {
        self.fill_type == POUR_FILL
    }

    /// How far, in mm, the pour's fill keeps from copper of other nets: its
    /// own clearance when set, else the board's (`PcbSetup::clearance`),
    /// else [`crate::pcb::PCB_DEFAULT_CLEARANCE_MM`].
    pub fn pour_clearance_mm(&self, board: &PcbBoard) -> f64 {
        let set = |v: f64| (v.is_finite() && v > 0.0).then_some(v);
        set(self.clearance)
            .or_else(|| board.setup.as_ref().and_then(|s| set(s.clearance)))
            .unwrap_or(crate::pcb::PCB_DEFAULT_CLEARANCE_MM)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pour_is_named_by_its_fill_type() {
        let mut zone: Zone = serde_json::from_str(
            r#"{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "fill_type": "solid" }"#,
        )
        .expect("zone");
        assert!(!zone.is_pour());
        zone.fill_type = POUR_FILL.into();
        assert!(zone.is_pour());
    }
}
