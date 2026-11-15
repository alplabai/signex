//! Excellon drill files, plated and non-plated apart, with slots, and the
//! drill table. Format: metric, absolute, decimal point, three decimals
//! (micrometres). See `docs/FABRICATION_OUTPUTS.md`, "Drill".

use pcb_geom::{GeomError, PointNm, mm_to_nm, pad_world_center, pad_world_rotation_deg};
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard};

use super::frame::{fab_point, mm3};
use super::gerber::GENERATION_SOFTWARE;

/// One drilled hole or routed slot, in the fabrication frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hole {
    pub at: PointNm,
    pub diameter: i64,
    pub plated: bool,
    /// The far end of a slot: the tool runs from `at` to here.
    pub slot_to: Option<PointNm>,
}

/// Every hole of the board, in board order: pads, vias, then the board
/// sketch's mounting holes. A hole of no diameter is not drilled.
pub(crate) fn collect_holes(board: &PcbBoard, skipped: &mut usize) -> Vec<Hole> {
    let mut holes = Vec::new();
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            match pad_hole(footprint, pad) {
                Ok(Some(hole)) => holes.push(hole),
                Ok(None) => {}
                Err(_) => *skipped += 1,
            }
        }
    }
    let vias = board.vias.iter().map(|via| (via.position, via.drill, true));
    let mounting = board
        .sketch
        .iter()
        .flat_map(|s| s.holes.iter())
        .map(|hole| (hole.position, hole.drill, hole.plated));
    for (position, drill, plated) in vias.chain(mounting) {
        let built = PointNm::from_mm(position, "hole.position")
            .and_then(|at| Ok((at, mm_to_nm(drill, "hole.drill")?.abs())));
        match built {
            Ok((_, 0)) => {}
            Ok((at, diameter)) => holes.push(Hole {
                at: fab_point(at),
                diameter,
                plated,
                slot_to: None,
            }),
            Err(_) => *skipped += 1,
        }
    }
    holes
}

/// A pad's hole. An oval drill is a slot along the pad's longer side,
/// keeping the annular ring the pad has across it: its length is the
/// drill width plus the pad's length minus its width.
fn pad_hole(footprint: &Footprint, pad: &Pad) -> Result<Option<Hole>, GeomError> {
    let Some(drill) = &pad.drill else {
        return Ok(None);
    };
    let diameter = mm_to_nm(drill.diameter, "pad.drill")?.abs();
    if diameter == 0 {
        return Ok(None);
    }
    let center = pad_world_center(footprint, pad)?;
    let plated = pad.pad_type != PadType::NpThru;
    let w = mm_to_nm(pad.size.x, "pad.size")?.abs();
    let h = mm_to_nm(pad.size.y, "pad.size")?.abs();
    let reach = (w.max(h) - w.min(h)) / 2;
    if drill.shape != "oval" || reach == 0 {
        return Ok(Some(Hole {
            at: fab_point(center),
            diameter,
            plated,
            slot_to: None,
        }));
    }
    let (sin, cos) = pad_world_rotation_deg(footprint, pad)?
        .to_radians()
        .sin_cos();
    let (ax, ay) = if w > h { (1.0, 0.0) } else { (0.0, 1.0) };
    let tip = PointNm::new(
        ((ax * cos - ay * sin) * reach as f64).round() as i64,
        ((ax * sin + ay * cos) * reach as f64).round() as i64,
    );
    Ok(Some(Hole {
        at: fab_point(center.minus(tip)),
        diameter,
        plated,
        slot_to: Some(fab_point(center.plus(tip))),
    }))
}

/// The tools of one file: each distinct diameter, smallest first.
fn tools(holes: &[&Hole]) -> Vec<i64> {
    let mut tools: Vec<i64> = holes.iter().map(|h| h.diameter).collect();
    tools.sort_unstable();
    tools.dedup();
    tools
}

/// One Excellon file for the plated or the non-plated holes; `None` when
/// there are none.
pub(crate) fn write_drill(holes: &[Hole], plated: bool, layers: usize) -> Option<String> {
    let mine: Vec<&Hole> = holes.iter().filter(|h| h.plated == plated).collect();
    if mine.is_empty() {
        return None;
    }
    let tools = tools(&mine);
    let (kind, short) = if plated {
        ("Plated", "PTH")
    } else {
        ("NonPlated", "NPTH")
    };
    let mut out = String::from("M48\n");
    out.push_str(&format!(
        "; #@! TF.GenerationSoftware,{GENERATION_SOFTWARE}\n"
    ));
    out.push_str(&format!(
        "; #@! TF.FileFunction,{kind},1,{layers},{short}\n"
    ));
    out.push_str("FMAT,2\nMETRIC\n");
    for (index, diameter) in tools.iter().enumerate() {
        out.push_str(&format!("T{}C{}\n", index + 1, mm3(*diameter)));
    }
    out.push_str("%\nG90\nG05\n");
    for (index, diameter) in tools.iter().enumerate() {
        out.push_str(&format!("T{}\n", index + 1));
        for hole in mine.iter().filter(|h| h.diameter == *diameter) {
            let at = format!("X{}Y{}", mm3(hole.at.x), mm3(hole.at.y));
            match hole.slot_to {
                Some(to) => {
                    out.push_str(&format!("{at}G85X{}Y{}\n", mm3(to.x), mm3(to.y)));
                }
                None => out.push_str(&format!("{at}\n")),
            }
        }
    }
    out.push_str("T0\nM30\n");
    Some(out)
}

/// The drill table: one row per tool of each file, with its hole and slot
/// counts.
pub(crate) fn write_table(holes: &[Hole]) -> String {
    let mut out = String::from("File,Tool,Diameter (mm),Holes,Slots\n");
    for (plated, file) in [(true, "PTH"), (false, "NPTH")] {
        let mine: Vec<&Hole> = holes.iter().filter(|h| h.plated == plated).collect();
        for (index, diameter) in tools(&mine).iter().enumerate() {
            let of_tool = mine.iter().filter(|h| h.diameter == *diameter);
            let slots = of_tool.clone().filter(|h| h.slot_to.is_some()).count();
            let all = of_tool.count();
            out.push_str(&format!(
                "{file},T{},{},{},{slots}\n",
                index + 1,
                mm3(*diameter),
                all - slots
            ));
        }
    }
    out
}
