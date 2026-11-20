//! The four quarter-turn steps of every Rotation control in Properties: a
//! placed object's, a text note's and the pre-placement form's. One place
//! decides which step a stored angle shows.

/// The quarter turns a Rotation control offers, in degrees.
pub const ROTATION_STEPS: [f64; 4] = [0.0, 90.0, 180.0, 270.0];

/// The step nearest a stored angle: what a placed object's control marks.
/// A non-finite angle reads as 0.
pub fn nearest_rotation_step(degrees: f64) -> f64 {
    let turns = (degrees.rem_euclid(360.0) / 90.0).round();
    let index = if turns.is_finite() {
        turns as usize % 4
    } else {
        0
    };
    ROTATION_STEPS[index]
}

/// The step an angle sits on when rounded to whole degrees (89.9999 is 90),
/// `None` off the quarter turns: the pre-placement control marks no step
/// for a turn it cannot set.
pub fn exact_rotation_step(degrees: f64) -> Option<f64> {
    let step = nearest_rotation_step(degrees);
    (degrees.round().rem_euclid(360.0) == step).then_some(step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_angle_shows_as_its_nearest_step() {
        assert_eq!(nearest_rotation_step(89.6), 90.0);
        assert_eq!(nearest_rotation_step(-90.0), 270.0);
        assert_eq!(nearest_rotation_step(359.0), 0.0);
        assert_eq!(nearest_rotation_step(f64::NAN), 0.0);
    }

    #[test]
    fn only_an_angle_on_a_step_marks_it_exactly() {
        assert_eq!(exact_rotation_step(89.9999), Some(90.0));
        assert_eq!(exact_rotation_step(360.0), Some(0.0));
        assert_eq!(exact_rotation_step(-90.0), Some(270.0));
        assert_eq!(exact_rotation_step(45.0), None);
        assert_eq!(exact_rotation_step(88.0), None);
        assert_eq!(exact_rotation_step(f64::NAN), None);
    }
}
