//! A polygon pour (#2216): a copper zone whose `fill_type` is [`POUR_FILL`].
//! Its outline only bounds the copper; the copper itself is the fill
//! `pcb_geom::zone_fills` computes, clear of other nets' copper by the
//! pour's clearance. The fill is not stored: see `docs/PCB_TOOLS.md`,
//! "Polygon Pour". A shelved pour (`Zone::shelved`) has no fill at all, and
//! an unfilled one (`Zone::unfilled`) none until the next Repour.

use crate::pcb::{PcbBoard, Zone};

/// The `fill_type` of a polygon pour.
pub const POUR_FILL: &str = "pour";

/// `Zone::min_island_area` is left out of the file at 0.
pub(crate) fn is_unset_area(value: &f64) -> bool {
    *value == 0.0
}

/// `Zone::remove_dead_islands`, `Zone::shelved` and `Zone::unfilled` are
/// left out of the file when off.
pub(crate) fn is_off(value: &bool) -> bool {
    !*value
}

impl Zone {
    /// Whether this zone is a polygon pour, whose copper is computed.
    pub fn is_pour(&self) -> bool {
        self.fill_type == POUR_FILL
    }

    /// Whether this zone is a shelved polygon pour (#2523): its outline and
    /// settings stay, but it has no fill. `shelved` on any other zone is
    /// ignored.
    pub fn is_shelved_pour(&self) -> bool {
        self.is_pour() && self.shelved
    }

    /// Whether this zone is an unfilled polygon pour (#2506): its outline
    /// and settings stay and it has no fill until the next Repour. A shelved
    /// pour is shelved, not unfilled; `unfilled` on any other zone is
    /// ignored.
    pub fn is_unfilled_pour(&self) -> bool {
        self.is_pour() && self.unfilled && !self.shelved
    }

    /// Whether this zone is a polygon pour that fills: neither shelved nor
    /// unfilled. Only such a pour holds copper and clears other copper.
    pub fn is_filling_pour(&self) -> bool {
        self.is_pour() && !self.shelved && !self.unfilled
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

    /// The smallest island, in mm², the pour keeps; `None` keeps every
    /// island (0, or a value that is not a positive number).
    pub fn pour_min_island_mm2(&self) -> Option<f64> {
        let area = self.min_island_area;
        (area.is_finite() && area > 0.0).then_some(area)
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

    /// The island settings default off, are not written while off, and
    /// round-trip once set.
    #[test]
    fn island_settings_are_written_only_when_set() {
        let mut zone: Zone = serde_json::from_str(
            r#"{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "fill_type": "pour" }"#,
        )
        .expect("zone");
        assert_eq!(zone.min_island_area, 0.0);
        assert!(!zone.remove_dead_islands);
        assert_eq!(zone.pour_min_island_mm2(), None);
        let text = serde_json::to_string(&zone).expect("json");
        assert!(!text.contains("min_island_area") && !text.contains("remove_dead_islands"));

        zone.min_island_area = 2.5;
        zone.remove_dead_islands = true;
        assert_eq!(zone.pour_min_island_mm2(), Some(2.5));
        let back: Zone =
            serde_json::from_str(&serde_json::to_string(&zone).expect("json")).expect("zone");
        assert_eq!(back, zone);
        zone.min_island_area = -1.0;
        assert_eq!(zone.pour_min_island_mm2(), None);
    }

    /// #2523: only a pour is shelved; the flag on another zone is ignored.
    #[test]
    fn only_a_pour_is_shelved() {
        let mut zone: Zone = serde_json::from_str(
            r#"{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "fill_type": "pour" }"#,
        )
        .expect("zone");
        assert!(!zone.shelved && !zone.is_shelved_pour());
        zone.shelved = true;
        assert!(zone.is_shelved_pour());
        zone.fill_type = "solid".into();
        assert!(!zone.is_shelved_pour());
    }

    /// #2506: only a pour is unfilled; a shelved pour is shelved, not
    /// unfilled; neither fills. The key is written only when on.
    #[test]
    fn only_a_pour_is_unfilled_and_neither_it_nor_a_shelved_pour_fills() {
        let mut zone: Zone = serde_json::from_str(
            r#"{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "fill_type": "pour" }"#,
        )
        .expect("zone");
        assert!(!zone.unfilled && !zone.is_unfilled_pour() && zone.is_filling_pour());
        assert!(
            !serde_json::to_string(&zone)
                .expect("json")
                .contains("unfilled")
        );
        zone.unfilled = true;
        assert!(zone.is_unfilled_pour() && !zone.is_filling_pour());
        assert!(
            serde_json::to_string(&zone)
                .expect("json")
                .contains("\"unfilled\":true")
        );
        zone.shelved = true;
        assert!(!zone.is_unfilled_pour() && !zone.is_filling_pour());
        zone.shelved = false;
        zone.fill_type = "solid".into();
        assert!(!zone.is_unfilled_pour() && !zone.is_filling_pour());
    }
}
