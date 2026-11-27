//! #2797: the length-matching group's band test and topology words.

use super::*;

fn group() -> LengthGroup {
    LengthGroup {
        uuid: Uuid::from_u128(1),
        name: "DDR_DQ".into(),
        target_mm: 28.5,
        tolerance_mm: 0.25,
        topology: LengthTopology::Chain,
        members: vec!["DDR_DQ0".into(), "DDR_DQ6".into()],
    }
}

#[test]
fn a_length_on_the_band_edge_is_in_the_band() {
    let g = group();
    assert!(!g.is_out_of_band(28.75));
    assert!(!g.is_out_of_band(28.25));
    assert!(g.is_out_of_band(28.751));
    assert!(g.is_out_of_band(28.91));
    assert!(g.is_out_of_band(f64::NAN));
}

#[test]
fn a_negative_tolerance_counts_as_zero() {
    assert!(!is_out_of_band(10.0, 10.0, -1.0));
    assert!(is_out_of_band(10.001, 10.0, -1.0));
}

#[test]
fn topology_words_round_trip() {
    for topology in LengthTopology::ALL {
        assert_eq!(LengthTopology::from_token(topology.token()), Some(topology));
    }
    assert_eq!(LengthTopology::from_token("ring"), None);
    assert_eq!(LengthTopology::FlyBy.token(), "fly_by");
}

#[test]
fn the_rule_name_follows_the_group() {
    let g = group();
    assert_eq!(g.rule_name(), "MatchedLengths_DDR_DQ");
    assert!(g.has_member("DDR_DQ6"));
    assert!(!g.has_member("DDR_DQ7"));
}
