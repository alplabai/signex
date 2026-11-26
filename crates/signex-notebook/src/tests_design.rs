//! The design index read from placed sheets and a netlist.

use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{
    InstancePath, LibSymbol, SchematicSheet, Symbol, power_port_lib_symbol,
};
use uuid::Uuid;

use crate::{PinName, PlacedSheet, design_index};

fn lib(id: &str, pins: &[(&str, &str)]) -> LibSymbol {
    let base = power_port_lib_symbol("power:GND");
    let template = base.pins[0].clone();
    LibSymbol {
        id: id.into(),
        description: "buck regulator".into(),
        pins: pins
            .iter()
            .map(|(number, name)| {
                let mut pin = template.clone();
                pin.pin.number = (*number).into();
                pin.pin.name = (*name).into();
                pin
            })
            .collect(),
        ..base
    }
}

fn symbol(n: u128, lib_id: &str, reference: &str) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(n),
        lib_id: lib_id.into(),
        reference: reference.into(),
        value: "Buck 3 A".into(),
        footprint: "QFN-16".into(),
        ..Symbol::default()
    }
}

fn terminal(n: u128, reference: &str, pin: &str) -> Terminal {
    Terminal {
        symbol: Uuid::from_u128(n),
        reference: reference.into(),
        pin: pin.into(),
        occurrence: InstancePath::root(),
    }
}

fn net(id: u32, name: &str, terminals: Vec<Terminal>) -> Net {
    Net {
        id: NetId(id),
        name: name.into(),
        class: None,
        wires: Vec::new(),
        junctions: Vec::new(),
        terminals,
    }
}

#[test]
fn parts_carry_their_pins_nets_sheet_and_id() {
    let mut port = symbol(9, "power:GND", "#PWR1");
    port.is_power = true;
    let sheet = SchematicSheet {
        symbols: vec![
            symbol(3, "Device:U", "U3"),
            // A second unit of U3: the first unit's entry stands.
            symbol(4, "Device:U", "U3"),
            symbol(5, "Device:U", ""),
            port,
        ],
        lib_symbols: [(
            "Device:U".to_string(),
            lib("Device:U", &[("1", "VIN"), ("3", "SW"), ("3", "SW")]),
        )]
        .into_iter()
        .collect(),
        ..SchematicSheet::default()
    };
    let netlist = Netlist {
        nets: vec![
            net(
                1,
                "SW",
                vec![terminal(3, "U3", "3"), terminal(7, "L1", "1")],
            ),
            net(2, "", vec![terminal(3, "U3", "1")]),
        ],
    };
    let root = InstancePath::root();
    let placed = [PlacedSheet {
        file: "power.snxsch",
        placement: &root,
        sheet: &sheet,
    }];
    let design = design_index(&placed, &netlist);
    assert_eq!(
        design.parts().len(),
        1,
        "power ports and blank designators are left out"
    );
    let u3 = design.part("U3").expect("U3");
    assert_eq!(u3.id, Uuid::from_u128(3));
    assert_eq!(u3.sheet, "power.snxsch");
    assert_eq!(u3.description, "buck regulator");
    assert_eq!(
        (u3.value.as_str(), u3.footprint.as_str()),
        ("Buck 3 A", "QFN-16")
    );
    assert_eq!(u3.pin_numbers(), ["1", "3"], "a pin number once");
    assert_eq!(u3.pin("3").and_then(|p| p.net.as_deref()), Some("SW"));
    let sw = design.net("SW").expect("SW");
    assert_eq!(
        sw.pins,
        [
            PinName {
                component: "L1".into(),
                pin: "1".into()
            },
            PinName {
                component: "U3".into(),
                pin: "3".into()
            },
        ]
    );
    assert_eq!(sw.sheets, ["power.snxsch"]);
    assert_eq!(design.nets().len(), 1, "an unnamed net is left out");
}
