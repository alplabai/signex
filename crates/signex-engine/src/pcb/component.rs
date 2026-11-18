//! The edits of the Properties panel's Component view (#2354): the location,
//! side, lock, text visibility and parameters of the selected footprints.
//!
//! Pure: each function reads a board and returns the commands, which the
//! caller runs as one undo step. A locked footprint takes only an unlock;
//! every other edit of it is refused, as placement and text edits are.
//! Footprints already as asked get no command, so an edit that changes
//! nothing makes no undo step.

use std::collections::BTreeSet;

use signex_types::numeric::parse_finite_f64;
use signex_types::pcb::{Footprint, PcbBoard, Point};
use signex_types::pcb_layer::BoardSide;
use signex_types::property::PcbProperty;
use uuid::Uuid;

use super::command::BoardCommand;
use super::designators::FootprintTextField;
use super::error::BoardCommandError;
use super::footprint_content::{is_text_key, text_key};
use super::placement::{self, PlacementRefusal};

/// A number of the Location section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LocationField {
    X,
    Y,
    Rotation,
}

/// Why a Component edit did nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ComponentRefusal {
    #[error(transparent)]
    Placement(#[from] PlacementRefusal),
    #[error(transparent)]
    Command(#[from] BoardCommandError),
    #[error("{0:?} is not a number")]
    NotANumber(String),
    #[error("{} has no {key} text", .footprints.join(", "))]
    NoText {
        footprints: Vec<String>,
        key: &'static str,
    },
    #[error("{} has no parameter {key}", .footprints.join(", "))]
    NoParameter {
        footprints: Vec<String>,
        key: String,
    },
    #[error("{0} follows the designator or value; edit it in its own field")]
    TextParameter(String),
    #[error("{0:?} is not a whole number")]
    NotAWholeNumber(String),
    #[error("the {0} is out of range")]
    OutOfRange(&'static str),
    #[error("a parameter needs a name")]
    EmptyName,
    #[error("{} already has a parameter {key}", .footprints.join(", "))]
    ParameterTaken {
        footprints: Vec<String>,
        key: String,
    },
}

/// A number typed into a Location field: a finite decimal, with a trailing
/// `°` allowed on the rotation.
pub fn parse_location(field: LocationField, text: &str) -> Result<f64, ComponentRefusal> {
    let text = text.trim();
    let number = match field {
        LocationField::Rotation => text.trim_end_matches('°'),
        LocationField::X | LocationField::Y => text,
    };
    parse_finite_f64(number).map_err(|_| ComponentRefusal::NotANumber(text.to_string()))
}

/// Set `field` of every footprint in `ids` to `value`. X or Y moves each
/// footprint's origin to that coordinate and keeps the other; a rotation is
/// stored in `[0, 360)`.
pub fn location_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    field: LocationField,
    value: f64,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    if !value.is_finite() {
        return Err(ComponentRefusal::NotANumber(value.to_string()));
    }
    let footprints = placement::editable_footprints(board, ids)?;
    let commands = footprints
        .into_iter()
        .filter_map(|fp| {
            let footprint = fp.uuid;
            let at = fp.position;
            match field {
                LocationField::X => (at.x != value).then(|| BoardCommand::MoveFootprint {
                    footprint,
                    to: Point::new(value, at.y),
                }),
                LocationField::Y => (at.y != value).then(|| BoardCommand::MoveFootprint {
                    footprint,
                    to: Point::new(at.x, value),
                }),
                LocationField::Rotation => {
                    // `+ 0.0` turns a -0.0 remainder into 0.0.
                    let degrees = value.rem_euclid(360.0) + 0.0;
                    (fp.rotation != degrees).then_some(BoardCommand::Rotate { footprint, degrees })
                }
            }
        })
        .collect();
    Ok(commands)
}

/// The side `footprint` is on: the side its layer reads as for a flip, so a
/// layer with no side is the top.
pub fn side_of(footprint: &Footprint) -> BoardSide {
    match pcb_geom::Side::of_layer(&footprint.layer) {
        pcb_geom::Side::Top => BoardSide::Top,
        pcb_geom::Side::Bottom => BoardSide::Bottom,
    }
}

/// Put every footprint in `ids` on `side`: flip each one on the other side.
pub fn side_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    side: BoardSide,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    let footprints = placement::editable_footprints(board, ids)?;
    Ok(footprints
        .into_iter()
        .filter(|fp| side_of(fp) != side)
        .map(|fp| BoardCommand::Flip { footprint: fp.uuid })
        .collect())
}

