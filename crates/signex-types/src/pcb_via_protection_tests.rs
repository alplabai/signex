use super::*;
use crate::pcb::{Point, ViaType};
use uuid::Uuid;

fn p(fill: ViaFill, plug_from: PlugSide, capped: bool, top: bool, bottom: bool) -> ViaProtection {
    ViaProtection {
        fill,
        plug_from,
        capped,
        tented_top: top,
        tented_bottom: bottom,
    }
}

#[test]
fn every_combination_names_its_ipc_4761_type() {
    use Ipc4761 as C;
    use PlugSide::{Both, Bottom, Top};
    use ViaFill::{Conductive, None as Open, Plugged, Resin};
    let cases = [
        // Open hole: the tents decide.
        (p(Open, Top, false, false, false), C::Open),
        (p(Open, Top, false, true, false), C::IA),
        (p(Open, Top, false, false, true), C::IA),
        (p(Open, Top, false, true, true), C::IB),
        // A cap needs a filled hole.
        (p(Open, Top, true, false, false), C::Custom),
        (p(Plugged, Both, true, true, true), C::Custom),
        // Plugged without a cover: the plug side decides.
        (p(Plugged, Top, false, false, false), C::IIIA),
        (p(Plugged, Bottom, false, false, false), C::IIIA),
        (p(Plugged, Both, false, false, false), C::IIIB),
        // Plugged and covered: the cover must be over the plug.
        (p(Plugged, Top, false, true, false), C::IVA),
        (p(Plugged, Bottom, false, false, true), C::IVA),
        (p(Plugged, Both, false, false, true), C::IVA),
        (p(Plugged, Both, false, true, true), C::IVB),
        (p(Plugged, Top, false, true, true), C::IVB),
        (p(Plugged, Top, false, false, true), C::Custom),
        (p(Plugged, Bottom, false, true, false), C::Custom),
        // Filled: capped wins over the tents.
        (p(Resin, Top, false, false, false), C::V),
        (p(Conductive, Top, false, true, false), C::VIA),
        (p(Resin, Top, false, true, true), C::VIB),
        (p(Resin, Top, true, false, false), C::VII),
        (p(Conductive, Top, true, true, true), C::VII),
    ];
    for (protection, class) in cases {
        assert_eq!(protection.class(), class, "{protection:?}");
    }
}

#[test]
fn a_preset_makes_its_own_type_and_two_types_have_none() {
    for class in Ipc4761::ALL {
        match class.preset() {
            Some(preset) => assert_eq!(preset.class(), class, "{class:?}"),
            None => assert!(
                matches!(class, Ipc4761::II | Ipc4761::Custom),
                "{class:?} has a preset"
            ),
        }
    }
}

#[test]
fn every_numbered_type_has_its_designation() {
    let codes: Vec<Option<&str>> = Ipc4761::ALL.into_iter().map(Ipc4761::code).collect();
    assert_eq!(
        codes,
        [
            None,
            Some("I-a"),
            Some("I-b"),
            Some("II"),
            Some("III-a"),
            Some("III-b"),
            Some("IV-a"),
            Some("IV-b"),
            Some("V"),
            Some("VI-a"),
            Some("VI-b"),
            Some("VII"),
            None,
        ]
    );
}

#[test]
fn a_via_reads_its_stored_tenting_and_no_fill_or_cap() {
    let mut via = Via {
        uuid: Uuid::from_u128(1),
        position: Point::new(0.0, 0.0),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 0,
        via_type: ViaType::Through,
        locked: false,
        layer_sizes: None,
        attributes: Default::default(),
        unknown_columns: Default::default(),
    };
    assert_eq!(ViaProtection::of_via(&via).class(), Ipc4761::Open);
    via.attributes.bottom.tented = true;
    let read = ViaProtection::of_via(&via);
    assert_eq!(read.fill, ViaFill::None);
    assert!(!read.capped);
    assert!(!read.tented_top && read.tented_bottom);
    assert_eq!(read.class(), Ipc4761::IA);
    via.attributes.top.tented = true;
    assert_eq!(ViaProtection::of_via(&via).class(), Ipc4761::IB);
}
