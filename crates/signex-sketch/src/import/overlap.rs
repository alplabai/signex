//! Collinear overlapping lines of one tag (mockup row 179c): named by the
//! profile check and folded into one line by Heal and merge.

use std::collections::BTreeMap;

use super::{ImportPiece, ImportShape, SAME_MM};

/// How far (mm) an end may sit off another line and still lie on it.
const ON_LINE_MM: f64 = 1.0e-4;

type Pt = (f64, f64);

/// Lines of one tag that overlap one another.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct OverlapGroup {
    /// The pieces' indices, smallest first.
    pub members: Vec<usize>,
    /// The ends of the one line they make.
    pub span: (Pt, Pt),
    /// The middle of the first stretch two of them share.
    pub at: Pt,
}

fn line_of(piece: &ImportPiece) -> Option<(Pt, Pt)> {
    match piece.shape {
        ImportShape::Line { a, b } if dist(a, b) > SAME_MM => Some((a, b)),
        _ => None,
    }
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// How far `r` lies along line `a` from its first end, in mm.
fn along(a: (Pt, Pt), r: Pt) -> f64 {
    let (p, q) = a;
    let len = dist(p, q);
    ((r.0 - p.0) * (q.0 - p.0) + (r.1 - p.1) * (q.1 - p.1)) / len
}

/// How far `r` lies off line `a`'s line, signed, in mm.
fn off(a: (Pt, Pt), r: Pt) -> f64 {
    let (p, q) = a;
    let len = dist(p, q);
    ((r.0 - p.0) * (q.1 - p.1) - (r.1 - p.1) * (q.0 - p.0)) / len
}

/// The middle of the stretch line `b` shares with line `a`, when both ends
/// of `b` lie on `a`'s line and the stretch is longer than a point.
fn overlap_at(a: (Pt, Pt), b: (Pt, Pt)) -> Option<Pt> {
    if off(a, b.0).abs() > ON_LINE_MM || off(a, b.1).abs() > ON_LINE_MM {
        return None;
    }
    let len = dist(a.0, a.1);
    let (t0, t1) = (along(a, b.0), along(a, b.1));
    let lo = t0.min(t1).max(0.0);
    let hi = t0.max(t1).min(len);
    (hi - lo > SAME_MM).then(|| {
        let m = (lo + hi) / 2.0 / len;
        (a.0.0 + (a.1.0 - a.0.0) * m, a.0.1 + (a.1.1 - a.0.1) * m)
    })
}

/// The root of `i` in the union-find `parent`, halving the path.
pub(super) fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// The groups of overlapping lines in `pieces`, in the order of their first
/// member. Lines are swept by the left edge of their bounds.
pub(super) fn overlap_groups(pieces: &[ImportPiece]) -> Vec<OverlapGroup> {
    let mut lines: Vec<(usize, (Pt, Pt))> = pieces
        .iter()
        .enumerate()
        .filter_map(|(i, p)| line_of(p).map(|l| (i, l)))
        .collect();
    let min_x = |l: &(Pt, Pt)| l.0.0.min(l.1.0);
    let max_x = |l: &(Pt, Pt)| l.0.0.max(l.1.0);
    let y_range = |l: &(Pt, Pt)| (l.0.1.min(l.1.1), l.0.1.max(l.1.1));
    lines.sort_by(|a, b| min_x(&a.1).total_cmp(&min_x(&b.1)));
    let mut parent: Vec<usize> = (0..pieces.len()).collect();
    let mut first_at: Vec<(usize, Pt)> = Vec::new();
    for (k, &(i, a)) in lines.iter().enumerate() {
        for &(j, b) in &lines[k + 1..] {
            if min_x(&b) > max_x(&a) + ON_LINE_MM {
                break;
            }
            let ((ay0, ay1), (by0, by1)) = (y_range(&a), y_range(&b));
            let apart = by0 > ay1 + ON_LINE_MM || ay0 > by1 + ON_LINE_MM;
            if pieces[i].tag != pieces[j].tag || apart {
                continue;
            }
            if let Some(at) = overlap_at(a, b) {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri.max(rj)] = ri.min(rj);
                first_at.push((i.min(j), at));
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(i, _) in &lines {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out = Vec::new();
    for (_, mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort_unstable();
        let at = first_at
            .iter()
            .find(|(i, _)| members.contains(i))
            .map_or((0.0, 0.0), |(_, at)| *at);
        let span = span_of(pieces, &members);
        out.push(OverlapGroup { members, span, at });
    }
    out.sort_by_key(|g| g.members[0]);
    out
}

/// The two ends furthest apart along the first member's line.
fn span_of(pieces: &[ImportPiece], members: &[usize]) -> (Pt, Pt) {
    let axis = line_of(&pieces[members[0]]).unwrap_or_default();
    let ends = members
        .iter()
        .filter_map(|&i| line_of(&pieces[i]))
        .flat_map(|(a, b)| [a, b]);
    let mut lo = (f64::INFINITY, axis.0);
    let mut hi = (f64::NEG_INFINITY, axis.1);
    for end in ends {
        let t = along(axis, end);
        if t < lo.0 {
            lo = (t, end);
        }
        if t > hi.0 {
            hi = (t, end);
        }
    }
    (lo.1, hi.1)
}

/// `pieces` with each group of collinear overlapping lines of one tag
/// folded into one line, at the place of its first member, and how many
/// lines were folded away.
pub fn merge_overlaps(pieces: &[ImportPiece]) -> (Vec<ImportPiece>, usize) {
    let groups = overlap_groups(pieces);
    let mut folded = vec![false; pieces.len()];
    let mut spans = BTreeMap::new();
    for group in &groups {
        spans.insert(group.members[0], group.span);
        for &i in &group.members[1..] {
            folded[i] = true;
        }
    }
    let merged = pieces
        .iter()
        .enumerate()
        .filter(|(i, _)| !folded[*i])
        .map(|(i, piece)| match spans.get(&i) {
            Some(&(a, b)) => ImportPiece {
                shape: ImportShape::Line { a, b },
                tag: piece.tag,
            },
            None => *piece,
        })
        .collect();
    let count = groups.iter().map(|g| g.members.len() - 1).sum();
    (merged, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: Pt, b: Pt, tag: usize) -> ImportPiece {
        ImportPiece {
            shape: ImportShape::Line { a, b },
            tag,
        }
    }

    #[test]
    fn two_overlapping_lines_fold_into_their_span() {
        let pieces = [
            line((0.0, 0.0), (30.0, 0.0), 0),
            line((60.0, 0.0), (25.0, 0.0), 0),
        ];
        let groups = overlap_groups(&pieces);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members, [0, 1]);
        assert!((groups[0].at.0 - 27.5).abs() < 1e-9, "{:?}", groups[0].at);
        let (merged, folded) = merge_overlaps(&pieces);
        assert_eq!(folded, 1);
        assert_eq!(merged, [line((0.0, 0.0), (60.0, 0.0), 0)]);
    }

    #[test]
    fn lines_that_only_touch_or_are_parallel_do_not_overlap() {
        let pieces = [
            line((0.0, 0.0), (10.0, 0.0), 0),
            line((10.0, 0.0), (20.0, 0.0), 0),
            line((0.0, 1.0), (10.0, 1.0), 0),
        ];
        assert!(overlap_groups(&pieces).is_empty());
        assert_eq!(merge_overlaps(&pieces), (pieces.to_vec(), 0));
    }

    #[test]
    fn lines_of_two_tags_never_merge() {
        let pieces = [
            line((0.0, 0.0), (10.0, 0.0), 0),
            line((0.0, 0.0), (10.0, 0.0), 1),
        ];
        assert!(overlap_groups(&pieces).is_empty());
    }

    #[test]
    fn a_duplicate_vertical_line_folds_and_other_pieces_keep_their_order() {
        let circle = ImportPiece {
            shape: ImportShape::Circle {
                center: (5.0, 5.0),
                radius: 1.0,
            },
            tag: 0,
        };
        let pieces = [
            line((0.0, 0.0), (0.0, 10.0), 0),
            circle,
            line((0.0, 10.0), (0.0, 0.0), 0),
        ];
        let (merged, folded) = merge_overlaps(&pieces);
        assert_eq!(folded, 1);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1], circle);
    }
}
