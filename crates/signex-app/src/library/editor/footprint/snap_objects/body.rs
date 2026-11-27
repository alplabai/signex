//! The footprint's 3D body as the object snap sees it: the box the 3D
//! preview draws (`preview3d::body_bbox`), projected into the footprint
//! plane (#2726). Top and bottom faces project onto the same rectangle.

use signex_library::primitive::footprint::Footprint;

/// The 3D body's box seen from above, in footprint mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyBox {
    pub min: (f64, f64),
    pub max: (f64, f64),
}

impl BodyBox {
    /// The body of `footprint`, as its 3D preview draws it.
    pub fn of(footprint: &Footprint) -> Self {
        let (x0, y0, x1, y1) = super::super::preview3d::body_bbox(footprint);
        Self {
            min: (x0.min(x1), y0.min(y1)),
            max: (x0.max(x1), y0.max(y1)),
        }
    }

    /// The centre, where the body's vertical axis meets the plane.
    pub fn centre(self) -> (f64, f64) {
        (
            (self.min.0 + self.max.0) / 2.0,
            (self.min.1 + self.max.1) / 2.0,
        )
    }

    /// The four vertical edges of the box, seen end on.
    pub fn corners(self) -> [(f64, f64); 4] {
        [
            self.min,
            (self.max.0, self.min.1),
            self.max,
            (self.min.0, self.max.1),
        ]
    }

    /// The body's two horizontal axes through its centre, each across the
    /// box.
    pub fn axes(self) -> [((f64, f64), (f64, f64)); 2] {
        let (cx, cy) = self.centre();
        [
            ((self.min.0, cy), (self.max.0, cy)),
            ((cx, self.min.1), (cx, self.max.1)),
        ]
    }

    /// Whether the box has finite corners to snap to.
    pub fn is_usable(self) -> bool {
        [self.min.0, self.min.1, self.max.0, self.max.1]
            .iter()
            .all(|v| v.is_finite())
    }
}
