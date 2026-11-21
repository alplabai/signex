//! An angle dimension whose parameter is read in the opposite sense to the
//! model's. A board shows Y up while the model's y runs down, so a board
//! sketch's parameter holds the value the user reads and the dimension
//! stores its negative (`-angle_1`): the solver sees the same angle.

use signex_expr::{expression_body, is_identifier};

use crate::constraint::{ConstraintKind, DimTarget};
use crate::id::ConstraintId;
use crate::parameter_refs::{literal_of, unique_name};
use crate::sketch::SketchData;
use crate::unit::Quantity;

/// The parameter `source` reads negated (`-angle_1`), if that is all it is.
pub fn mirrored_parameter(source: &str) -> Option<&str> {
    let name = expression_body(source).strip_prefix('-')?;
    is_identifier(name).then_some(name)
}

/// The expression reading the parameter `name` negated.
pub fn mirrored_reference(name: &str) -> String {
    format!("-{name}")
}

/// Name the literal angle `constraint` by the value read in the opposite
/// sense: a new parameter holds that value and the dimension reads its
/// negative, so the angle the solver sees is the same to within the 1e-9
/// degree a parameter's literal keeps. A dimension already reading one this
/// way keeps it. `None` for anything else, which this does not touch.
pub fn parameter_from_mirrored_angle(
    sketch: &mut SketchData,
    constraint: ConstraintId,
) -> Option<String> {
    let index = sketch.constraints.iter().position(|c| c.id == constraint)?;
    let ConstraintKind::Angle { target, .. } = &sketch.constraints[index].kind else {
        return None;
    };
    let value = match target {
        DimTarget::Literal(rad) => 0.0 - *rad,
        DimTarget::Expr(source) => {
            let name = mirrored_parameter(source)?;
            let known = sketch.parameters.get_raw(name).is_some();
            return known.then(|| name.to_string());
        }
    };
    let name = unique_name(&sketch.parameters, "angle");
    sketch
        .parameters
        .insert(name.clone(), literal_of(Quantity::angle(value)));
    if let ConstraintKind::Angle { target, .. } = &mut sketch.constraints[index].kind {
        *target = DimTarget::Expr(mirrored_reference(&name));
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_negated_name_is_a_mirrored_parameter() {
        assert_eq!(mirrored_parameter("-angle_1"), Some("angle_1"));
        assert_eq!(mirrored_parameter("= -angle_1"), Some("angle_1"));
        assert_eq!(mirrored_parameter("angle_1"), None);
        assert_eq!(mirrored_parameter("-30deg"), None);
        assert_eq!(mirrored_parameter("-(a + b)"), None);
    }
}
