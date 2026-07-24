use serde::{Deserialize, Serialize};

/// Represents a Cartesian point on the three-dimensional Smith sphere.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(super) struct SmithSpherePoint {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) z: f64,
}

impl SmithSpherePoint {
    #[cfg(test)]
    pub(super) const NORTH_POLE: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    #[cfg(test)]
    pub(super) const SOUTH_POLE: Self = Self {
        x: 0.0,
        y: 0.0,
        z: -1.0,
    };

    /// Creates a sphere point from Cartesian coordinates.
    pub(super) const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}
