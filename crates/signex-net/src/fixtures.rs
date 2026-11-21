//! Sheet fixtures shared by the build and project test modules.
use signex_types::schematic::{Junction, Label, LabelType, Point, SchematicSheet, Wire};
use uuid::Uuid;

pub(crate) fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(crate) fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

pub(crate) fn wire(a: Point, b: Point) -> Wire {
    Wire {
        start: a,
        end: b,
        ..Default::default()
    }
}

pub(crate) fn junction(pos: Point) -> Junction {
    Junction {
        position: pos,
        ..Default::default()
    }
}

pub(crate) fn label(text: &str, pos: Point, ty: LabelType) -> Label {
    Label {
        uuid: Uuid::nil(),
        ..Label::new(text, pos, ty)
    }
}

/// The four-channel buck project of the multi-channel tests (#1279):
/// `top.snxsch` places `power_in.snxsch`, `buck_channel.snxsch` as
/// `Repeat(CH, 1, 4)` (entries VIN_12V, EN, GND and `Repeat(VOUT_3V3)`) and
/// `mcu.snxsch`, whose `VOUT_3V3[1..4]` entry meets the repeated entry over
/// one bus. Every sheet's key is its file name.
pub(crate) mod multi_channel {
    use std::collections::HashMap;

    use signex_types::schematic::{
        Bus, BusEntry, ChildSheet, FillType, LabelType, LibPin, LibSymbol, Pin, PinDirection,
        PinShapeStyle, Point, SchematicSheet, SheetPin, Symbol,
    };
    use uuid::Uuid;

    use super::{empty_sheet, label, pt, wire};
    use crate::{ProjectGraph, ProjectRoot, SheetKey};

    pub(crate) const TOP: &str = "top.snxsch";
    pub(crate) const POWER_IN: &str = "power_in.snxsch";
    pub(crate) const BUCK: &str = "buck_channel.snxsch";
    pub(crate) const MCU: &str = "mcu.snxsch";
    pub(crate) const BUCK_SYMBOL: Uuid = Uuid::from_u128(0x12);

    /// The nine one-pin parts of one buck channel.
    pub(crate) const BUCK_PARTS: [&str; 9] = ["U1", "L1", "D1", "R4", "R5", "C3", "C4", "C5", "C6"];

    /// The sheets, resolution map and roots of a [`ProjectGraph`].
    pub(crate) struct Project {
        pub sheets: HashMap<SheetKey, SchematicSheet>,
        pub resolved: HashMap<SheetKey, HashMap<String, SheetKey>>,
        pub roots: Vec<ProjectRoot>,
    }

    impl Project {
        pub(crate) fn graph(&self) -> ProjectGraph<'_> {
            ProjectGraph {
                sheets: &self.sheets,
                resolved: &self.resolved,
                roots: &self.roots,
            }
        }

