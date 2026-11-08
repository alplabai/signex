//! Generated 10,000-track board for the PCB render benchmark (#2176).
//!
//! Deterministic: every uuid counts up from a fixed base per object kind, and
//! every coordinate is a whole number of nanometres, so the board survives a
//! `.snxpcb` save and reopen unchanged. Layout and how to run the benchmark:
//! `docs/architecture/pcb-render-bench.md`.

use signex_types::pcb::{
    Footprint, LayerDef, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment, Via,
    ViaType, Zone,
};
use uuid::Uuid;

/// Routed rows; each is one net running left to right on one copper layer.
pub const ROW_COUNT: usize = 100;
/// Track segments per row.
pub const SEGMENTS_PER_ROW: usize = 100;
/// A via sits on every `VIA_EVERY`-th joint of a row.
pub const VIA_EVERY: usize = 10;

pub const TRACK_COUNT: usize = ROW_COUNT * SEGMENTS_PER_ROW;
pub const VIA_COUNT: usize = ROW_COUNT * (SEGMENTS_PER_ROW / VIA_EVERY);
/// One two-pad part at each end of every row.
pub const FOOTPRINT_COUNT: usize = ROW_COUNT * 2;
pub const PAD_COUNT: usize = FOOTPRINT_COUNT * 2;
/// Ground pours on the two inner layers.
pub const ZONE_COUNT: usize = 2;

/// Board outline, in nm.
pub const BOARD_WIDTH_NM: i64 = 205_000_000;
pub const BOARD_HEIGHT_NM: i64 = 130_000_000;

const MARGIN_NM: i64 = 5_000_000;
const ROW_PITCH_NM: i64 = 1_200_000;
const SEGMENT_PITCH_NM: i64 = 1_900_000;
/// Every fifth joint the row steps up or down by this much, so a row is a
/// run of horizontal segments joined by 45-degree-ish jogs.
const JOG_NM: i64 = 500_000;
const PAD_OFFSET_NM: i64 = 800_000;

const COPPER_LAYERS: [&str; 4] = ["TopCopper", "InnerCopper1", "InnerCopper2", "BottomCopper"];
/// Rows use layers 0, 3, 0, 3, ... with every third row on an inner layer,
/// so all four layers carry tracks.
const ROW_LAYER_CYCLE: [usize; 6] = [0, 3, 1, 0, 3, 2];
const TRACK_WIDTHS_NM: [i64; 4] = [150_000, 200_000, 250_000, 400_000];

const GND_NET: u32 = ROW_COUNT as u32 + 1;

const UUID_BOARD: u128 = 1;
const UUID_FOOTPRINT_BASE: u128 = 1_000;
const UUID_PAD_BASE: u128 = 2_000;
const UUID_SEGMENT_BASE: u128 = 10_000;
const UUID_VIA_BASE: u128 = 30_000;
const UUID_ZONE_BASE: u128 = 40_000;

/// The same nm-to-mm step the `.snxpcb` reader takes, so a reopened board
/// compares equal to the generated one.
fn mm(nm: i64) -> f64 {
    nm as f64 * 1.0e-6
}

fn point(x_nm: i64, y_nm: i64) -> Point {
    Point {
        x: mm(x_nm),
        y: mm(y_nm),
    }
}

fn row_net(row: usize) -> u32 {
    row as u32 + 1
}

/// The joint `joint` (0..=SEGMENTS_PER_ROW) of row `row`, in nm.
fn joint_nm(row: usize, joint: usize) -> (i64, i64) {
    let x = MARGIN_NM + PAD_OFFSET_NM * 2 + joint as i64 * SEGMENT_PITCH_NM;
    let jog = if (joint / 5) % 2 == 1 { JOG_NM } else { 0 };
    let y = MARGIN_NM + row as i64 * ROW_PITCH_NM + jog;
    (x, y)
}

