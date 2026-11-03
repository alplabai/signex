//! Footprint preview geometry: what extent the drawing covers, how it
//! fits the box, and the size / scale readouts. Pure, so it is tested
//! without a renderer.

use iced::{Point, Rectangle};
use signex_library::primitive::{Footprint, FpGraphic, FpGraphicKind, Pad};

/// Courtyard clearance added around the pads when a footprint stores no
/// courtyard of its own (IPC-7351 nominal excess, mm).
pub(super) const DERIVED_COURTYARD_EXCESS_MM: f64 = 0.25;
/// Floor for a span so a single-point footprint still gets a finite scale.
const MIN_SPAN_MM: f64 = 0.1;
/// Scale-bar lengths come from this mantissa ladder times a power of ten.
const NICE_STEPS: [f64; 3] = [1.0, 2.0, 5.0];

/// Footprint-local bounding box in mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Extent {
    pub lo: [f64; 2],
    pub hi: [f64; 2],
}

impl Extent {
    pub fn width(&self) -> f64 {
        self.hi[0] - self.lo[0]
    }

    pub fn height(&self) -> f64 {
        self.hi[1] - self.lo[1]
    }

    fn from_points(points: &[[f64; 2]]) -> Option<Self> {
        let first = *points.first()?;
        Some(points.iter().fold(
            Self {
                lo: first,
                hi: first,
            },
            |e, p| Self {
                lo: [e.lo[0].min(p[0]), e.lo[1].min(p[1])],
                hi: [e.hi[0].max(p[0]), e.hi[1].max(p[1])],
            },
        ))
    }
}

/// The courtyard to draw and whether it was derived from the pads.
pub(super) struct Courtyard {
    pub points: Vec<[f64; 2]>,
    pub derived: bool,
}

/// The stored courtyard, or a rectangle around the pads when the footprint
/// stores none, so every footprint shows its keep-out.
pub(super) fn courtyard(fp: &Footprint) -> Option<Courtyard> {
    if fp.courtyard.points.len() > 2 {
        return Some(Courtyard {
            points: fp.courtyard.points.clone(),
            derived: false,
        });
    }
    let pads = Extent::from_points(&pad_points(&fp.pads))?;
    let e = DERIVED_COURTYARD_EXCESS_MM;
    Some(Courtyard {
        points: vec![
            [pads.lo[0] - e, pads.lo[1] - e],
            [pads.hi[0] + e, pads.lo[1] - e],
            [pads.hi[0] + e, pads.hi[1] + e],
            [pads.lo[0] - e, pads.hi[1] + e],
        ],
        derived: true,
    })
}

/// Corners of each pad's rotated bounding box.
fn pad_points(pads: &[Pad]) -> Vec<[f64; 2]> {
    pads.iter()
        .flat_map(|pad| {
            let (sin, cos) = pad.rotation.to_radians().sin_cos();
            let hx = (pad.size[0] / 2.0 * cos).abs() + (pad.size[1] / 2.0 * sin).abs();
            let hy = (pad.size[0] / 2.0 * sin).abs() + (pad.size[1] / 2.0 * cos).abs();
            let [x, y] = pad.position;
            [[x - hx, y - hy], [x + hx, y + hy]]
        })
        .collect()
}

/// Bounding box of everything the preview draws, `None` for an empty
/// footprint.
pub(super) fn extent(fp: &Footprint) -> Option<Extent> {
    let mut points = pad_points(&fp.pads);
    if let Some(court) = courtyard(fp) {
        points.extend(court.points);
    }
    let graphics = [&fp.silk_f, &fp.silk_b, &fp.fab_f, &fp.fab_b];
    for g in graphics.into_iter().flatten() {
        graphic_points(g, &mut points);
    }
    Extent::from_points(&points)
}

fn graphic_points(g: &FpGraphic, out: &mut Vec<[f64; 2]>) {
    match &g.kind {
        FpGraphicKind::Line { from, to } | FpGraphicKind::Rectangle { from, to } => {
            out.extend([*from, *to]);
        }
        FpGraphicKind::Circle { center, radius } | FpGraphicKind::Arc { center, radius, .. } => {
            out.push([center[0] - radius, center[1] - radius]);
            out.push([center[0] + radius, center[1] + radius]);
        }
        FpGraphicKind::Polygon { vertices } => out.extend(vertices.iter().copied()),
        FpGraphicKind::Text { position, .. } => out.push(*position),
    }
}

/// Mapping from footprint mm (y-down, like the canvas) to canvas pixels.
#[derive(Clone, Copy, Debug)]
pub(super) struct Fit {
    /// Pixels per mm.
    pub scale: f64,
    mid: [f64; 2],
    centre: Point,
}

impl Fit {
    /// Largest scale at which `extent` fits `area`, centred in it.
    pub fn new(extent: Extent, area: Rectangle) -> Option<Self> {
        if area.width <= 0.0 || area.height <= 0.0 {
            return None;
        }
        let span = [
            extent.width().max(MIN_SPAN_MM),
            extent.height().max(MIN_SPAN_MM),
        ];
        let scale = (f64::from(area.width) / span[0]).min(f64::from(area.height) / span[1]);
        Some(Self {
            scale,
            mid: [
                (extent.lo[0] + extent.hi[0]) / 2.0,
                (extent.lo[1] + extent.hi[1]) / 2.0,
            ],
            centre: area.center(),
        })
    }

