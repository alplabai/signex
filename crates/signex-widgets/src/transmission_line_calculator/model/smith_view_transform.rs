use iced::{Point, Vector};

const MINIMUM_SCALE: f32 = 0.5;
const MAXIMUM_SCALE: f32 = 16.0;

/// Applies uniform zoom and screen-space pan through a homogeneous 3x3 matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmithViewTransform {
    matrix: [[f32; 3]; 3],
}

impl SmithViewTransform {
    pub const MINIMUM_SCALE: f32 = MINIMUM_SCALE;
    pub const MAXIMUM_SCALE: f32 = MAXIMUM_SCALE;

    /// Returns the identity view transformation.
    pub const fn identity() -> Self {
        Self {
            matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Returns the uniform scale encoded by the transformation matrix.
    pub fn scale(self) -> f32 {
        self.matrix[0][0]
    }

    /// Returns the screen-space translation encoded by the matrix.
    pub fn translation(self) -> Vector {
        Vector::new(self.matrix[0][2], self.matrix[1][2])
    }

    /// Applies the affine view matrix to a screen point.
    pub fn transform_point(self, point: Point) -> Point {
        Point::new(
            self.matrix[0][0].mul_add(point.x, self.matrix[0][2]),
            self.matrix[1][1].mul_add(point.y, self.matrix[1][2]),
        )
    }

    /// Returns the inverse affine view transform when it is finite.
    pub fn inverse(self) -> Option<Self> {
        let scale = self.scale();
        if !scale.is_finite() || scale.abs() <= f32::EPSILON {
            return None;
        }

        let inverse_scale = scale.recip();
        let translation = self.translation();
        Some(Self {
            matrix: [
                [inverse_scale, 0.0, -translation.x * inverse_scale],
                [0.0, inverse_scale, -translation.y * inverse_scale],
                [0.0, 0.0, 1.0],
            ],
        })
    }

    /// Maps a screen point through the inverse view transform.
    pub fn inverse_transform_point(self, point: Point) -> Option<Point> {
        self.inverse().map(|inverse| inverse.transform_point(point))
    }

    /// Returns the transform after a screen-space translation.
    pub fn translated(self, delta: Vector) -> Self {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return self;
        }

        Self::translation_matrix(delta).multiply(self)
    }

    /// Returns a bounded zoom transform that keeps the anchor point fixed.
    pub fn zoomed_at(self, anchor: Point, factor: f32) -> Self {
        if !anchor.x.is_finite() || !anchor.y.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return self;
        }

        let current_scale = self.scale();
        let target_scale = (current_scale * factor).clamp(MINIMUM_SCALE, MAXIMUM_SCALE);
        let actual_factor = target_scale / current_scale;
        if !actual_factor.is_finite() || (actual_factor - 1.0).abs() <= f32::EPSILON {
            return self;
        }

        Self::translation_matrix(Vector::new(anchor.x, anchor.y))
            .multiply(Self::scale_matrix(actual_factor))
            .multiply(Self::translation_matrix(Vector::new(-anchor.x, -anchor.y)))
            .multiply(self)
    }

    /// Returns whether the transform leaves chart coordinates unchanged.
    pub fn is_identity(self) -> bool {
        self == Self::identity()
    }

    /// Creates a homogeneous translation matrix.
    fn translation_matrix(translation: Vector) -> Self {
        Self {
            matrix: [
                [1.0, 0.0, translation.x],
                [0.0, 1.0, translation.y],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a homogeneous matrix for uniform scaling.
    fn scale_matrix(scale: f32) -> Self {
        Self {
            matrix: [[scale, 0.0, 0.0], [0.0, scale, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Multiplies two homogeneous transformation matrices.
    fn multiply(self, right: Self) -> Self {
        let mut matrix = [[0.0; 3]; 3];
        for (row, values) in matrix.iter_mut().enumerate() {
            for (column, value) in values.iter_mut().enumerate() {
                *value = (0..3)
                    .map(|index| self.matrix[row][index] * right.matrix[index][column])
                    .sum();
            }
        }
        Self { matrix }
    }
}

impl Default for SmithViewTransform {
    /// Creates the default value for this type.
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
#[path = "../../../tests/transmission_line_calculator/smith_view_transform_tests.rs"]
mod tests;
