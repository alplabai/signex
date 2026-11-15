//! The IPC-D-356 bare-board test netlist: fixed 80-column records, metric
//! units (`UNITS CUST 1`: millimetres in thousandths). See
//! `docs/FABRICATION_OUTPUTS.md`, "Test netlist".

use pcb_geom::{PointNm, mm_to_nm, pad_world_rotation_deg};
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard};

use super::collect::{pad_aperture, pad_openings};
use super::frame::{div_round, fab_angle_udeg, fab_point};
use super::layers::{CopperStack, FabLayer, Face};

/// The longest net name a record holds; a longer one gets an alias.
const NET_FIELD: usize = 14;

/// Where a test point can be probed from.
#[derive(Clone, Copy)]
enum Access {
    Both,
    Top,
    Bottom,
}

/// One test record before it is laid out.
struct Record {
    code: &'static str,
    net: String,
    designator: String,
    pin: String,
    /// Drill diameter in nm and whether it is plated.
    hole: Option<(i64, bool)>,
    access: Access,
    at: PointNm,
    size: (i64, i64),
    udeg: i64,
    /// The faces whose mask covers the point.
    covered: (bool, bool),
}

pub(crate) fn write_ipc356(
    board: &PcbBoard,
    stack: &CopperStack,
    name: &str,
    skipped: &mut usize,
) -> String {
    let mut records = Vec::new();
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            match pad_record(board, footprint, pad) {
                Some(record) => records.push(record),
                None => *skipped += 1,
            }
        }
    }
    for via in &board.vias {
        let at = PointNm::from_mm(via.position, "via.position");
        let size = mm_to_nm(via.diameter, "via.diameter");
        let drill = mm_to_nm(via.drill, "via.drill");
        let (Ok(at), Ok(size), Ok(drill)) = (at, size, drill) else {
            *skipped += 1;
            continue;
        };
        records.push(Record {
            code: "317",
            net: net_name(board, via.net),
            designator: "VIA".to_string(),
            pin: String::new(),
            hole: Some((drill.abs(), true)),
            access: Access::Both,
            at: fab_point(at),
            size: (size.abs(), size.abs()),
            udeg: 0,
            covered: (true, true),
        });
    }
    let aliases = aliases(&records);
    let mut out = String::new();
    out.push_str("C  IPC-D-356 bare-board test netlist\n");
    out.push_str(&format!("P  JOB   {name}\n"));
    out.push_str("P  UNITS CUST 1\n");
    for (index, net) in aliases.iter().enumerate() {
        out.push_str(&format!("P  NNAME{} {net}\n", index + 1));
    }
    for record in &records {
        let net = match aliases.iter().position(|n| *n == record.net) {
            Some(index) => format!("NNAME{}", index + 1),
            None => record.net.clone(),
        };
        out.push_str(layout(record, &net, stack.count()).trim_end());
        out.push('\n');
    }
    out.push_str("999\n");
    out
}

fn net_name(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|n| n.number == net && net != 0 && !n.name.is_empty())
        .map(|n| n.name.clone())
        .unwrap_or_else(|| "N/C".to_string())
}

/// Every net name too long for its field, once, in first-use order.
fn aliases(records: &[Record]) -> Vec<String> {
    let mut long: Vec<String> = Vec::new();
    for record in records {
        if record.net.chars().count() > NET_FIELD && !long.contains(&record.net) {
            long.push(record.net.clone());
        }
    }
    long
}

