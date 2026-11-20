//! The Arc illustration (m98, `R.arc`): the arc as a band with its Radius
//! and Width callouts, the Start and End rays and the centre; no rotation
//! arrow (review.md, 2026-11-18). The arc runs counter-clockwise from the
//! start angle to the end angle, read with Y up as the fields are.

use iced::Point;
use signex_i18n::{I18n, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_widgets::property_illustration::{Anchor, FieldId, Illustration, TextRole};

use super::{at, band, callout, canvas, cross, dashed, keys, label, layer_label, layer_note};
use crate::panels::pcb_primitive_properties::{ArcFacts, AreaLayerChoice};

/// The shape drawn for a value the selection does not share, as the
/// mockup's stand-in arc: radius 2, width 0.5, 0° to 90°.
const STAND_IN: (f64, f64, f64, f64) = (2.0, 0.5, 0.0, 90.0);
/// The box the arc is fitted into, and the largest scale (view units per mm).
const FIT_BOX: (f32, f32, f32, f32) = (40.0, 12.0, 224.0, 124.0);
const MAX_SCALE: f32 = 80.0;
/// Room left round the arc for the rays' labels, view units.
const LABEL_ROOM: f32 = 24.0;

pub(in crate::panels::pcb_primitive_properties) fn illustration(
    i18n: &I18n,
    f: &ArcFacts,
) -> Illustration {
    let radius = f.radius.unwrap_or(STAND_IN.0).max(0.01);
    let width = f.width.unwrap_or(STAND_IN.1).max(0.01);
    let start = f.start_angle.unwrap_or(STAND_IN.2);
    let end = f.end_angle.unwrap_or(STAND_IN.3);
    let mut sweep = (end - start).rem_euclid(360.0);
    if sweep == 0.0 && end != start {
        sweep = 360.0;
    }
    let shape = Shape::fit(radius as f32, width as f32, start as f32, sweep as f32);
    let ring = shape.r + shape.ws / 2.0;
    let ray = |angle: f32, name: signex_i18n::MessageId, key: &FieldId| {
        [
            dashed(
                shape.centre,
                shape.point(angle, ring + 8.0),
                std::slice::from_ref(key),
            ),
            label(
                shape.point(angle, ring + 17.0),
                t!(i18n, name),
                Anchor::Middle,
                TextRole::Caption,
                std::slice::from_ref(key),
            ),
        ]
    };
    let layer = f
        .layer
        .as_ref()
        .map_or(PcbLayer::Copper(CopperLayer::Top), |l| l.layer);
    let mid = shape.start + shape.sweep / 2.0;
    let quarter = shape.start + shape.sweep / 4.0;
    let (sin_mid, cos_mid) = mid.to_radians().sin_cos();
    let radius_at = shape.point(mid, shape.r * 0.5);
    let drawing = canvas()
        .parts(ray(shape.start, ids::PCB_PRIM_POINT_START, &keys::ANGLES))
        .parts(ray(
            shape.start + shape.sweep,
            ids::PCB_PRIM_POINT_END,
            &keys::ANGLES,
        ))
        .part(band(
            shape.points(),
            shape.ws,
            super::layer_paint(layer),
            &[
                keys::RADIUS,
                keys::WIDTH,
                keys::ANGLES,
                keys::LAYER,
                keys::ENDS,
            ],
        ))
        .parts(cross(shape.centre, 4.0, &[keys::CENTRE]))
        .part(callout(
            shape.centre,
            shape.point(mid, shape.r),
            Some((
                t!(i18n, ids::PCB_PRIM_RADIUS).into(),
                at(radius_at.x + 6.0 * sin_mid, radius_at.y + 6.0 * cos_mid),
                Anchor::Middle,
            )),
            &[keys::RADIUS],
        ))
        .part(callout(
            shape.point(quarter, shape.r - shape.ws / 2.0),
            shape.point(quarter, ring),
            Some((
                t!(i18n, ids::PCB_PRIM_WIDTH).into(),
                shape.point(quarter, ring + 16.0),
                Anchor::Middle,
            )),
            &[keys::WIDTH],
        ));
    let name = f.layer.as_ref().map_or("", |l| l.label.as_str());
    let place = copper_place(&f.layers, layer);
    drawing.part(layer_note(i18n, &layer_label(i18n, layer, place, name)))
}

/// Where `layer` sits among the copper layers of `layers`, top first.
fn copper_place(layers: &[AreaLayerChoice], layer: PcbLayer) -> Option<usize> {
    layers
        .iter()
        .filter(|c| matches!(c.layer, PcbLayer::Copper(_)))
        .position(|c| c.layer == layer)
}

/// The arc fitted into the view box: centre, radius and width in view
/// units, the angles in degrees.
struct Shape {
    centre: Point,
    r: f32,
    ws: f32,
    start: f32,
    sweep: f32,
}

impl Shape {
    /// Fit an arc of `radius` and `width` mm into [`FIT_BOX`], with
    /// [`LABEL_ROOM`] round it, as the mockup fits it twice.
    fn fit(radius: f32, width: f32, start: f32, sweep: f32) -> Self {
        let bounds = |room: f32| {
            let outer = radius + width / 2.0 + room;
            let steps = ((sweep / 10.0).ceil() as usize).max(8);
            let (mut x0, mut y0, mut x1, mut y1) =
                (-width / 2.0, -width / 2.0, width / 2.0, width / 2.0);
            for i in 0..=steps {
                let a = (start + sweep * i as f32 / steps as f32).to_radians();
                x0 = x0.min(outer * a.cos());
                x1 = x1.max(outer * a.cos());
                y0 = y0.min(outer * a.sin());
                y1 = y1.max(outer * a.sin());
            }
            (x0, y0, x1, y1)
        };
        let (bx0, by0, bx1, by1) = FIT_BOX;
        let scale_of = |b: (f32, f32, f32, f32)| {
            let w = (b.2 - b.0).max(1e-6);
            let h = (b.3 - b.1).max(1e-6);
            ((bx1 - bx0) / w).min((by1 - by0) / h).min(MAX_SCALE)
        };
        let first = scale_of(bounds(0.0));
        let b = bounds(LABEL_ROOM / first);
        let s = scale_of(b);
        let centre = Point::new(
            (bx0 + bx1) / 2.0 - (b.0 + b.2) / 2.0 * s,
            (by0 + by1) / 2.0 + (b.1 + b.3) / 2.0 * s,
        );
        Self {
            centre,
            r: radius * s,
            ws: (width * s).max(1.5),
            start,
            sweep,
        }
    }

    /// The point at `angle` degrees and `distance` from the centre, Y up.
    fn point(&self, angle: f32, distance: f32) -> Point {
        let (sin, cos) = angle.to_radians().sin_cos();
        Point::new(
            self.centre.x + distance * cos,
            self.centre.y - distance * sin,
        )
    }

    /// The arc as a polyline, one point every few degrees.
    fn points(&self) -> Vec<Point> {
        let steps = ((self.sweep / 3.0).ceil() as usize).max(2);
        (0..=steps)
            .map(|i| self.point(self.start + self.sweep * i as f32 / steps as f32, self.r))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use pcb_geom::ItemRef;
    use signex_widgets::property_illustration::Primitive;
    use uuid::Uuid;

    use super::super::fixture::{common, lights};
    use super::*;

    fn facts() -> ArcFacts {
        ArcFacts {
            common: common(vec![ItemRef::Arc(Uuid::nil())]),
            is_copper: true,
            layer: None,
            layers: Vec::new(),
            center: (Some(8.0), Some(33.5)),
            radius: Some(2.0),
            start_angle: Some(0.0),
            end_angle: Some(90.0),
            width: Some(0.5),
            length_mm: None,
            mode: None,
        }
    }

    #[test]
    fn radius_width_angles_and_centre_light_from_their_rows() {
        let drawing = illustration(&I18n::en_us(), &facts());
        for key in [keys::RADIUS, keys::WIDTH, keys::ANGLES, keys::CENTRE] {
            assert!(lights(&drawing, &key), "{key}");
        }
        let rays = drawing
            .visible_parts()
            .filter(|p| {
                matches!(p.primitive, Primitive::Line { .. }) && p.is_linked_to(&keys::ANGLES)
            })
            .count();
        assert_eq!(rays, 2, "a start and an end ray");
    }

    #[test]
    fn the_quarter_arc_fits_the_view_box() {
        let shape = Shape::fit(2.0, 0.5, 0.0, 90.0);
        for p in shape.points() {
            assert!((0.0..=super::super::VIEW_W).contains(&p.x), "{p:?}");
            assert!((0.0..=super::super::VIEW_H).contains(&p.y), "{p:?}");
        }
    }
}
