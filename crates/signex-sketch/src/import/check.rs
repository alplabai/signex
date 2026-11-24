//! The profile check of an import (mockup row 179c): before anything lands,
//! what Heal and merge would fix and what it cannot. Overlaps are found on
//! the pieces as drawn; gaps and open ends on the pieces with the overlaps
//! folded, joined only where ends touch.

use super::overlap::{merge_overlaps, overlap_groups};
use super::{Ends, ImportPiece, ImportShape, profile_tags};
use crate::heal::heals;

/// Two open ends of one tag within the heal tolerance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportGap {
    /// The middle of the gap, in sketch mm.
    pub at: (f64, f64),
    pub gap_mm: f64,
    pub tag: usize,
}

/// Collinear lines of one tag that overlap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportOverlap {
    /// The middle of the first stretch two of them share, in sketch mm.
    pub at: (f64, f64),
    /// How many lines overlap there.
    pub segments: usize,
    pub tag: usize,
}

/// What the check found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileReport {
    /// The tag of each profile: edges that meet end to end, or a circle.
    pub profiles: Vec<usize>,
    /// Gaps Heal and merge closes.
    pub gaps: Vec<ImportGap>,
    /// Open ends with no other open end of their tag within the heal
    /// tolerance, with their tag: nothing heals them.
    pub open_ends: Vec<((f64, f64), usize)>,
    /// Overlaps Heal and merge folds into one line.
    pub overlaps: Vec<ImportOverlap>,
    /// Circles: closed by themselves.
    pub circles: usize,
}

impl ProfileReport {
    /// Each gap, each open end and each overlap is one problem.
    pub fn problems(&self) -> usize {
        self.gaps.len() + self.open_ends.len() + self.overlaps.len()
    }

    /// The profiles of `tag`.
    pub fn profiles_of(&self, tag: usize) -> usize {
        self.profiles.iter().filter(|t| **t == tag).count()
    }
}

/// Check `pieces` as drawn.
pub fn check_profiles(pieces: &[ImportPiece]) -> ProfileReport {
    let overlaps = overlap_groups(pieces)
        .into_iter()
        .map(|g| ImportOverlap {
            at: g.at,
            segments: g.members.len(),
            tag: pieces[g.members[0]].tag,
        })
        .collect();
    let (merged, _) = merge_overlaps(pieces);
    let mut ends = Ends::new(true, false);
    let mut edges: Vec<(usize, usize, usize)> = Vec::new();
    let mut circles: Vec<usize> = Vec::new();
    for piece in &merged {
        match piece.shape {
            ImportShape::Line { a, b }
            | ImportShape::Arc {
                start: a, end: b, ..
            } => {
                if let Some((i, j)) = ends.pair(a, b, piece.tag) {
                    edges.push((i, j, piece.tag));
                }
            }
            ImportShape::Circle { .. } => circles.push(piece.tag),
        }
    }
    let mut end_tag = vec![0; ends.ids.len()];
    for &(i, j, tag) in &edges {
        end_tag[i] = tag;
        end_tag[j] = tag;
    }
    let profiles = profile_tags(ends.ids.len(), &edges, &circles);
    let open: Vec<usize> = (0..ends.ids.len()).filter(|&i| ends.uses[i] == 1).collect();
    let (gaps, open_ends) = pair_open_ends(&ends.at, &end_tag, &open);
    ProfileReport {
        profiles,
        gaps,
        open_ends,
        overlaps,
        circles: circles.len(),
    }
}

/// Pair each open end with the nearest unpaired open end of its tag within
/// the heal tolerance; the ends left over stay open.
fn pair_open_ends(
    at: &[(f64, f64)],
    tag: &[usize],
    open: &[usize],
) -> (Vec<ImportGap>, Vec<((f64, f64), usize)>) {
    let mut paired = vec![false; open.len()];
    let (mut gaps, mut lone) = (Vec::new(), Vec::new());
    for k in 0..open.len() {
        if paired[k] {
            continue;
        }
        let (a, ta) = (at[open[k]], tag[open[k]]);
        let nearest = (k + 1..open.len())
            .filter(|&m| !paired[m] && tag[open[m]] == ta)
            .map(|m| {
                let b = at[open[m]];
                (m, (a.0 - b.0).hypot(a.1 - b.1))
            })
            .filter(|(_, gap)| heals(*gap))
            .min_by(|x, y| x.1.total_cmp(&y.1));
        match nearest {
            Some((m, gap_mm)) => {
                paired[k] = true;
                paired[m] = true;
                let b = at[open[m]];
                gaps.push(ImportGap {
                    at: ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
                    gap_mm,
                    tag: ta,
                });
            }
            None => lone.push((a, ta)),
        }
    }
    (gaps, lone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> ImportPiece {
        ImportPiece {
            shape: ImportShape::Line { a, b },
            tag: 0,
        }
    }

    fn hole(x: f64, y: f64) -> ImportPiece {
        ImportPiece {
            shape: ImportShape::Circle {
                center: (x, y),
                radius: 1.6,
            },
            tag: 1,
        }
    }

    /// The mockup's lid: a 60 x 40 outline open 0.04 mm at its top right,
    /// its bottom edge drawn as two overlapping lines, and four holes.
    fn lid() -> Vec<ImportPiece> {
        vec![
            line((0.0, 0.0), (56.96, 0.0)),
            line((57.0, 0.0), (60.0, 0.0)),
            line((60.0, 0.0), (60.0, 40.0)),
            line((60.0, 40.0), (26.0, 40.0)),
            line((30.0, 40.0), (0.0, 40.0)),
            line((0.0, 40.0), (0.0, 0.0)),
            hole(3.5, 3.5),
            hole(56.5, 3.5),
            hole(56.5, 36.5),
            hole(3.5, 36.5),
        ]
    }

    #[test]
    fn the_lid_has_a_gap_and_an_overlap_in_one_of_five_profiles() {
        let report = check_profiles(&lid());
        assert_eq!(report.problems(), 2);
        assert_eq!(report.profiles.len(), 5);
        assert_eq!(report.profiles_of(0), 1);
        assert_eq!(report.circles, 4);
        assert_eq!(report.gaps.len(), 1);
        let gap = report.gaps[0];
        assert!((gap.gap_mm - 0.04).abs() < 1e-9, "{gap:?}");
        assert!((gap.at.0 - 56.98).abs() < 1e-9 && gap.at.1.abs() < 1e-9);
        assert_eq!(report.overlaps.len(), 1);
        assert_eq!(report.overlaps[0].segments, 2);
        assert!((report.overlaps[0].at.1 - 40.0).abs() < 1e-9);
        assert!(report.open_ends.is_empty());
    }

    #[test]
    fn a_wide_gap_leaves_two_open_ends_nothing_heals() {
        let report = check_profiles(&[
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 0.0), (10.0, 10.0)),
            line((10.0, 10.0), (0.0, 10.0)),
            line((0.0, 10.0), (0.0, 0.5)),
        ]);
        assert!(report.gaps.is_empty());
        assert_eq!(report.open_ends.len(), 2);
        assert_eq!(report.problems(), 2);
        assert_eq!(report.profiles.len(), 1);
    }

    #[test]
    fn a_closed_square_has_no_problems() {
        let report = check_profiles(&[
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 0.0), (10.0, 10.0)),
            line((10.0, 10.0), (0.0, 10.0)),
            line((0.0, 10.0), (0.0, 0.0)),
        ]);
        assert_eq!(report.problems(), 0);
        assert_eq!(report.profiles, [0]);
    }
}