fn pad_record(board: &PcbBoard, footprint: &Footprint, pad: &Pad) -> Option<Record> {
    let (_, at) = pad_aperture(footprint, pad).ok()?;
    let udeg = fab_angle_udeg(pad_world_rotation_deg(footprint, pad).ok()?);
    let size = (
        mm_to_nm(pad.size.x, "pad.size").ok()?.abs(),
        mm_to_nm(pad.size.y, "pad.size").ok()?.abs(),
    );
    let hole = match &pad.drill {
        Some(drill) if drill.diameter > 0.0 => Some((
            mm_to_nm(drill.diameter, "pad.drill").ok()?.abs(),
            pad.pad_type != PadType::NpThru,
        )),
        _ => None,
    };
    let net = pad
        .net
        .as_ref()
        .map(|n| n.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| net_name(board, pad.net.as_ref().map_or(0, |n| n.number)));
    let access = match (&hole, pcb_geom::Side::of_layer(&footprint.layer)) {
        (Some(_), _) => Access::Both,
        (None, pcb_geom::Side::Top) => Access::Top,
        (None, pcb_geom::Side::Bottom) => Access::Bottom,
    };
    let openings = pad_openings(footprint, pad);
    let covered = (
        !openings.contains(&FabLayer::Mask(Face::Top)),
        !openings.contains(&FabLayer::Mask(Face::Bottom)),
    );
    Some(Record {
        code: if hole.is_some() { "317" } else { "327" },
        net,
        designator: footprint.reference.clone(),
        pin: pad.number.clone(),
        hole,
        access,
        at,
        size,
        udeg,
        covered,
    })
}

/// A record in its fixed columns (1-based, as the format counts them):
/// code 1-3, net 4-17, designator 21-26, `-` 27, pin 28-31, hole 33-38,
/// access 39-41, X 42-49, Y 50-57, size 58-71, mask 73-74.
fn layout(record: &Record, net: &str, layers: usize) -> String {
    let mut line = vec![' '; 80];
    let mut put = |column: usize, text: &str| {
        for (i, c) in text.chars().enumerate() {
            if let Some(slot) = line.get_mut(column - 1 + i) {
                *slot = c;
            }
        }
    };
    put(1, record.code);
    put(4, &fit(net, NET_FIELD));
    put(21, &fit(&record.designator, 6));
    if !record.pin.is_empty() {
        put(27, "-");
        put(28, &fit(&record.pin, 4));
    }
    if let Some((diameter, plated)) = record.hole {
        put(33, &format!("D{:04}", micrometres(diameter).min(9999)));
        put(38, if plated { "P" } else { "U" });
    }
    let access = match record.access {
        Access::Both => 0,
        Access::Top => 1,
        Access::Bottom => layers,
    };
    put(39, &format!("A{access:02}"));
    put(42, &signed('X', micrometres(record.at.x)));
    put(50, &signed('Y', micrometres(record.at.y)));
    let (w, h) = record.size;
    put(58, &format!("X{:04}", micrometres(w).min(9999)));
    put(63, &format!("Y{:04}", micrometres(h).min(9999)));
    let degrees = div_round(record.udeg, 1_000_000).rem_euclid(360);
    put(68, &format!("R{degrees:03}"));
    let mask = u8::from(record.covered.0) + 2 * u8::from(record.covered.1);
    put(73, &format!("S{mask}"));
    line.into_iter().collect()
}

fn micrometres(nm: i64) -> i64 {
    div_round(nm, 1_000)
}

/// `X+012345`: the axis letter, the sign and six digits, clamped.
fn signed(axis: char, value: i64) -> String {
    let sign = if value < 0 { '-' } else { '+' };
    format!("{axis}{sign}{:06}", value.unsigned_abs().min(999_999))
}

/// `text` cut to `width` characters.
fn fit(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_are_signed_six_digit_micrometres() {
        assert_eq!(signed('X', 12_345), "X+012345");
        assert_eq!(signed('Y', -5_000), "Y-005000");
        assert_eq!(signed('Y', -5_000_000), "Y-999999");
    }

    #[test]
    fn a_record_lands_in_its_columns() {
        let record = Record {
            code: "327",
            net: "GND".into(),
            designator: "R1".into(),
            pin: "1".into(),
            hole: None,
            access: Access::Top,
            at: PointNm::new(4_000_000, -5_000_000),
            size: (1_000_000, 1_200_000),
            udeg: 0,
            covered: (false, true),
        };
        let line = layout(&record, "GND", 2);
        assert_eq!(line.len(), 80);
        assert_eq!(&line[0..3], "327");
        assert_eq!(&line[3..6], "GND");
        assert_eq!(&line[20..22], "R1");
        assert_eq!(&line[26..28], "-1");
        assert_eq!(&line[38..41], "A01");
        assert_eq!(&line[41..57], "X+004000Y-005000");
        assert_eq!(&line[57..71], "X1000Y1200R000");
        assert_eq!(&line[72..74], "S2");
    }
}
