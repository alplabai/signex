//! The board body: the outline extruded to the board thickness, less its
//! holes and cut-outs.

use solidcore::boolean::api_brep::boolean_difference;
use solidcore::curve::curve::Curve;
use solidcore::primitive::api_curves::create_line_segment;
use solidcore::primitive::api_primitives::create_cylinder;
use solidcore::sweep::api_brep::create_linear_sweep;
use solidcore::topology::Brep;
use solidcore::{Kernel, Point3, Vec3};

use crate::error::{McadError, contained, kernel_call};
use crate::naming::{FaceName, NameTable};
use crate::solid::Solid;
use crate::topology::{complete_trim_curves, face_vertex_points};

/// A hole through the board, in the outline's frame (mm).
#[derive(Debug, Clone, PartialEq)]
pub enum Hole {
    /// A round drilled hole.
    Round { center: [f64; 2], diameter: f64 },
    /// A cut-out bounded by a closed polygon.
    Polygon(Vec<[f64; 2]>),
}

/// Two points closer than this (mm) are the same point.
const SAME_POINT_MM: f64 = 1e-6;

/// The kernel every operation of this crate starts from: default tolerances
/// with the opt-in fix for overlapping cylindrical cuts, so a hole that
/// overlaps another hole cuts instead of failing.
pub(crate) fn new_kernel() -> Kernel {
    let mut kernel = Kernel::default();
    kernel.fixes.cone_parallel_line_midpoint = true;
    kernel.fixes.cone_parallel_line_domains = true;
    kernel
}

/// Builds the board body: `outline` (a closed polygon, mm, the closing point
/// may be repeated or left out) extruded from z = 0 to `thickness_mm`, with
/// every hole cut through.
///
/// Faces are named [`FaceName::Top`], [`FaceName::Bottom`],
/// [`FaceName::OutlineSide`] per outline segment, and
/// [`FaceName::HoleWall`] / [`FaceName::CutoutWall`] per hole.
///
/// # Errors
///
/// [`McadError::InvalidInput`] for a thickness that is not finite and
/// positive, an outline or cut-out with fewer than three distinct points, no
/// area or a non-finite coordinate, or a hole diameter that is not finite and
/// positive; [`McadError::Kernel`] / [`McadError::KernelPanicked`] when the
/// kernel fails.
pub fn board_body(
    outline: &[[f64; 2]],
    holes: &[Hole],
    thickness_mm: f64,
) -> Result<Solid, McadError> {
    contained("board body", || build(outline, holes, thickness_mm))
}

fn build(outline: &[[f64; 2]], holes: &[Hole], thickness: f64) -> Result<Solid, McadError> {
    const OP: &str = "board body";
    if !thickness.is_finite() || thickness <= 0.0 {
        return Err(McadError::invalid(
            OP,
            "thickness must be finite and positive",
        ));
    }
    let ring = profile_ring(OP, outline)?;
    let mut kernel = new_kernel();
    let mut names = NameTable::default();
    let mut body = prism(&mut kernel, &ring, 0.0, thickness)?;
    name_board_faces(&mut body, &mut names, &ring, thickness)?;

    // Tools reach past both faces so no sliver of a face survives the cut.
    let margin = thickness.max(1.0);
    for (index, hole) in holes.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| McadError::invalid(OP, "too many holes"))?;
        let (mut tool, name) = match hole {
            Hole::Round { center, diameter } => {
                let radius = round_hole_radius(*center, *diameter)?;
                let origin = Point3::new(center[0], center[1], -margin);
                let tool = kernel_call("hole tool", || {
                    create_cylinder(&mut kernel, origin, radius, thickness + 2.0 * margin)
                })?;
                (tool, FaceName::HoleWall(index))
            }
            Hole::Polygon(points) => {
                let ring = profile_ring("cut-out", points)?;
                let tool = prism(&mut kernel, &ring, -margin, thickness + 2.0 * margin)?;
                (tool, FaceName::CutoutWall(index))
            }
        };
        for f in tool.face_list().to_vec() {
            names.tag(&mut tool, f, name)?;
        }
        let blank = body;
        body = kernel_call("cut hole", || boolean_difference(&mut kernel, blank, tool))?;
    }
    complete_trim_curves(&kernel, &mut body)?;
    Ok(Solid { brep: body, names })
}

