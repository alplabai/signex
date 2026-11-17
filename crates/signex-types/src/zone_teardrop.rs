//! A teardrop (#2408): a copper zone whose `fill_type` is
//! [`TEARDROP_FILL`], drawn by Tools > Teardrops where a track enters a pad
//! or via of its net. It is solid copper on the track's net and layer, as a
//! Solid Region is; the fill type only lets Remove Teardrops find it. See
//! `docs/PCB_TOOLS.md`, "Teardrops".

use crate::pcb::Zone;

/// The `fill_type` of a teardrop.
pub const TEARDROP_FILL: &str = "teardrop";

impl Zone {
    /// Whether this zone is a teardrop Tools > Teardrops added.
    pub fn is_teardrop(&self) -> bool {
        self.fill_type == TEARDROP_FILL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_teardrop_is_named_by_its_fill_type_and_is_neither_pour_nor_rule() {
        let mut zone: Zone = serde_json::from_str(
            r#"{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "fill_type": "solid" }"#,
        )
        .expect("zone");
        assert!(!zone.is_teardrop());
        zone.fill_type = TEARDROP_FILL.into();
        assert!(zone.is_teardrop());
        assert!(!zone.is_pour() && !zone.is_rule_area());
    }
}
