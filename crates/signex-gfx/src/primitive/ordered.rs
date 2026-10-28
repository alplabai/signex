//! Paint-ordered drawing items and the run plan that draws them in order (#1579).
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.
//!
//! Design and rejected alternatives: the "Design" comment on #1579.

use crate::pipeline::polygon::{PolygonVertex, triangulate_polygons};
use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::polygon::GpuPolygon;

/// One drawing, in file paint order. The variants are the existing primitive
/// types, so each kind keeps its own pipeline and shader.
#[derive(Clone, Debug, PartialEq)]
pub enum OrderedItem {
    Line(LineSegment),
    Circle(Circle),
    Arc(Arc),
    Polygon(GpuPolygon),
}

/// Which pipeline draws a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawKind {
    Line,
    Circle,
    Arc,
    Polygon,
}

/// A maximal stretch of consecutive same-kind items. `first` and `count` index
/// that kind's own buffer: instances for line, circle and arc, vertices for
/// polygon (an item may tessellate to many).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawRun {
    pub kind: DrawKind,
    pub first: u32,
    pub count: u32,
}

/// The per-kind buffers' contents and the runs that interleave them.
#[derive(Default)]
pub(crate) struct OrderedBatches {
    pub(crate) lines: Vec<LineSegment>,
    pub(crate) circles: Vec<Circle>,
    pub(crate) arcs: Vec<Arc>,
    pub(crate) polygon_vertices: Vec<PolygonVertex>,
    pub(crate) runs: Vec<DrawRun>,
}

impl OrderedBatches {
    /// Fill the per-kind buffers in list order and plan the runs over them.
    pub(crate) fn build(items: &[OrderedItem]) -> Self {
        let mut batches = Self::default();
        for item in items {
            let (kind, added) = batches.push(item);
            batches.extend_runs(kind, added);
        }
        batches
    }

    /// Append `item` to its kind's buffer; returns the kind and how many
    /// buffer elements it added (0 for a polygon that tessellates to nothing).
    fn push(&mut self, item: &OrderedItem) -> (DrawKind, u32) {
        match item {
            OrderedItem::Line(line) => {
                self.lines.push(*line);
                (DrawKind::Line, 1)
            }
            OrderedItem::Circle(circle) => {
                self.circles.push(*circle);
                (DrawKind::Circle, 1)
            }
            OrderedItem::Arc(arc) => {
                self.arcs.push(*arc);
                (DrawKind::Arc, 1)
            }
            OrderedItem::Polygon(polygon) => {
                let before = self.polygon_vertices.len();
                self.polygon_vertices
                    .extend(triangulate_polygons(std::slice::from_ref(polygon)));
                (
                    DrawKind::Polygon,
                    (self.polygon_vertices.len() - before) as u32,
                )
            }
        }
    }

    /// Account for `added` just-pushed elements of `kind`: grow the open run
    /// when it is the same kind, else start a new one. Nothing added, nothing
    /// drawn, so an empty item neither splits nor starts a run.
    fn extend_runs(&mut self, kind: DrawKind, added: u32) {
        if added == 0 {
            return;
        }
        match self.runs.last_mut() {
            Some(open) if open.kind == kind => open.count += added,
            _ => {
                let first = self.len_of(kind) - added;
                self.runs.push(DrawRun {
                    kind,
                    first,
                    count: added,
                });
            }
        }
    }

    fn len_of(&self, kind: DrawKind) -> u32 {
        (match kind {
            DrawKind::Line => self.lines.len(),
            DrawKind::Circle => self.circles.len(),
            DrawKind::Arc => self.arcs.len(),
            DrawKind::Polygon => self.polygon_vertices.len(),
        }) as u32
    }
}

/// Split `items` into draw runs, starting a new run wherever the kind changes.
pub fn plan_runs(items: &[OrderedItem]) -> Vec<DrawRun> {
    OrderedBatches::build(items).runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> OrderedItem {
        OrderedItem::Line(LineSegment {
            p0: [0.0, 0.0],
            p1: [1.0, 0.0],
            width: 0.1,
            color: [1.0; 4],
            style: 0,
            _pad: 0,
        })
    }

    fn circle(stroke_width: f32) -> OrderedItem {
        OrderedItem::Circle(Circle {
            center: [0.0, 0.0],
            radius: 1.0,
            stroke_width,
            color: [1.0; 4],
        })
    }

    fn arc() -> OrderedItem {
        OrderedItem::Arc(Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: 1.0,
            width: 0.1,
            color: [1.0; 4],
            _pad: [0.0; 3],
        })
    }

    fn triangle() -> OrderedItem {
        OrderedItem::Polygon(GpuPolygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            fill_color: [1.0; 4],
            ..GpuPolygon::default()
        })
    }

    fn run(kind: DrawKind, first: u32, count: u32) -> DrawRun {
        DrawRun { kind, first, count }
    }

    #[test]
    fn an_empty_list_has_no_runs() {
        assert!(plan_runs(&[]).is_empty());
    }

    #[test]
    fn same_kind_items_share_one_run() {
        assert_eq!(
            plan_runs(&[line(), line(), line()]),
            [run(DrawKind::Line, 0, 3)]
        );
    }

    #[test]
    fn a_fill_and_stroke_circle_pair_is_two_instances_in_one_run() {
        assert_eq!(
            plan_runs(&[circle(0.0), circle(0.2)]),
            [run(DrawKind::Circle, 0, 2)]
        );
    }

    #[test]
    fn alternating_kinds_make_one_run_per_item() {
        let items = [line(), circle(0.0), arc(), line(), circle(0.2)];
        let runs = plan_runs(&items);
        assert_eq!(runs.len(), items.len());
        assert_eq!(
            runs,
            [
                run(DrawKind::Line, 0, 1),
                run(DrawKind::Circle, 0, 1),
                run(DrawKind::Arc, 0, 1),
                run(DrawKind::Line, 1, 1),
                run(DrawKind::Circle, 1, 1),
            ]
        );
    }

    #[test]
    fn polygon_runs_carry_contiguous_vertex_ranges() {
        // A triangle is one fill triangle: 3 vertices. Two of them, then a
        // circle, then one more.
        let runs = plan_runs(&[triangle(), triangle(), circle(0.0), triangle()]);
        assert_eq!(
            runs,
            [
                run(DrawKind::Polygon, 0, 6),
                run(DrawKind::Circle, 0, 1),
                run(DrawKind::Polygon, 6, 3),
            ]
        );
    }

    #[test]
    fn a_degenerate_polygon_draws_nothing_and_does_not_split_a_run() {
        let degenerate = OrderedItem::Polygon(GpuPolygon::default());
        assert_eq!(
            plan_runs(&[circle(0.0), degenerate, circle(0.2)]),
            [run(DrawKind::Circle, 0, 2)]
        );
    }
}
