//! An assembly drawing (#2486) on a printed page: the marks of
//! [`crate::assembly`] through the page frame, in the drawing's inks.

use super::super::surface::PdfSurface;
use super::PageFrame;
use super::draw::{Pt, fill, label, stroke};
use crate::assembly::{AssemblyDrawing, AssemblyMark};

/// Draw `drawing` on the page `frame` describes.
pub(super) fn page(
    s: &mut PdfSurface<'_>,
    drawing: &AssemblyDrawing,
    frame: &PageFrame,
    monochrome: bool,
) {
    // Round caps and joins, so a bent line has no notch.
    s.raw_operator("1 J 1 j\n");
    let ring = |points: &[signex_types::pcb::Point]| -> Vec<Pt> {
        points.iter().map(|p| frame.to_pt(p.x, p.y)).collect()
    };
    for mark in &drawing.marks {
        match mark {
            AssemblyMark::Stroke {
                points,
                closed,
                width_mm,
                ink,
            } => {
                let width = frame.length_pt(width_mm.max(0.0));
                stroke(s, &ring(points), *closed, width, ink.rgb(monochrome));
            }
            AssemblyMark::Fill { rings, ink } => {
                let rings: Vec<Vec<Pt>> = rings.iter().map(|r| ring(r)).collect();
                fill(s, &rings, ink.rgb(monochrome));
            }
            AssemblyMark::Text {
                at,
                rotation_deg,
                size_mm,
                text,
                ink,
            } => label(
                s,
                frame,
                (at.x, at.y),
                *rotation_deg,
                text,
                *size_mm,
                ink.rgb(monochrome),
            ),
        }
    }
}
