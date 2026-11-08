//! Pad count and pad pitch of a footprint, as a part summary states them
//! ("0.95 mm pitch, 5 pads").

use std::collections::HashMap;

use super::{Footprint, PadKind};

/// Pad coordinates are rounded to this many µm; pads in the same bucket share a row or column.
const ROW_TOLERANCE_UM: f64 = 1.0;
/// A row needs this many pads before its spacing reads as a pitch: two pads
/// are a span (a chip resistor), not a pitch.
const MIN_PADS_PER_ROW: usize = 3;

impl Footprint {
    /// Pads that carry copper: every pad but a non-plated mounting hole.
    pub fn copper_pad_count(&self) -> usize {
        self.pads
            .iter()
            .filter(|pad| pad.kind != PadKind::NptHole)
            .count()
    }

    /// The smallest centre spacing of the copper pads along a row or column
    /// of at least three pads, rounded to 1 µm. `None` when no row is that
    /// long, so a two-pad part reports no pitch.
    pub fn pad_pitch_mm(&self) -> Option<f64> {
        let centres: Vec<[f64; 2]> = self
            .pads
            .iter()
            .filter(|pad| pad.kind != PadKind::NptHole)
            .map(|pad| pad.position)
            .filter(|p| p[0].is_finite() && p[1].is_finite())
            .collect();
        // Rows share y and run along x; columns share x and run along y.
        let rows = min_spacing(&centres, 1, 0);
        let columns = min_spacing(&centres, 0, 1);
        let pitch = match (rows, columns) {
            (Some(a), Some(b)) => a.min(b),
            (a, b) => a.or(b)?,
        };
        Some((pitch * 1000.0).round() / 1000.0)
    }
}

/// The smallest gap along axis `along` between neighbours that share axis
/// `shared`, over groups of at least [`MIN_PADS_PER_ROW`] pads.
fn min_spacing(centres: &[[f64; 2]], shared: usize, along: usize) -> Option<f64> {
    let mut groups: HashMap<i64, Vec<f64>> = HashMap::new();
    for c in centres {
        let key = (c[shared] * 1000.0 / ROW_TOLERANCE_UM).round() as i64;
        groups.entry(key).or_default().push(c[along]);
    }
    groups
        .into_values()
        .filter(|g| g.len() >= MIN_PADS_PER_ROW)
        .filter_map(|mut g| {
            g.sort_by(f64::total_cmp);
            g.windows(2)
                .map(|w| w[1] - w[0])
                .filter(|gap| *gap * 1000.0 >= ROW_TOLERANCE_UM)
                .min_by(f64::total_cmp)
        })
        .min_by(f64::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::super::{Footprint, Pad, PadKind};

    fn footprint(pads: &[(f64, f64, PadKind)]) -> Footprint {
        let mut fp = Footprint::empty("test");
        fp.pads = pads
            .iter()
            .enumerate()
            .map(|(i, (x, y, kind))| Pad {
                number: (i + 1).to_string(),
                kind: *kind,
                position: [*x, *y],
                ..Pad::default()
            })
            .collect();
        fp
    }

    #[test]
    fn a_sot23_5_has_a_0_95_mm_pitch_and_five_pads() {
        let smd = PadKind::Smd;
        let fp = footprint(&[
            (-0.95, 1.1, smd),
            (0.0, 1.1, smd),
            (0.95, 1.1, smd),
            (0.95, -1.1, smd),
            (-0.95, -1.1, smd),
        ]);
        assert_eq!(fp.copper_pad_count(), 5);
        assert_eq!(fp.pad_pitch_mm(), Some(0.95));
    }

    #[test]
    fn a_column_of_pads_reads_its_pitch_along_y() {
        let smd = PadKind::Smd;
        let fp = footprint(&[(-1.4, -0.5, smd), (-1.4, 0.0, smd), (-1.4, 0.5, smd)]);
        assert_eq!(fp.pad_pitch_mm(), Some(0.5));
    }

    #[test]
    fn a_two_pad_chip_has_no_pitch() {
        let smd = PadKind::Smd;
        let fp = footprint(&[(-0.3, 0.0, smd), (0.3, 0.0, smd)]);
        assert_eq!(fp.copper_pad_count(), 2);
        assert_eq!(fp.pad_pitch_mm(), None);
    }

    #[test]
    fn mounting_holes_are_neither_counted_nor_spaced() {
        let fp = footprint(&[
            (-1.0, 0.0, PadKind::NptHole),
            (0.0, 0.0, PadKind::NptHole),
            (1.0, 0.0, PadKind::NptHole),
            (0.0, 2.0, PadKind::Tht),
        ]);
        assert_eq!(fp.copper_pad_count(), 1);
        assert_eq!(fp.pad_pitch_mm(), None);
    }
}
