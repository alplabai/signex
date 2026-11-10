//! A small design and board for the change list tests.

use signex_net::DesignPart;
use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::part::{FootprintResolveError, FootprintSourceRef, PinPadMap, ResolvedFootprint};
use signex_types::pcb::{Footprint, PcbBoard, Point, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::{EcoChangeList, EcoInput, EcoPart, Staging, diff_board};

pub(super) const LIB: u128 = 0x11b;

pub(super) fn u(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

pub(super) fn root() -> InstancePath {
    InstancePath::root()
}

/// A library footprint named `name` with pads numbered `pads`, in its board
/// form (top side, at the origin, empty Reference and Value texts).
pub(super) fn lib_fp(name: &str, pads: &[&str]) -> Footprint {
    let pads: Vec<serde_json::Value> = pads
        .iter()
        .enumerate()
        .map(|(i, number)| {
            serde_json::json!({
                "uuid": Uuid::from_u128(0xfff0 + i as u128), "number": number,
                "pad_type": "smd", "shape": "rect",
                "position": {"x": i as f64 * 1.27, "y": 0.0}, "size": {"x": 0.6, "y": 1.0},
                "drill": null, "layers": ["TopCopper", "TopSolderMask"], "net": null
            })
        })
        .collect();
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xfeed), "reference": "", "value": "",
        "footprint_id": name, "position": {"x": 0.0, "y": 0.0},
        "rotation": 0.0, "layer": "TopCopper", "pads": pads,
        "properties": [
            {"key": "Reference", "value": "", "position": {"x": 0.0, "y": -1.5}, "rotation": 0.0, "layer": "TopSilkscreen"},
            {"key": "Value", "value": "", "position": {"x": 0.0, "y": 1.5}, "rotation": 0.0, "layer": "TopFab"}
        ]
    }))
    .unwrap()
}

/// `name` from the test library, footprint id `fp`, at `version`.
pub(super) fn resolved(name: &str, fp: u128, version: &str, pads: &[&str]) -> ResolvedFootprint {
    let source = FootprintSourceRef {
        library: u(LIB),
        footprint: u(fp),
        version: version.into(),
    };
    let mut footprint = lib_fp(name, pads);
    footprint.library_ref = Some(source.clone());
    ResolvedFootprint {
        source,
        footprint,
        pin_map: PinPadMap::default(),
    }
}

pub(super) fn r0603() -> ResolvedFootprint {
    resolved("R0603", 0x603, "1.0.0", &["1", "2"])
}

pub(super) fn sot23() -> ResolvedFootprint {
    resolved("SOT-23", 0x23, "1.0.0", &["1", "2", "3"])
}

pub(super) fn part(
    designator: &str,
    units: &[u128],
    value: &str,
    footprint: Result<ResolvedFootprint, FootprintResolveError>,
) -> EcoPart {
    EcoPart {
        part: DesignPart {
            sheet: root(),
            designator: designator.into(),
            units: units.iter().copied().map(u).collect(),
            needs_annotation: designator.is_empty() || designator.contains('?'),
        },
        value: value.into(),
        on_board: true,
        footprint,
        read: true,
    }
}

/// A board footprint `id` built from `from`, linked to `units` on the root
/// sheet (none: unlinked), at `(x, y)`. Pad uuids are `id * 0x100 + i`.
pub(super) fn board_fp(
    id: u128,
    reference: &str,
    value: &str,
    units: &[u128],
    from: &ResolvedFootprint,
    at: (f64, f64),
) -> Footprint {
    let mut fp = from.footprint.clone();
    fp.uuid = u(id);
    fp.reference = reference.into();
    fp.value = value.into();
    fp.position = Point { x: at.0, y: at.1 };
    for (i, pad) in fp.pads.iter_mut().enumerate() {
        pad.uuid = u(id * 0x100 + i as u128);
    }
    for property in &mut fp.properties {
        match property.key.as_str() {
            "Reference" => property.value = reference.into(),
            "Value" => property.value = value.into(),
            _ => {}
        }
    }
    fp.schematic_link =
        (!units.is_empty()).then(|| SymbolLink::new(root(), units.iter().copied().map(u)));
    fp
}

/// An empty 50 x 30 mm board holding `footprints`.
pub(super) fn board(footprints: Vec<Footprint>) -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0}, {"x": 50.0, "y": 30.0}, {"x": 0.0, "y": 30.0}]
    }))
    .unwrap();
    board.footprints = footprints;
    board
}

/// One pin of a test net: `(unit, reference, pin)`.
pub(super) type Pin<'a> = (u128, &'a str, &'a str);

/// A netlist of `(name, [pin])` nets on the root sheet.
pub(super) fn netlist(nets: &[(&str, &[Pin<'_>])]) -> Netlist {
    Netlist {
        nets: nets
            .iter()
            .enumerate()
            .map(|(i, (name, terminals))| Net {
                id: NetId(i as u32 + 1),
                name: (*name).into(),
                class: None,
                wires: Vec::new(),
                junctions: Vec::new(),
                terminals: terminals
                    .iter()
                    .map(|(unit, reference, pin)| Terminal {
                        symbol: u(*unit),
                        reference: (*reference).into(),
                        pin: (*pin).into(),
                        occurrence: root(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Everything `diff_board` reads, owned.
#[derive(Clone)]
pub(super) struct Design {
    pub board: PcbBoard,
    pub revision: u64,
    pub parts: Vec<EcoPart>,
    pub netlist: Netlist,
    pub unreadable: Vec<InstancePath>,
}

impl Design {
    pub fn new(board: PcbBoard, parts: Vec<EcoPart>, netlist: Netlist) -> Self {
        Self {
            board,
            revision: 1,
            parts,
            netlist,
            unreadable: Vec::new(),
        }
    }

    pub fn input(&self) -> EcoInput<'_> {
        EcoInput {
            board: &self.board,
            revision: self.revision,
            parts: &self.parts,
            netlist: &self.netlist,
            unreadable: &self.unreadable,
            staging: Staging::DEFAULT,
        }
    }

    pub fn diff(&self) -> EcoChangeList {
        diff_board(&self.input())
    }
}
