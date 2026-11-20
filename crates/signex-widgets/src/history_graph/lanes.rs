//! Lane layout of the history graph: which lane each row's dot sits in and
//! which segments join a row to the next. Pure and deterministic.
//!
//! Rows run newest first. Each open lane waits for one commit (a parent of
//! a row above). A row takes the lowest lane waiting for it, or the lowest
//! free lane when none is; the other lanes waiting for it end there with a
//! fork curve in the span just above it. A row's first parent continues in
//! the row's own lane, so the first-parent chain of HEAD (or of the top row
//! when there is no HEAD) stays in lane 0. Each further parent of a merge
//! opens a lane with a merge curve in the span below the merge, or joins
//! the lane already waiting for that parent. Lanes are reused once freed.

use std::collections::HashMap;

use super::model::GraphCommit;

/// What is drawn at a row's dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotKind {
    Commit,
    /// A commit with more than one parent: a ring under the dot.
    Merge,
    /// The uncommitted-changes row above the commits.
    WorkingTree,
}

/// The shape of a segment between two adjacent rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// Stays in its lane.
    Straight,
    /// A branch lane bending into its parent's lane, in the span just above
    /// the parent.
    Fork,
    /// A merge's further parent leaving the merge's lane, in the span just
    /// below the merge.
    Merge,
}

/// How a segment is stroked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeStyle {
    Solid,
    /// The working tree's line down to HEAD.
    Dashed,
}

/// A segment from a lane of one row to a lane of the next row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub from_lane: usize,
    pub to_lane: usize,
    /// The lane whose colour the segment takes: the branch lane, so a fork
    /// and a merge curve match the branch they belong to.
    pub color_lane: usize,
    pub kind: EdgeKind,
    pub style: EdgeStyle,
}

/// One row of the layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowLayout {
    pub lane: usize,
    pub dot: DotKind,
    /// Segments from this row to the next; empty on the last row.
    pub edges: Vec<Edge>,
}

/// The whole layout: one entry per row (the working tree first when there
/// is one) and how many lanes are drawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphLayout {
    pub rows: Vec<RowLayout>,
    pub lane_count: usize,
}

/// An open lane: the row it waits for, and where its next segment starts
/// when it was opened by a merge in the row above.
#[derive(Debug, Clone, Copy)]
struct Track {
    target: usize,
    color_lane: usize,
    style: EdgeStyle,
    /// Holds lane 0 for HEAD before HEAD's row; draws nothing.
    is_reserved: bool,
    merged_from: Option<usize>,
}

/// One row's dot and parents, as row indices below it.
struct Node {
    dot: DotKind,
    style: EdgeStyle,
    first_parent: Option<usize>,
    other_parents: Vec<usize>,
}

/// Lay out `commits` (newest first), with the working-tree row on top when
/// `has_working_tree`. A parent that is not in the list is dropped, so a
/// filtered or truncated history draws no lane for it.
pub fn layout(commits: &[GraphCommit], has_working_tree: bool) -> GraphLayout {
    let nodes = nodes(commits, has_working_tree);
    let head_row = commits
        .iter()
        .position(GraphCommit::is_head)
        .map(|i| i + usize::from(has_working_tree));

    let mut lanes: Vec<Option<Track>> = Vec::new();
    if !has_working_tree
        && let Some(head) = head_row
        && head > 0
    {
        lanes.push(Some(Track {
            target: head,
            color_lane: 0,
            style: EdgeStyle::Solid,
            is_reserved: true,
            merged_from: None,
        }));
    }

    let mut out = GraphLayout::default();
    for (row, node) in nodes.iter().enumerate() {
        let lane = place(&mut lanes, row);
        let joins = open_parents(&mut lanes, lane, node);
        let edges = if row + 1 < nodes.len() {
            span_edges(&mut lanes, lane, &joins, row + 1)
        } else {
            Vec::new()
        };
        out.lane_count = edges
            .iter()
            .map(|e| e.from_lane.max(e.to_lane) + 1)
            .fold(out.lane_count.max(lane + 1), usize::max);
        out.rows.push(RowLayout {
            lane,
            dot: node.dot,
            edges,
        });
    }
    out
}

