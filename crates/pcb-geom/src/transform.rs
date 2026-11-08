//! The pad world transform: a footprint-local point placed on the board.
//!
//! Order: mirror (bottom side only), then rotate, then translate. A
//! bottom-side footprint mirrors its local x axis, as seen from the top.
//! Pad coordinates stay library-local in the model; the mirror lives here
//! only (ADR-0005 §7).
//! Rotation is counter-clockwise in the model's axes, the sense the PCB
//! renderer turns pads (`signex-renderer` `pcb/emit.rs`). A board pad has
//! no angle of its own: it turns with its footprint.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadShape};
use signex_types::pcb_layer::BoardSide;

use crate::shape::Shape;
use crate::units::{GeomError, PointNm, mm_to_nm};

/// Which face of the board a footprint sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
}

impl Side {
    /// The side a footprint's native layer token (`TopCopper`,
    /// `BottomCopper`, ...) puts it on. A token this build does not know,
    /// or a layer with no side, is the top.
    pub fn of_layer(layer: &str) -> Side {
        match parse_pcb_layer(layer).and_then(|l| l.side()) {
            Some(BoardSide::Bottom) => Side::Bottom,
            _ => Side::Top,
        }
    }
}

/// Where a footprint frame sits on the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub origin: PointNm,
    pub rotation_deg: f64,
    pub side: Side,
}

impl Placement {
    /// The placement of `footprint`, refusing a non-finite position or angle.
    pub fn of_footprint(footprint: &Footprint) -> Result<Self, GeomError> {
        if !footprint.rotation.is_finite() {
            return Err(GeomError::Unrepresentable("footprint.rotation"));
        }
        Ok(Placement {
            origin: PointNm::from_mm(footprint.position, "footprint.position")?,
            rotation_deg: footprint.rotation,
            side: Side::of_layer(&footprint.layer),
        })
    }

    /// A footprint-local point on the board.
    pub fn to_world(&self, local: PointNm) -> PointNm {
        let mirrored = match self.side {
            Side::Top => local,
            Side::Bottom => PointNm::new(-local.x, local.y),
        };
        let (sin, cos) = sin_cos_deg(self.rotation_deg);
        rotate(mirrored, sin, cos).plus(self.origin)
    }

    /// A board point in the footprint's frame; the inverse of [`to_world`](Self::to_world).
    pub fn to_local(&self, world: PointNm) -> PointNm {
        let (sin, cos) = sin_cos_deg(self.rotation_deg);
        let unrotated = rotate(world.minus(self.origin), -sin, cos);
        match self.side {
            Side::Top => unrotated,
            Side::Bottom => PointNm::new(-unrotated.x, unrotated.y),
        }
    }
}

/// The board position of `pad`'s centre.
pub fn pad_world_center(footprint: &Footprint, pad: &Pad) -> Result<PointNm, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    Ok(placement.to_world(PointNm::from_mm(pad.position, "pad.position")?))
}

/// `pad`'s copper outline on the board. Rect-like shapes without their own
/// parameters on a board pad (round-rect, trapezoid, custom) are their
/// bounding rectangle, as the renderer draws them (`pad_outline.rs`).
pub fn pad_shape(footprint: &Footprint, pad: &Pad) -> Result<Shape, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    let center = placement.to_world(PointNm::from_mm(pad.position, "pad.position")?);
    let half_w = (mm_to_nm(pad.size.x, "pad.size")? / 2).abs();
    let half_h = (mm_to_nm(pad.size.y, "pad.size")? / 2).abs();
    // Rect and ellipse outlines are symmetric about both axes, so the x
    // mirror of a bottom-side footprint moves the centre but not the angle.
    let rotation_deg = placement.rotation_deg;
    Ok(match pad.shape {
        PadShape::Circle => Shape::Disc {
            center,
            radius: half_w.min(half_h),
        },
        PadShape::Oval if half_w == half_h => Shape::Disc {
            center,
            radius: half_w,
        },
        PadShape::Oval => Shape::Ellipse {
            center,
            half_w,
            half_h,
            rotation_deg,
        },
        PadShape::Rect | PadShape::RoundRect | PadShape::Trapezoid | PadShape::Custom => {
            Shape::Rect {
                center,
                half_w,
                half_h,
                rotation_deg,
            }
        }
    })
}

/// `sin` and `cos` of an angle in degrees, exact on the quarter turns so a
/// 90° footprint lands on whole nanometres.
pub(crate) fn sin_cos_deg(deg: f64) -> (f64, f64) {
    let turn = deg.rem_euclid(360.0);
    if turn == 0.0 || turn == 360.0 {
        (0.0, 1.0)
    } else if turn == 90.0 {
        (1.0, 0.0)
    } else if turn == 180.0 {
        (0.0, -1.0)
    } else if turn == 270.0 {
        (-1.0, 0.0)
    } else {
        turn.to_radians().sin_cos()
    }
}

