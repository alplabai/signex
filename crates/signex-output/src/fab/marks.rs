//! What a Gerber image is made of: flashes, strokes and filled regions, all
//! in the fabrication frame (`frame::FabFrame`), in integer nm.

use pcb_geom::PointNm;

/// The shape of a flashed aperture. Lengths in nm, angles in
/// micro-degrees counter-clockwise in the fabrication frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApertureShape {
    Circle {
        diameter: i64,
    },
    /// An axis-aligned rectangle.
    Rect {
        w: i64,
        h: i64,
    },
    /// An axis-aligned stadium.
    Obround {
        w: i64,
        h: i64,
    },
    /// A rectangle turned off the axes.
    TurnedRect {
        w: i64,
        h: i64,
        udeg: i64,
    },
    /// A stadium turned off the axes.
    TurnedObround {
        w: i64,
        h: i64,
        udeg: i64,
    },
    /// A rectangle with corner radius `r`, at any angle.
    RoundRect {
        w: i64,
        h: i64,
        r: i64,
        udeg: i64,
    },
}

/// The X2 `.AperFunction` of an aperture: what its marks are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Function {
    SmdPad,
    ComponentPad,
    ConnectorPad,
    /// The copper of a non-plated hole's pad.
    NonPlatedPad,
    ViaPad,
    Conductor,
    NonConductor,
    Profile,
    /// A V-score line on the V-cut image (#2410).
    VScore,
    /// A pad's opening on a solder mask or paste image. The X2 pad
    /// functions are for copper images only.
    PadOpening,
    /// A drawing on a legend, mask or paste image.
    Graphic,
}

impl Function {
    pub(crate) fn x2(self) -> &'static str {
        match self {
            Function::SmdPad => "SMDPad,CuDef",
            Function::ComponentPad => "ComponentPad",
            Function::ConnectorPad => "ConnectorPad",
            Function::NonPlatedPad => "OtherPad,NPTH",
            Function::ViaPad => "ViaPad",
            Function::Conductor => "Conductor",
            Function::NonConductor => "NonConductor",
            Function::Profile => "Profile",
            Function::VScore => "Other,VScore",
            Function::PadOpening => "Other,PadOpening",
            Function::Graphic => "Other,Graphic",
        }
    }
}

/// X2 object attributes, written on copper images only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Attributes {
    /// `.N`: the net name.
    pub net: Option<String>,
    /// `.P` and `.C`: the component designator and the pad number.
    pub pin: Option<(String, String)>,
}

impl Attributes {
    pub(crate) fn net(net: Option<String>) -> Self {
        Self { net, pin: None }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.net.is_none() && self.pin.is_none()
    }
}

/// One step of a stroked path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Line(PointNm),
    /// An arc to `to` about `center`, counter-clockwise when `ccw`. An arc
    /// back to its own start is a full circle.
    Arc {
        to: PointNm,
        center: PointNm,
        ccw: bool,
    },
}

/// One mark on an image.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Mark {
    Flash {
        shape: ApertureShape,
        function: Function,
        at: PointNm,
        attributes: Attributes,
    },
    /// A path drawn with a round pen of `width` nm.
    Stroke {
        width: i64,
        function: Function,
        start: PointNm,
        steps: Vec<Step>,
        attributes: Attributes,
    },
    /// A filled polygon with polygon holes.
    Region {
        outline: Vec<PointNm>,
        holes: Vec<Vec<PointNm>>,
        attributes: Attributes,
    },
}

impl Mark {
    /// A closed polyline through `points`, drawn with a round pen.
    pub(crate) fn closed_path(points: &[PointNm], width: i64, function: Function) -> Option<Mark> {
        let (&start, rest) = points.split_first()?;
        let mut steps: Vec<Step> = rest.iter().copied().map(Step::Line).collect();
        steps.push(Step::Line(start));
        Some(Mark::Stroke {
            width,
            function,
            start,
            steps,
            attributes: Attributes::default(),
        })
    }

    /// A circle of `radius` about `center`, drawn with a round pen.
    pub(crate) fn circle(center: PointNm, radius: i64, width: i64, function: Function) -> Mark {
        let start = PointNm::new(center.x + radius, center.y);
        Mark::Stroke {
            width,
            function,
            start,
            steps: vec![Step::Arc {
                to: start,
                center,
                ccw: true,
            }],
            attributes: Attributes::default(),
        }
    }
}

/// Twice the signed area of a ring, positive counter-clockwise.
pub(crate) fn doubled_area(ring: &[PointNm]) -> i128 {
    let n = ring.len();
    (0..n)
        .map(|i| ring[i].cross(ring[(i + 1) % n]))
        .sum::<i128>()
}

/// Whether the arc from `start` through `mid` to `end` turns
/// counter-clockwise.
pub(crate) fn turns_ccw(start: PointNm, mid: PointNm, end: PointNm) -> bool {
    mid.minus(start).cross(end.minus(mid)) > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_path_returns_to_its_start() {
        let pts = [PointNm::new(0, 0), PointNm::new(10, 0), PointNm::new(10, 5)];
        let Some(Mark::Stroke { start, steps, .. }) = Mark::closed_path(&pts, 1, Function::Profile)
        else {
            panic!("a stroke");
        };
        assert_eq!(start, pts[0]);
        assert_eq!(steps.last(), Some(&Step::Line(pts[0])));
        assert_eq!(steps.len(), 3);
        assert!(Mark::closed_path(&[], 1, Function::Profile).is_none());
    }

    #[test]
    fn ring_area_and_turn_follow_the_frame() {
        let ccw = [PointNm::new(0, 0), PointNm::new(4, 0), PointNm::new(4, 2)];
        assert_eq!(doubled_area(&ccw), 8);
        assert!(turns_ccw(
            PointNm::new(1, 0),
            PointNm::new(0, 1),
            PointNm::new(-1, 0)
        ));
        assert!(!turns_ccw(
            PointNm::new(1, 0),
            PointNm::new(0, -1),
            PointNm::new(-1, 0)
        ));
    }
}