/// Rows as nodes with parents resolved to row indices below them.
fn nodes(commits: &[GraphCommit], has_working_tree: bool) -> Vec<Node> {
    let offset = usize::from(has_working_tree);
    let mut row_of: HashMap<&str, usize> = HashMap::new();
    for (i, c) in commits.iter().enumerate() {
        row_of.entry(c.sha.as_str()).or_insert(i + offset);
    }
    let mut nodes = Vec::with_capacity(commits.len() + offset);
    if has_working_tree {
        let head = commits.iter().position(GraphCommit::is_head);
        nodes.push(Node {
            dot: DotKind::WorkingTree,
            style: EdgeStyle::Dashed,
            first_parent: head.map(|i| i + offset),
            other_parents: Vec::new(),
        });
    }
    for (i, c) in commits.iter().enumerate() {
        let row = i + offset;
        // Only rows below resolve, so a malformed cycle cannot loop.
        let below = |sha: &String| row_of.get(sha.as_str()).copied().filter(|&p| p > row);
        let first_parent = c.parents.first().and_then(below);
        let mut other_parents: Vec<usize> = Vec::new();
        for p in c.parents.iter().skip(1).filter_map(below) {
            if Some(p) != first_parent && !other_parents.contains(&p) {
                other_parents.push(p);
            }
        }
        nodes.push(Node {
            dot: if c.is_merge() {
                DotKind::Merge
            } else {
                DotKind::Commit
            },
            style: EdgeStyle::Solid,
            first_parent,
            other_parents,
        });
    }
    nodes
}

/// The lowest lane waiting for `row`, or the lowest free lane; every lane
/// waiting for `row` is closed.
fn place(lanes: &mut Vec<Option<Track>>, row: usize) -> usize {
    let lane = lane_for(lanes, row);
    for slot in lanes.iter_mut() {
        if slot.is_some_and(|t| t.target == row) {
            *slot = None;
        }
    }
    if lane == lanes.len() {
        lanes.push(None);
    }
    lane
}

/// Where `row` will sit given the open lanes, without changing them.
fn lane_for(lanes: &[Option<Track>], row: usize) -> usize {
    lanes
        .iter()
        .position(|slot| slot.is_some_and(|t| t.target == row))
        .unwrap_or_else(|| free_lane(lanes))
}

fn free_lane(lanes: &[Option<Track>]) -> usize {
    lanes
        .iter()
        .position(Option::is_none)
        .unwrap_or(lanes.len())
}

/// Opens the lanes for `node`'s parents; returns the lanes a merge joins
/// that were already open (they get an extra merge segment).
fn open_parents(lanes: &mut Vec<Option<Track>>, lane: usize, node: &Node) -> Vec<usize> {
    if let Some(target) = node.first_parent {
        lanes[lane] = Some(Track {
            target,
            color_lane: lane,
            style: node.style,
            is_reserved: false,
            merged_from: None,
        });
    }
    let mut joins = Vec::new();
    for &target in &node.other_parents {
        let open = lanes
            .iter()
            .position(|slot| slot.is_some_and(|t| t.target == target));
        match open {
            Some(k) => match lanes[k].as_mut() {
                // The HEAD reservation becomes the merge's lane.
                Some(t) if t.is_reserved => {
                    t.is_reserved = false;
                    t.merged_from = Some(lane);
                }
                _ => joins.push(k),
            },
            None => {
                let k = free_lane(lanes);
                if k == lanes.len() {
                    lanes.push(None);
                }
                lanes[k] = Some(Track {
                    target,
                    color_lane: k,
                    style: node.style,
                    is_reserved: false,
                    merged_from: Some(lane),
                });
            }
        }
    }
    joins
}

/// The segments from the current row (dot in `lane`) to row `next`.
fn span_edges(lanes: &mut [Option<Track>], lane: usize, joins: &[usize], next: usize) -> Vec<Edge> {
    let next_lane = lane_for(lanes, next);
    let end_of = |k: usize, t: &Track| if t.target == next { next_lane } else { k };
    let mut edges = Vec::new();
    for (k, slot) in lanes.iter_mut().enumerate() {
        let Some(t) = slot.as_mut() else { continue };
        if t.is_reserved {
            continue;
        }
        let from_lane = t.merged_from.take().unwrap_or(k);
        let to_lane = end_of(k, &*t);
        let kind = if from_lane != k {
            EdgeKind::Merge
        } else if to_lane != k {
            EdgeKind::Fork
        } else {
            EdgeKind::Straight
        };
        edges.push(Edge {
            from_lane,
            to_lane,
            color_lane: t.color_lane,
            kind,
            style: t.style,
        });
    }
    for &k in joins {
        if let Some(t) = lanes[k] {
            edges.push(Edge {
                from_lane: lane,
                to_lane: end_of(k, &t),
                color_lane: t.color_lane,
                kind: EdgeKind::Merge,
                style: t.style,
            });
        }
    }
    edges
}
