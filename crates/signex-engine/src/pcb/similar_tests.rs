use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// R1 and R2 (10k), C1 (100n); a 0.25 mm and a 0.5 mm track on GND and a
/// 0.25 mm one on VCC; one via.
fn board() -> PcbBoard {
    let fp = |n: u128, reference: &str, value: &str| {
        serde_json::json!({
            "uuid": id(n), "reference": reference, "value": value,
            "footprint_id": "R_0603", "position": {"x": n as f64, "y": 0.0},
            "layer": "TopCopper"
        })
    };
    let track = |n: u128, width: f64, net: u32| {
        serde_json::json!({
            "uuid": id(n), "start": {"x": 0.0, "y": n as f64},
            "end": {"x": 5.0, "y": n as f64}, "width": width,
            "layer": "TopCopper", "net": net
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(1000),
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "VCC"}],
        "footprints": [fp(1, "R1", "10k"), fp(2, "R2", "10k"), fp(3, "C1", "100n")],
        "segments": [track(11, 0.25, 1), track(12, 0.5, 1), track(13, 0.25, 2)],
        "vias": [{"uuid": id(21), "position": {"x": 1.0, "y": 1.0},
                  "diameter": 0.6, "drill": 0.3, "net": 1}]
    }))
    .unwrap()
}

fn same(board: &PcbBoard, seed: ItemRef, keep: &[SimilarProperty]) -> Vec<ItemRef> {
    let criteria: Vec<SimilarValue> = properties_of(board, seed)
        .into_iter()
        .filter(|v| keep.contains(&v.property))
        .collect();
    find_similar(board, &criteria)
}

#[test]
fn a_footprint_matches_footprints_of_the_same_value() {
    let b = board();
    use SimilarProperty as P;
    let seed = ItemRef::Footprint(id(1));
    assert_eq!(
        same(&b, seed, &[P::Kind, P::Value]),
        [ItemRef::Footprint(id(1)), ItemRef::Footprint(id(2))]
    );
    assert_eq!(same(&b, seed, &[P::Kind]).len(), 3);
    assert_eq!(
        same(&b, seed, &[P::Kind, P::DesignatorPrefix]).len(),
        2,
        "R1 and R2, not C1"
    );
    assert_eq!(describe(&b, seed), "Footprint R1");
}

#[test]
fn a_track_matches_tracks_by_width_and_net() {
    let b = board();
    use SimilarProperty as P;
    let seed = ItemRef::Track(id(11));
    assert_eq!(
        same(&b, seed, &[P::Kind, P::Width]),
        [ItemRef::Track(id(11)), ItemRef::Track(id(13))]
    );
    assert_eq!(
        same(&b, seed, &[P::Kind, P::NetName]),
        [ItemRef::Track(id(11)), ItemRef::Track(id(12))]
    );
    let width = properties_of(&b, seed)
        .into_iter()
        .find(|v| v.property == P::Width)
        .unwrap();
    assert_eq!(width.value, "0.25 mm");
}

#[test]
fn a_via_matches_vias_and_an_unknown_item_has_no_properties() {
    let b = board();
    let seed = ItemRef::Via(id(21));
    assert_eq!(
        same(&b, seed, &[SimilarProperty::Kind]),
        [ItemRef::Via(id(21))]
    );
    assert!(properties_of(&b, ItemRef::Via(id(99))).is_empty());
    assert_eq!(
        SimilarProperty::from_key("net_name"),
        Some(SimilarProperty::NetName)
    );
    assert_eq!(SimilarProperty::from_key("nope"), None);
}
