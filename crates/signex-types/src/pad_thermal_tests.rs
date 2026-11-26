use uuid::Uuid;

use super::*;
use crate::zone_pour::POUR_FILL;

fn pour(on: bool, gap: f64, width: f64) -> Zone {
    Zone {
        uuid: Uuid::from_u128(1),
        net: 1,
        net_name: "GND".into(),
        layer: "TopCopper".into(),
        outline: Vec::new(),
        holes: Vec::new(),
        priority: 0,
        fill_type: POUR_FILL.into(),
        thermal_relief: on,
        thermal_gap: gap,
        thermal_width: width,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        unknown: Default::default(),
    }
}

#[test]
fn a_pad_on_the_rule_takes_the_pours_values_and_four_spokes() {
    let relief = pour(true, 0.3, 0.2).pour_relief(&PadThermal::RULE);
    assert_eq!(
        relief,
        Some(Relief {
            air_gap: 0.3,
            spoke_width: 0.2,
            spokes: DEFAULT_SPOKES,
        })
    );
}

#[test]
fn a_pads_own_values_win_one_by_one() {
    let pad = PadThermal {
        air_gap: Some(0.5),
        spoke_width: None,
        spokes: Some(2),
    };
    let relief = pour(true, 0.3, 0.2).pour_relief(&pad).unwrap();
    assert_eq!(relief.air_gap, 0.5);
    assert_eq!(relief.spoke_width, 0.2, "the width follows the pour");
    assert_eq!(relief.spokes, 2);
}

#[test]
fn thermal_relief_off_or_no_pour_joins_solidly() {
    let pad = PadThermal {
        air_gap: Some(0.5),
        ..PadThermal::RULE
    };
    assert_eq!(pour(false, 0.3, 0.2).pour_relief(&pad), None);
    let mut solid = pour(true, 0.3, 0.2);
    solid.fill_type = "solid".into();
    assert_eq!(solid.pour_relief(&pad), None);
}

/// A pour switched on with no stored gap or width (as boards written before
/// the spokes were drawn hold) uses the defaults.
#[test]
fn a_pour_with_no_stored_gap_or_width_uses_the_defaults() {
    let relief = pour(true, 0.0, f64::NAN)
        .pour_relief(&PadThermal::RULE)
        .unwrap();
    assert_eq!(relief.air_gap, PCB_THERMAL_GAP_MM);
    assert_eq!(relief.spoke_width, PCB_THERMAL_BRIDGE_MM);
}

#[test]
fn a_value_no_pour_can_use_is_a_fault_naming_it() {
    assert_eq!(PadThermal::RULE.fault(), None);
    assert!(PadThermal::RULE.is_rule());
    let gap = PadThermal {
        air_gap: Some(0.0),
        ..PadThermal::RULE
    };
    assert_eq!(gap.fault(), Some(PadThermalFault::AirGap));
    let width = PadThermal {
        spoke_width: Some(f64::INFINITY),
        ..PadThermal::RULE
    };
    assert_eq!(width.fault(), Some(PadThermalFault::SpokeWidth));
    for n in [0, MAX_SPOKES + 1] {
        let spokes = PadThermal {
            spokes: Some(n),
            ..PadThermal::RULE
        };
        assert_eq!(spokes.fault(), Some(PadThermalFault::Spokes), "{n}");
        assert_eq!(spokes.fault().unwrap().field(), "spokes");
    }
    let manual = PadThermal {
        air_gap: Some(0.25),
        spoke_width: Some(0.3),
        spokes: Some(1),
    };
    assert_eq!(manual.fault(), None);
    assert!(!manual.is_rule());
}