    pub fn to_px(self, p: [f64; 2]) -> Point {
        Point::new(
            self.centre.x + ((p[0] - self.mid[0]) * self.scale) as f32,
            self.centre.y + ((p[1] - self.mid[1]) * self.scale) as f32,
        )
    }

    pub fn px(&self, mm: f64) -> f32 {
        (mm * self.scale) as f32
    }
}

/// "3.50 × 1.75 mm" — the overall size of the drawing.
pub(super) fn size_label(extent: &Extent) -> String {
    format!("{:.2} × {:.2} mm", extent.width(), extent.height())
}

/// Largest 1 / 2 / 5 × 10^n mm length not above `target_mm`.
pub(super) fn scale_bar_mm(target_mm: f64) -> Option<f64> {
    if !target_mm.is_finite() || target_mm <= 0.0 {
        return None;
    }
    let decade = 10f64.powf(target_mm.log10().floor());
    NICE_STEPS
        .iter()
        .map(|step| step * decade)
        .rfind(|len| *len <= target_mm * (1.0 + 1e-9))
}

/// "0.5 mm" / "2 mm" label for a scale bar.
pub(super) fn scale_bar_label(mm: f64) -> String {
    format!("{mm} mm")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use iced::Size;
    use signex_library::primitive::Polygon;
    use uuid::Uuid;

    fn chip() -> Footprint {
        let mut fp = Footprint::with_identity("Chip-0805", Uuid::nil(), Utc::now());
        for x in [-1.0, 1.0] {
            fp.pads.push(Pad {
                number: "1".into(),
                size: [1.0, 1.25],
                position: [x, 0.0],
                ..Pad::default()
            });
        }
        fp
    }

    #[test]
    fn a_footprint_without_a_courtyard_gets_one_around_its_pads() {
        let court = courtyard(&chip()).expect("pads give a courtyard");
        assert!(court.derived);
        assert_eq!(court.points[0], [-1.75, -0.875]);
        assert_eq!(court.points[2], [1.75, 0.875]);
    }

    #[test]
    fn a_stored_courtyard_is_used_as_is() {
        let mut fp = chip();
        fp.courtyard = Polygon::new(vec![[-1.9, -0.9], [1.9, -0.9], [1.9, 0.9], [-1.9, 0.9]]);
        let court = courtyard(&fp).unwrap();
        assert!(!court.derived);
        assert_eq!(court.points.len(), 4);
        let e = extent(&fp).unwrap();
        assert_eq!((e.lo, e.hi), ([-1.9, -0.9], [1.9, 0.9]));
    }

    #[test]
    fn an_empty_footprint_has_no_extent() {
        let fp = Footprint::with_identity("empty", Uuid::nil(), Utc::now());
        assert!(courtyard(&fp).is_none());
        assert!(extent(&fp).is_none());
    }

    #[test]
    fn a_rotated_pad_widens_the_extent_the_other_way() {
        let mut fp = Footprint::with_identity("p", Uuid::nil(), Utc::now());
        fp.pads.push(Pad {
            size: [2.0, 1.0],
            rotation: 90.0,
            ..Pad::default()
        });
        let e = Extent::from_points(&pad_points(&fp.pads)).unwrap();
        assert!((e.width() - 1.0).abs() < 1e-9 && (e.height() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_size_label_reports_the_drawing_extent() {
        let e = extent(&chip()).unwrap();
        assert_eq!(size_label(&e), "3.50 × 1.75 mm");
    }

    #[test]
    fn the_fit_keeps_the_whole_extent_inside_the_area_and_centred() {
        let e = extent(&chip()).unwrap();
        let area = Rectangle::new(Point::new(10.0, 5.0), Size::new(300.0, 100.0));
        let fit = Fit::new(e, area).unwrap();
        let (lo, hi) = (fit.to_px(e.lo), fit.to_px(e.hi));
        for p in [lo, hi] {
            assert!(p.x >= area.x - 0.01 && p.x <= area.x + area.width + 0.01);
            assert!(p.y >= area.y - 0.01 && p.y <= area.y + area.height + 0.01);
        }
        // Height is the limiting side here, so the drawing spans all of it.
        assert!((hi.y - lo.y - area.height).abs() < 0.01);
        let mid = fit.to_px([(e.lo[0] + e.hi[0]) / 2.0, (e.lo[1] + e.hi[1]) / 2.0]);
        assert!((mid.x - area.center().x).abs() < 0.01);
    }

    #[test]
    fn a_zero_sized_area_has_no_fit() {
        let e = extent(&chip()).unwrap();
        assert!(Fit::new(e, Rectangle::new(Point::ORIGIN, Size::new(0.0, 50.0))).is_none());
    }

    #[test]
    fn scale_bar_lengths_follow_the_1_2_5_ladder() {
        assert_eq!(scale_bar_mm(1.0), Some(1.0));
        assert_eq!(scale_bar_mm(1.9), Some(1.0));
        assert_eq!(scale_bar_mm(4.9), Some(2.0));
        assert_eq!(scale_bar_mm(5.0), Some(5.0));
        assert_eq!(scale_bar_mm(0.7), Some(0.5));
        assert_eq!(scale_bar_mm(12.0), Some(10.0));
        assert_eq!(scale_bar_mm(0.0), None);
        assert_eq!(scale_bar_label(0.5), "0.5 mm");
    }
}