        pub(crate) fn sheet_mut(&mut self, key: &str) -> &mut SchematicSheet {
            self.sheets
                .get_mut(&SheetKey::new(key))
                .unwrap_or_else(|| panic!("no sheet {key}"))
        }
    }

    /// The project as described above.
    pub(crate) fn project() -> Project {
        let sheets = HashMap::from([
            (SheetKey::new(TOP), top()),
            (SheetKey::new(POWER_IN), power_in()),
            (SheetKey::new(BUCK), buck_channel()),
            (SheetKey::new(MCU), mcu()),
        ]);
        let resolved = sheets
            .iter()
            .map(|(key, sheet)| {
                let submap = sheet
                    .child_sheets
                    .iter()
                    .map(|cs| (cs.filename.clone(), SheetKey::new(cs.filename.clone())))
                    .collect();
                (key.clone(), submap)
            })
            .collect();
        Project {
            sheets,
            resolved,
            roots: vec![ProjectRoot {
                key: SheetKey::new(TOP),
            }],
        }
    }

    fn top() -> SchematicSheet {
        let mut s = empty_sheet();
        let entries = |x: f64| {
            vec![
                entry("VIN_12V", pt(x, 10.0)),
                entry("EN", pt(x, 20.0)),
                entry("GND", pt(x, 30.0)),
            ]
        };
        s.child_sheets
            .push(sheet_symbol(0x11, "power_in", POWER_IN, entries(20.0)));
        let mut buck = entries(50.0);
        buck.push(entry("Repeat(VOUT_3V3)", pt(80.0, 15.0)));
        s.child_sheets
            .push(sheet_symbol(0x12, "Repeat(CH, 1, 4)", BUCK, buck));
        s.child_sheets.push(sheet_symbol(
            0x13,
            "mcu",
            MCU,
            vec![entry("VOUT_3V3[1..4]", pt(120.0, 15.0))],
        ));
        for y in [10.0, 20.0, 30.0] {
            s.wires.push(wire(pt(20.0, y), pt(50.0, y)));
        }
        s.labels
            .push(label("VIN_12V", pt(35.0, 10.0), LabelType::Net));
        add_lib(&mut s, "PWR");
        place(&mut s, 0x100, "#PWR1", "PWR", pt(20.0, 30.0));
        let gnd = s.symbols.last_mut().expect("just placed");
        gnd.is_power = true;
        gnd.value = "GND".to_string();
        s.buses.push(Bus {
            uuid: Uuid::from_u128(0x200),
            start: pt(80.0, 15.0),
            end: pt(120.0, 15.0),
        });
        s.labels
            .push(label("VOUT_3V3[1..4]", pt(100.0, 15.0), LabelType::Net));
        s
    }

    fn power_in() -> SchematicSheet {
        let mut s = empty_sheet();
        add_lib(&mut s, "P");
        for (i, (part, port)) in [("J1", "VIN_12V"), ("R1", "EN"), ("J2", "GND")]
            .into_iter()
            .enumerate()
        {
            let y = 10.0 * i as f64;
            hier_stub(&mut s, 0x300 + i as u128, part, port, y);
        }
        s
    }

    /// U1 and C5 on VIN_12V, C3 and C6 on GND, R4 on EN, L1 and C4 on the
    /// repeated VOUT_3V3, R5 and D1 on the channel-local net FB.
    fn buck_channel() -> SchematicSheet {
        let mut s = empty_sheet();
        add_lib(&mut s, "P");
        hier_stub(&mut s, 0x400, "U1", "VIN_12V", 0.0);
        place(&mut s, 0x401, "C5", "P", pt(10.0, 0.0));
        hier_stub(&mut s, 0x402, "C3", "GND", 10.0);
        place(&mut s, 0x403, "C6", "P", pt(10.0, 10.0));
        hier_stub(&mut s, 0x404, "R4", "EN", 20.0);
        s.wires.push(wire(pt(0.0, 30.0), pt(10.0, 30.0)));
        place(&mut s, 0x405, "L1", "P", pt(0.0, 30.0));
        place(&mut s, 0x406, "C4", "P", pt(10.0, 30.0));
        s.labels
            .push(label("VOUT_3V3", pt(5.0, 30.0), LabelType::Hierarchical));
        s.wires.push(wire(pt(0.0, 40.0), pt(10.0, 40.0)));
        place(&mut s, 0x407, "R5", "P", pt(0.0, 40.0));
        place(&mut s, 0x408, "D1", "P", pt(10.0, 40.0));
        s.labels.push(label("FB", pt(5.0, 40.0), LabelType::Net));
        s
    }

    /// A Hierarchical `VOUT_3V3[1..4]` bus with one entry, member label and
    /// resistor (R11..R14) per member.
    fn mcu() -> SchematicSheet {
        let mut s = empty_sheet();
        add_lib(&mut s, "P");
        s.buses.push(Bus {
            uuid: Uuid::from_u128(0x500),
            start: pt(0.0, 0.0),
            end: pt(0.0, 50.0),
        });
        s.labels.push(label(
            "VOUT_3V3[1..4]",
            pt(0.0, 0.0),
            LabelType::Hierarchical,
        ));
        for i in 1..=4u32 {
            let y = 10.0 * f64::from(i);
            s.bus_entries.push(BusEntry {
                uuid: Uuid::from_u128(0x510 + u128::from(i)),
                position: pt(0.0, y),
                size: (2.54, 2.54),
            });
            let end = y + 2.54;
            s.wires.push(wire(pt(2.54, end), pt(12.54, end)));
            place(
                &mut s,
                0x520 + u128::from(i),
                &format!("R1{i}"),
                "P",
                pt(12.54, end),
            );
            s.labels
                .push(label(&format!("VOUT_3V3{i}"), pt(6.0, end), LabelType::Net));
        }
        s
    }

    /// A part at `(0, y)` wired to a Hierarchical port at `(10, y)`.
    fn hier_stub(s: &mut SchematicSheet, id: u128, part: &str, port: &str, y: f64) {
        s.wires.push(wire(pt(0.0, y), pt(10.0, y)));
        place(s, id, part, "P", pt(0.0, y));
        s.labels
            .push(label(port, pt(10.0, y), LabelType::Hierarchical));
    }

    pub(crate) fn entry(name: &str, at: Point) -> SheetPin {
        SheetPin {
            uuid: Uuid::now_v7(),
            name: name.to_string(),
            direction: String::new(),
            position: at,
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }
    }

    pub(crate) fn sheet_symbol(
        id: u128,
        name: &str,
        filename: &str,
        pins: Vec<SheetPin>,
    ) -> ChildSheet {
        ChildSheet {
            uuid: Uuid::from_u128(id),
            name: name.to_string(),
            filename: filename.to_string(),
            position: pt(0.0, 0.0),
            size: (20.0, 30.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins,
            instances: Vec::new(),
        }
    }

    fn place(s: &mut SchematicSheet, id: u128, reference: &str, lib_id: &str, at: Point) {
        s.symbols.push(Symbol {
            uuid: Uuid::from_u128(id),
            lib_id: lib_id.to_string(),
            reference: reference.to_string(),
            position: at,
            ..Default::default()
        });
    }

    /// A one-pin library symbol whose pin sits at local `(0, 0)`.
    fn add_lib(s: &mut SchematicSheet, lib_id: &str) {
        let pin = Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: pt(0.0, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: "1".to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        };
        s.lib_symbols.insert(
            lib_id.to_string(),
            LibSymbol {
                id: lib_id.to_string(),
                reference: String::new(),
                value: String::new(),
                footprint: String::new(),
                datasheet: String::new(),
                description: String::new(),
                keywords: String::new(),
                fp_filters: String::new(),
                in_bom: true,
                on_board: true,
                in_pos_files: true,
                duplicate_pin_numbers_are_jumpers: false,
                graphics: Vec::new(),
                pins: vec![LibPin {
                    unit: 0,
                    body_style: 1,
                    pin,
                }],
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
    }
}