/// Build the benchmark board. The same call always returns the same board.
pub fn board_10k_tracks() -> PcbBoard {
    let mut segments = Vec::with_capacity(TRACK_COUNT);
    let mut vias = Vec::with_capacity(VIA_COUNT);
    let mut footprints = Vec::with_capacity(FOOTPRINT_COUNT);

    for row in 0..ROW_COUNT {
        let layer = COPPER_LAYERS[ROW_LAYER_CYCLE[row % ROW_LAYER_CYCLE.len()]];
        let width = mm(TRACK_WIDTHS_NM[(row / ROW_LAYER_CYCLE.len()) % TRACK_WIDTHS_NM.len()]);
        let net = row_net(row);

        for seg in 0..SEGMENTS_PER_ROW {
            let (x0, y0) = joint_nm(row, seg);
            let (x1, y1) = joint_nm(row, seg + 1);
            segments.push(Segment {
                uuid: Uuid::from_u128(UUID_SEGMENT_BASE + segments.len() as u128),
                start: point(x0, y0),
                end: point(x1, y1),
                width,
                layer: layer.to_string(),
                net,
                unknown_columns: Default::default(),
            });
        }

        for joint in (VIA_EVERY..=SEGMENTS_PER_ROW).step_by(VIA_EVERY) {
            let (x, y) = joint_nm(row, joint);
            vias.push(Via {
                uuid: Uuid::from_u128(UUID_VIA_BASE + vias.len() as u128),
                position: point(x, y),
                diameter: mm(600_000),
                drill: mm(300_000),
                layers: vec![COPPER_LAYERS[0].to_string(), COPPER_LAYERS[3].to_string()],
                net,
                via_type: ViaType::Through,
                unknown_columns: Default::default(),
            });
        }

        let (start_x, start_y) = joint_nm(row, 0);
        let (end_x, end_y) = joint_nm(row, SEGMENTS_PER_ROW);
        // The row's first pad lands on the track end; the other one is GND.
        let start = (start_x - PAD_OFFSET_NM, start_y);
        let end = (end_x + PAD_OFFSET_NM, end_y);
        footprints.push(two_pad_part(
            footprints.len(),
            format!("R{}", row + 1),
            start,
            [(PAD_OFFSET_NM, net), (-PAD_OFFSET_NM, GND_NET)],
        ));
        footprints.push(two_pad_part(
            footprints.len(),
            format!("C{}", row + 1),
            end,
            [(-PAD_OFFSET_NM, net), (PAD_OFFSET_NM, GND_NET)],
        ));
    }

    PcbBoard {
        uuid: Uuid::from_u128(UUID_BOARD),
        version: 1,
        generator: "signex-renderer pcb_10k_track_fixture".to_string(),
        thickness: 1.6,
        outline: vec![
            point(0, 0),
            point(BOARD_WIDTH_NM, 0),
            point(BOARD_WIDTH_NM, BOARD_HEIGHT_NM),
            point(0, BOARD_HEIGHT_NM),
        ],
        layers: COPPER_LAYERS
            .iter()
            .enumerate()
            .map(|(id, name)| LayerDef {
                id: id as u8,
                name: (*name).to_string(),
                layer_type: "signal".to_string(),
                display_name: String::new(),
            })
            .collect(),
        setup: None,
        nets: nets(),
        footprints,
        segments,
        vias,
        zones: vec![
            ground_pour(0, COPPER_LAYERS[1]),
            ground_pour(1, COPPER_LAYERS[2]),
        ],
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn nets() -> Vec<NetDef> {
    let mut nets = Vec::with_capacity(ROW_COUNT + 2);
    nets.push(NetDef {
        number: 0,
        name: String::new(),
    });
    nets.extend((0..ROW_COUNT).map(|row| NetDef {
        number: row_net(row),
        name: format!("SIG{}", row + 1),
    }));
    nets.push(NetDef {
        number: GND_NET,
        name: "GND".to_string(),
    });
    nets
}

fn net_name(net: u32) -> String {
    if net == GND_NET {
        "GND".to_string()
    } else {
        format!("SIG{net}")
    }
}

/// A two-pad SMD part at `origin_nm`; `pads` is (x offset in nm, net) per pad.
fn two_pad_part(
    index: usize,
    reference: String,
    origin_nm: (i64, i64),
    pads: [(i64, u32); 2],
) -> Footprint {
    let footprint_index = index as u128;
    let pads = pads
        .iter()
        .enumerate()
        .map(|(pad_index, &(offset_nm, net))| Pad {
            uuid: Uuid::from_u128(UUID_PAD_BASE + footprint_index * 2 + pad_index as u128),
            number: (pad_index + 1).to_string(),
            pad_type: PadType::Smd,
            shape: PadShape::Rect,
            position: point(offset_nm, 0),
            size: point(800_000, 900_000),
            drill: None,
            layers: vec![COPPER_LAYERS[0].to_string()],
            net: Some(PadNet {
                number: net,
                name: net_name(net),
            }),
            roundrect_ratio: 0.0,
            net_locked: false,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        })
        .collect();

    Footprint {
        uuid: Uuid::from_u128(UUID_FOOTPRINT_BASE + footprint_index),
        reference,
        value: "10k".to_string(),
        footprint_id: "R0603.snxfpt".to_string(),
        position: point(origin_nm.0, origin_nm.1),
        rotation: 0.0,
        layer: COPPER_LAYERS[0].to_string(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn ground_pour(index: u128, layer: &str) -> Zone {
    let inset = 1_000_000;
    Zone {
        uuid: Uuid::from_u128(UUID_ZONE_BASE + index),
        net: GND_NET,
        net_name: "GND".to_string(),
        layer: layer.to_string(),
        outline: vec![
            point(inset, inset),
            point(BOARD_WIDTH_NM - inset, inset),
            point(BOARD_WIDTH_NM - inset, BOARD_HEIGHT_NM - inset),
            point(inset, BOARD_HEIGHT_NM - inset),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: "solid".to_string(),
        thermal_relief: true,
        thermal_gap: 0.5,
        thermal_width: 0.5,
        clearance: 0.2,
        min_thickness: 0.2,
        unknown: Default::default(),
    }
}
