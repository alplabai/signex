//! Where a footprint's property text (designator, comment) sits on the
//! board (#2368).
//!
//! A property's position and rotation are footprint-local, as a pad's are,
//! so the text follows the footprint's move, turn and flip through the same
//! [`Placement`]. A property with no position takes the default spot: the
//! designator [`PCB_FP_TEXT_OFFSET_MM`] above the origin, the comment that
//! far below it, any other text at the origin.

use signex_types::pcb::{Footprint, PCB_FP_TEXT_OFFSET_MM, Point};
use signex_types::property::PcbProperty;

use crate::transform::{Placement, Side};
use crate::units::{GeomError, PointNm};

/// The designator's property key.
const DESIGNATOR_KEY: &str = "Reference";
/// The comment's property key.
const COMMENT_KEY: &str = "Value";

/// Whether a property of `key` is a text the board shows (the designator or
/// the comment) rather than a parameter.
pub fn is_board_text_key(key: &str) -> bool {
    key == DESIGNATOR_KEY || key == COMMENT_KEY
}

/// A property text placed on the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextPlacement {
    /// The text's anchor; the text is centred on it.
    pub anchor: PointNm,
    /// The angle of the text's baseline with the board's x axis, degrees,
    /// in the sense a pad turns.
    pub rotation_deg: f64,
    /// Drawn mirrored: the footprint sits on the bottom side.
    pub mirrored: bool,
}

/// Where `property` of `footprint` is drawn. Refuses a non-finite position,
/// angle or footprint placement.
pub fn property_text_placement(
    footprint: &Footprint,
    property: &PcbProperty,
) -> Result<TextPlacement, GeomError> {
    if !property.rotation.is_finite() {
        return Err(GeomError::Unrepresentable("property.rotation"));
    }
    let placement = Placement::of_footprint(footprint)?;
    let local = property
        .position
        .unwrap_or_else(|| default_position(&property.key));
    let anchor = placement.to_world(PointNm::from_mm(local, "property.position")?);
    // The x mirror reverses the text's own turn (`M R(t) M = R(-t)`), as
    // for a pad (`pad_world_rotation_deg`).
    let (own, mirrored) = match placement.side {
        Side::Top => (property.rotation, false),
        Side::Bottom => (-property.rotation, true),
    };
    Ok(TextPlacement {
        anchor,
        rotation_deg: placement.rotation_deg + own,
        mirrored,
    })
}

fn default_position(key: &str) -> Point {
    match key {
        DESIGNATOR_KEY => Point::new(0.0, -PCB_FP_TEXT_OFFSET_MM),
        COMMENT_KEY => Point::new(0.0, PCB_FP_TEXT_OFFSET_MM),
        _ => Point::new(0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn footprint(rotation: f64, layer: &str) -> Footprint {
        Footprint {
            uuid: Uuid::nil(),
            reference: "R1".into(),
            value: String::new(),
            footprint_id: String::new(),
            position: Point::new(10.0, 20.0),
            rotation,
            layer: layer.into(),
            locked: false,
            pads: Vec::new(),
            graphics: Vec::new(),
            properties: Vec::new(),
            schematic_link: None,
            library_ref: None,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    fn designator(position: Option<Point>, rotation: f64) -> PcbProperty {
        PcbProperty {
            key: DESIGNATOR_KEY.into(),
            value: "R1".into(),
            position,
            rotation,
            layer: Some("TopSilkscreen".into()),
            font_size: None,
            hidden: false,
        }
    }

    fn mm(p: PointNm) -> (f64, f64) {
        let p = p.to_mm();
        (p.x, p.y)
    }

    #[test]
    fn the_text_follows_the_footprint_placement() {
        let text = designator(Some(Point::new(1.0, 0.0)), 0.0);
        let placed = property_text_placement(&footprint(0.0, "TopCopper"), &text).unwrap();
        assert_eq!(mm(placed.anchor), (11.0, 20.0));
        assert_eq!(placed.rotation_deg, 0.0);
        assert!(!placed.mirrored);
        // A quarter turn carries the local x axis onto the board's y axis.
        let placed = property_text_placement(&footprint(90.0, "TopCopper"), &text).unwrap();
        assert_eq!(mm(placed.anchor), (10.0, 21.0));
        assert_eq!(placed.rotation_deg, 90.0);
    }

    #[test]
    fn a_bottom_footprint_mirrors_its_text() {
        let text = designator(Some(Point::new(1.0, 0.5)), 30.0);
        let placed = property_text_placement(&footprint(90.0, "BottomCopper"), &text).unwrap();
        // Mirrored x (-1, 0.5), turned 90 degrees: (-0.5, -1).
        assert_eq!(mm(placed.anchor), (9.5, 19.0));
        assert_eq!(placed.rotation_deg, 60.0);
        assert!(placed.mirrored);
    }

    #[test]
    fn a_text_with_no_position_takes_the_default_spot() {
        let placed =
            property_text_placement(&footprint(0.0, "TopCopper"), &designator(None, 0.0)).unwrap();
        assert_eq!(mm(placed.anchor), (10.0, 20.0 - PCB_FP_TEXT_OFFSET_MM));
        let comment = PcbProperty {
            key: COMMENT_KEY.into(),
            ..designator(None, 0.0)
        };
        let placed = property_text_placement(&footprint(0.0, "TopCopper"), &comment).unwrap();
        assert_eq!(mm(placed.anchor), (10.0, 20.0 + PCB_FP_TEXT_OFFSET_MM));
    }

    #[test]
    fn only_the_designator_and_comment_are_board_texts() {
        assert!(is_board_text_key("Reference") && is_board_text_key("Value"));
        assert!(!is_board_text_key("MPN") && !is_board_text_key("value"));
    }

    #[test]
    fn a_non_finite_text_is_refused() {
        let text = designator(None, f64::NAN);
        assert!(property_text_placement(&footprint(0.0, "TopCopper"), &text).is_err());
        let text = designator(Some(Point::new(f64::INFINITY, 0.0)), 0.0);
        assert!(property_text_placement(&footprint(0.0, "TopCopper"), &text).is_err());
    }
}