fn round_hole_radius(center: [f64; 2], diameter: f64) -> Result<f64, McadError> {
    if !center.iter().all(|c| c.is_finite()) {
        return Err(McadError::invalid("hole", "centre must be finite"));
    }
    if !diameter.is_finite() || diameter <= 0.0 {
        return Err(McadError::invalid(
            "hole",
            "diameter must be finite and positive",
        ));
    }
    Ok(diameter / 2.0)
}

/// The polygon as a counter-clockwise ring without repeated points.
fn profile_ring(operation: &'static str, points: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, McadError> {
    if !points.iter().flatten().all(|c| c.is_finite()) {
        return Err(McadError::invalid(
            operation,
            "coordinates must be finite",
        ));
    }
    let mut ring: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for &p in points {
        if ring.last().is_none_or(|&q| !same_xy(p, q)) {
            ring.push(p);
        }
    }
    while ring.len() > 1 && ring.first().zip(ring.last()).is_some_and(|(&a, &b)| same_xy(a, b)) {
        ring.pop();
    }
    if ring.len() < 3 {
        return Err(McadError::invalid(
            operation,
            "a profile needs at least three distinct points",
        ));
    }
    let area = signed_area(&ring);
    if area.abs() <= SAME_POINT_MM * SAME_POINT_MM {
        return Err(McadError::invalid(operation, "the profile has no area"));
    }
    if area < 0.0 {
        ring.reverse();
    }
    Ok(ring)
}

fn same_xy(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() <= SAME_POINT_MM && (a[1] - b[1]).abs() <= SAME_POINT_MM
}

/// Shoelace area, positive for a counter-clockwise ring.
fn signed_area(ring: &[[f64; 2]]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        / 2.0
}

/// The ring at height `z0` extruded by `height` along +z, capped.
fn prism(kernel: &mut Kernel, ring: &[[f64; 2]], z0: f64, height: f64) -> Result<Brep, McadError> {
    let curves = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| {
            let start = Point3::new(a[0], a[1], z0);
            let end = Point3::new(b[0], b[1], z0);
            kernel_call("profile segment", || {
                create_line_segment(start, end).map(Curve::from)
            })
        })
        .collect::<Result<Vec<Curve>, McadError>>()?;
    let profile: Vec<Option<&Curve>> = curves.iter().map(Some).collect();
    kernel_call("extrude profile", || {
        create_linear_sweep(kernel, &profile, Vec3::new(0.0, 0.0, 1.0), height, true)
    })
}

/// Names the faces of the freshly extruded board from where they lie: the
/// cap at z = 0, the cap at the thickness, and per side the outline segment
/// whose two end points it holds.
fn name_board_faces(
    body: &mut Brep,
    names: &mut NameTable,
    ring: &[[f64; 2]],
    thickness: f64,
) -> Result<(), McadError> {
    for f in body.face_list().to_vec() {
        let corners = face_vertex_points(body, f)?;
        let at = |z: f64| corners.iter().all(|p| (p[2] - z).abs() <= SAME_POINT_MM);
        let name = if at(0.0) {
            Some(FaceName::Bottom)
        } else if at(thickness) {
            Some(FaceName::Top)
        } else {
            side_segment(ring, &corners).map(FaceName::OutlineSide)
        };
        if let Some(name) = name {
            names.tag(body, f, name)?;
        }
    }
    Ok(())
}

fn side_segment(ring: &[[f64; 2]], corners: &[[f64; 3]]) -> Option<u32> {
    let holds = |p: [f64; 2]| corners.iter().any(|c| same_xy([c[0], c[1]], p));
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .position(|(&a, &b)| holds(a) && holds(b))
        .and_then(|index| u32::try_from(index).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clockwise_outline_with_a_closing_point_becomes_a_ccw_ring() {
        let ring = profile_ring(
            "t",
            &[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        )
        .expect("ring");
        assert_eq!(ring.len(), 4);
        assert!(signed_area(&ring) > 0.0);
    }

    #[test]
    fn a_degenerate_outline_is_refused() {
        assert!(profile_ring("t", &[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]).is_err());
        assert!(profile_ring("t", &[[0.0, 0.0], [1.0, 0.0]]).is_err());
        assert!(profile_ring("t", &[[0.0, 0.0], [f64::NAN, 0.0], [1.0, 1.0]]).is_err());
    }
}