/// Lock or unlock every footprint in `ids`; the one edit a locked footprint
/// takes.
pub fn lock_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    locked: bool,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    if ids.is_empty() {
        return Err(PlacementRefusal::NothingSelected.into());
    }
    let mut commands = Vec::new();
    for &id in ids {
        let fp = board
            .footprints
            .iter()
            .find(|fp| fp.uuid == id)
            .ok_or(BoardCommandError::FootprintNotFound(id))?;
        if fp.locked != locked {
            commands.push(BoardCommand::SetFootprintLocked {
                footprint: id,
                locked,
            });
        }
    }
    Ok(commands)
}

/// Whether the board shows `field` of `footprint`: `None` when it has no
/// text for it, else whether any text of it is visible.
pub fn text_shown(footprint: &Footprint, field: FootprintTextField) -> Option<bool> {
    let key = text_key(field);
    let mut texts = footprint.properties.iter().filter(|p| p.key == key);
    let first = texts.next()?;
    Some(!first.hidden || texts.any(|p| !p.hidden))
}

/// Show or hide `field` on every footprint in `ids`: each text of it. Refused
/// whole when one of them has no such text.
pub fn text_shown_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    field: FootprintTextField,
    shown: bool,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    let footprints = placement::editable_footprints(board, ids)?;
    let key = text_key(field);
    let missing: Vec<String> = footprints
        .iter()
        .filter(|fp| text_shown(fp, field).is_none())
        .map(|fp| placement::name(fp))
        .collect();
    if !missing.is_empty() {
        return Err(ComponentRefusal::NoText {
            footprints: missing,
            key,
        });
    }
    let mut commands = Vec::new();
    for fp in footprints {
        for (index, property) in fp.properties.iter().enumerate() {
            if property.key == key && property.hidden == shown {
                commands.push(BoardCommand::SetFootprintProperty {
                    footprint: fp.uuid,
                    index,
                    property: PcbProperty {
                        hidden: !shown,
                        ..property.clone()
                    },
                });
            }
        }
    }
    Ok(commands)
}

/// The parameter `key` of `footprint`: its first property of that key.
pub fn parameter<'a>(footprint: &'a Footprint, key: &str) -> Option<&'a PcbProperty> {
    footprint.properties.iter().find(|p| p.key == key)
}

/// The parameters the Parameters tab lists: every property key but the
/// designator and value texts, in order, each once.
pub fn parameter_keys(footprint: &Footprint) -> Vec<&str> {
    let mut keys: Vec<&str> = Vec::new();
    for property in &footprint.properties {
        if !is_text_key(&property.key) && !keys.contains(&property.key.as_str()) {
            keys.push(&property.key);
        }
    }
    keys
}

/// Set parameter `key` of every footprint in `ids` to `value`. Refused for
/// the designator and value texts, and whole when a footprint lacks `key`.
pub fn parameter_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    key: &str,
    value: &str,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    if is_text_key(key) {
        return Err(ComponentRefusal::TextParameter(key.to_string()));
    }
    let footprints = placement::editable_footprints(board, ids)?;
    let missing: Vec<String> = footprints
        .iter()
        .filter(|fp| parameter(fp, key).is_none())
        .map(|fp| placement::name(fp))
        .collect();
    if !missing.is_empty() {
        return Err(ComponentRefusal::NoParameter {
            footprints: missing,
            key: key.to_string(),
        });
    }
    let mut commands = Vec::new();
    for fp in footprints {
        let Some(index) = fp.properties.iter().position(|p| p.key == key) else {
            continue;
        };
        let property = &fp.properties[index];
        if property.value != value {
            commands.push(BoardCommand::SetFootprintProperty {
                footprint: fp.uuid,
                index,
                property: PcbProperty {
                    value: value.to_string(),
                    ..property.clone()
                },
            });
        }
    }
    Ok(commands)
}

/// The area of `footprint`'s body in mm²: the rectangle a click selects it
/// by (`pcb_geom::footprint_body`), its courtyard's or else its pads'
/// bounds. A footprint with neither has no area; `None` when the body cannot
/// be built.
pub fn body_area_mm2(footprint: &Footprint) -> Option<f64> {
    match pcb_geom::footprint_body(footprint).ok()? {
        pcb_geom::Shape::Rect { half_w, half_h, .. }
        | pcb_geom::Shape::RoundRect { half_w, half_h, .. } => Some(
            pcb_geom::nm_to_mm(half_w.saturating_mul(2))
                * pcb_geom::nm_to_mm(half_h.saturating_mul(2)),
        ),
        pcb_geom::Shape::Disc { .. } | pcb_geom::Shape::Capsule { .. } => Some(0.0),
        // A body is never built as a polygon; its bounds would stand in.
        body @ pcb_geom::Shape::Polygon { .. } => {
            let bounds = body.aabb();
            Some(pcb_geom::nm_to_mm(bounds.width()) * pcb_geom::nm_to_mm(bounds.height()))
        }
    }
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