/// `v` turned counter-clockwise, rounded to the nearest nm.
pub(crate) fn rotate(v: PointNm, sin: f64, cos: f64) -> PointNm {
    let (x, y) = (v.x as f64, v.y as f64);
    PointNm::new(
        (x * cos - y * sin).round() as i64,
        (x * sin + y * cos).round() as i64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::{PadType, Point};
    use uuid::Uuid;

    fn footprint(x: f64, y: f64, rotation: f64, layer: &str) -> Footprint {
        Footprint {
            uuid: Uuid::nil(),
            reference: "R1".into(),
            value: String::new(),
            footprint_id: String::new(),
            position: Point::new(x, y),
            rotation,
            layer: layer.into(),
            locked: false,
            pads: Vec::new(),
            graphics: Vec::new(),
            properties: Vec::new(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    fn pad(x: f64, y: f64, w: f64, h: f64, shape: PadShape) -> Pad {
        Pad {
            uuid: Uuid::nil(),
            number: "1".into(),
            pad_type: PadType::Smd,
            shape,
            position: Point::new(x, y),
            size: Point::new(w, h),
            drill: None,
            layers: Vec::new(),
            net: None,
            roundrect_ratio: 0.0,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    fn mm(x: f64, y: f64) -> PointNm {
        PointNm::from_mm(Point::new(x, y), "test").unwrap()
    }

    #[test]
    fn top_side_rotates_counter_clockwise_then_translates() {
        let fp = footprint(10.0, 20.0, 90.0, "TopCopper");
        let p = pad(1.0, 0.0, 1.0, 1.0, PadShape::Rect);
        assert_eq!(pad_world_center(&fp, &p), Ok(mm(10.0, 21.0)));
        let fp = footprint(10.0, 20.0, 180.0, "TopCopper");
        assert_eq!(pad_world_center(&fp, &p), Ok(mm(9.0, 20.0)));
    }

    #[test]
    fn bottom_side_mirrors_x_before_rotating() {
        let p = pad(1.0, 2.0, 1.0, 1.0, PadShape::Rect);
        let fp = footprint(0.0, 0.0, 0.0, "BottomCopper");
        assert_eq!(pad_world_center(&fp, &p), Ok(mm(-1.0, 2.0)));
        // Mirror (−1, 2), then +90°: (−2, −1).
        let fp = footprint(0.0, 0.0, 90.0, "BottomCopper");
        assert_eq!(pad_world_center(&fp, &p), Ok(mm(-2.0, -1.0)));
    }

    #[test]
    fn quarter_turns_are_exact_and_negative_angles_wrap() {
        assert_eq!(sin_cos_deg(-90.0), (-1.0, 0.0));
        assert_eq!(sin_cos_deg(450.0), (1.0, 0.0));
        let fp = footprint(0.0, 0.0, -270.0, "TopCopper");
        let p = pad(1.234_567, 0.0, 1.0, 1.0, PadShape::Rect);
        assert_eq!(pad_world_center(&fp, &p), Ok(mm(0.0, 1.234_567)));
    }

    #[test]
    fn to_local_inverts_to_world() {
        for side in [Side::Top, Side::Bottom] {
            let placement = Placement {
                origin: mm(3.0, -4.0),
                rotation_deg: 37.5,
                side,
            };
            let local = mm(1.5, 0.75);
            let back = placement.to_local(placement.to_world(local));
            assert!((back.x - local.x).abs() <= 1 && (back.y - local.y).abs() <= 1);
        }
    }

    #[test]
    fn non_finite_rotation_or_position_is_refused() {
        let p = pad(0.0, 0.0, 1.0, 1.0, PadShape::Rect);
        let fp = footprint(0.0, 0.0, f64::NAN, "TopCopper");
        assert_eq!(
            pad_world_center(&fp, &p),
            Err(GeomError::Unrepresentable("footprint.rotation"))
        );
        let fp = footprint(f64::INFINITY, 0.0, 0.0, "TopCopper");
        assert_eq!(
            pad_shape(&fp, &p),
            Err(GeomError::Unrepresentable("footprint.position"))
        );
        let fp = footprint(0.0, 0.0, 0.0, "TopCopper");
        let bad = pad(0.0, 0.0, f64::NAN, 1.0, PadShape::Rect);
        assert_eq!(
            pad_shape(&fp, &bad),
            Err(GeomError::Unrepresentable("pad.size"))
        );
    }

    #[test]
    fn pad_shape_follows_the_pad_kind() {
        let fp = footprint(0.0, 0.0, 30.0, "TopCopper");
        let circle = pad_shape(&fp, &pad(0.0, 0.0, 2.0, 1.0, PadShape::Circle)).unwrap();
        assert_eq!(
            circle,
            Shape::Disc {
                center: PointNm::ORIGIN,
                radius: 500_000
            }
        );
        let round_oval = pad_shape(&fp, &pad(0.0, 0.0, 1.0, 1.0, PadShape::Oval)).unwrap();
        assert!(matches!(
            round_oval,
            Shape::Disc {
                radius: 500_000,
                ..
            }
        ));
        let oval = pad_shape(&fp, &pad(0.0, 0.0, 2.0, 1.0, PadShape::Oval)).unwrap();
        assert!(matches!(
            oval,
            Shape::Ellipse {
                half_w: 1_000_000,
                half_h: 500_000,
                ..
            }
        ));
        let rr = pad_shape(&fp, &pad(0.0, 0.0, 2.0, 1.0, PadShape::RoundRect)).unwrap();
        assert!(matches!(rr, Shape::Rect { rotation_deg, .. } if rotation_deg == 30.0));
    }

    #[test]
    fn side_reads_the_native_layer_token() {
        assert_eq!(Side::of_layer("BottomCopper"), Side::Bottom);
        assert_eq!(Side::of_layer("BottomSilkscreen"), Side::Bottom);
        assert_eq!(Side::of_layer("TopCopper"), Side::Top);
        assert_eq!(Side::of_layer("Inner1Copper"), Side::Top);
        assert_eq!(Side::of_layer(""), Side::Top);
        assert_eq!(Side::of_layer("not a layer"), Side::Top);
    }

    #[test]
    fn tiny_negative_angle_takes_the_zero_turn_path() {
        assert_eq!(sin_cos_deg(-1e-20), (0.0, 1.0));
    }
}
