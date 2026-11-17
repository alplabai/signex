//! The pick-and-place (centroid) file: one CSV row per placed part.
//! See `docs/FABRICATION_OUTPUTS.md`, "Pick and place".

use pcb_geom::{Placement, Side};
use signex_types::pcb::PcbBoard;

use super::frame::{FabFrame, deg3, fab_angle_udeg, mm3};

const HEADER: &str = "Designator,Value,Footprint,X (mm),Y (mm),Rotation,Side\n";

/// Every footprint the BOM lists (`ComponentType::in_bom`) with a
/// designator, sorted by designator then uuid. One the BOM lists with no
/// designator has no row and is counted in `unnamed`. X and Y are the
/// footprint's origin in the fabrication frame; the rotation is
/// counter-clockwise in that frame, the turn its pads are flashed with.
pub(crate) fn write_centroid(board: &PcbBoard, unnamed: &mut usize, skipped: &mut usize) -> String {
    let mut rows: Vec<(&str, uuid::Uuid, String)> = Vec::new();
    let frame = FabFrame::of(board);
    for footprint in &board.footprints {
        if !footprint.component.component_type.in_bom() {
            continue;
        }
        if footprint.reference.is_empty() {
            *unnamed += 1;
            continue;
        }
        let Ok(placement) = Placement::of_footprint(footprint) else {
            *skipped += 1;
            continue;
        };
        let at = frame.point(placement.origin);
        let side = match placement.side {
            Side::Top => "Top",
            Side::Bottom => "Bottom",
        };
        let row = format!(
            "{},{},{},{},{},{},{side}\n",
            csv(&footprint.reference),
            csv(&footprint.value),
            csv(&footprint.footprint_id),
            mm3(at.x),
            mm3(at.y),
            deg3(fab_angle_udeg(placement.rotation_deg)),
        );
        rows.push((&footprint.reference, footprint.uuid, row));
    }
    rows.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let mut out = String::from(HEADER);
    rows.into_iter().for_each(|(_, _, row)| out.push_str(&row));
    out
}

/// A CSV field, quoted when it holds a comma, a quote or a line break.
fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_with_separators_are_quoted() {
        assert_eq!(csv("10k"), "10k");
        assert_eq!(csv("1,5k"), "\"1,5k\"");
        assert_eq!(csv("a\"b"), "\"a\"\"b\"");
    }
}
