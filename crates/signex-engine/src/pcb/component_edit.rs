//! More edits of the Properties panel's Component view: the component
//! settings (#2365), adding and removing a parameter, and one Comment for
//! several components (#2369).
//!
//! Pure, like `component`: each function reads a board and returns the
//! commands the caller runs as one undo step. A locked footprint is refused;
//! a footprint already as asked gets no command.

use std::collections::BTreeSet;

use signex_types::numeric::parse_finite_f64;
use signex_types::pcb::{ComponentAttributes, ComponentType, PcbBoard};
use signex_types::property::PcbProperty;
use uuid::Uuid;

use super::command::BoardCommand;
use super::component::{ComponentRefusal, parameter};
use super::footprint_content::is_text_key;
use super::placement;

/// One component setting, set to an absolute value.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentSetting {
    Type(ComponentType),
    /// Height above the board in mm.
    Height(f64),
    /// 3D body opacity, 0 to 1.
    BodyOpacity(f64),
    LockPrimitives(bool),
    LockStrings(bool),
    PinSwapping(bool),
    PartSwapping(bool),
    ChannelOffset(u32),
    /// The library footprint's description.
    Description(String),
}

impl ComponentSetting {
    /// `attributes` with this setting written.
    fn written(&self, attributes: &ComponentAttributes) -> ComponentAttributes {
        let mut out = attributes.clone();
        match self {
            Self::Type(value) => out.component_type = *value,
            Self::Height(value) => out.height_mm = *value,
            Self::BodyOpacity(value) => out.body_opacity = *value,
            Self::LockPrimitives(value) => out.lock_primitives = *value,
            Self::LockStrings(value) => out.lock_strings = *value,
            Self::PinSwapping(value) => out.pin_swapping = *value,
            Self::PartSwapping(value) => out.part_swapping = *value,
            Self::ChannelOffset(value) => out.channel_offset = *value,
            Self::Description(value) => out.description.clone_from(value),
        }
        out
    }
}

/// A setting typed as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypedSetting {
    Height,
    BodyOpacity,
    ChannelOffset,
    Description,
}

/// Read typed `text` as a value of `setting`: a height in mm (a trailing
/// `mm` allowed), an opacity from 0 to 1 or a percentage, a whole channel
/// offset, or a description as typed.
pub fn parse_setting(setting: TypedSetting, text: &str) -> Result<ComponentSetting, ComponentRefusal> {
    let trimmed = text.trim();
    let not_a_number = || ComponentRefusal::NotANumber(trimmed.to_string());
    match setting {
        TypedSetting::Height => {
            let number = trimmed.strip_suffix("mm").unwrap_or(trimmed).trim();
            let height = parse_finite_f64(number).map_err(|_| not_a_number())?;
            Ok(ComponentSetting::Height(height))
        }
        TypedSetting::BodyOpacity => {
            let opacity = match trimmed.strip_suffix('%') {
                Some(percent) => parse_finite_f64(percent.trim()).map(|p| p / 100.0),
                None => parse_finite_f64(trimmed),
            }
            .map_err(|_| not_a_number())?;
            Ok(ComponentSetting::BodyOpacity(opacity))
        }
        TypedSetting::ChannelOffset => trimmed
            .parse::<u32>()
            .map(ComponentSetting::ChannelOffset)
            .map_err(|_| ComponentRefusal::NotAWholeNumber(trimmed.to_string())),
        TypedSetting::Description => Ok(ComponentSetting::Description(text.to_string())),
    }
}

/// Write `setting` on every footprint in `ids`. Refused whole for a height
/// below zero or an opacity outside 0 to 1.
pub fn setting_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    setting: &ComponentSetting,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    let footprints = placement::editable_footprints(board, ids)?;
    let mut commands = Vec::new();
    for fp in footprints {
        let component = setting.written(&fp.component);
        if let Some(field) = component.invalid_field() {
            return Err(ComponentRefusal::OutOfRange(field));
        }
        if component != *fp.component {
            commands.push(BoardCommand::SetFootprintComponent {
                footprint: fp.uuid,
                component,
            });
        }
    }
    Ok(commands)
}

/// Add the parameter `key` = `value` to every footprint in `ids`, after its
/// other properties. A new parameter is not drawn on the board. Refused for
/// an empty name, the designator and value texts, and whole when a footprint
/// already has `key`.
pub fn add_parameter_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    key: &str,
    value: &str,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    let key = key.trim();
    if key.is_empty() {
        return Err(ComponentRefusal::EmptyName);
    }
    if is_text_key(key) {
        return Err(ComponentRefusal::TextParameter(key.to_string()));
    }
    let footprints = placement::editable_footprints(board, ids)?;
    let taken: Vec<String> = footprints
        .iter()
        .filter(|fp| parameter(fp, key).is_some())
        .map(|fp| placement::name(fp))
        .collect();
    if !taken.is_empty() {
        return Err(ComponentRefusal::ParameterTaken {
            footprints: taken,
            key: key.to_string(),
        });
    }
    Ok(footprints
        .into_iter()
        .map(|fp| BoardCommand::InsertFootprintProperty {
            footprint: fp.uuid,
            index: fp.properties.len(),
            property: PcbProperty {
                key: key.to_string(),
                value: value.to_string(),
                hidden: true,
                ..PcbProperty::default()
            },
        })
        .collect())
}

/// Remove the parameter `key` from every footprint in `ids`: each one's
/// first property of that key. Refused for the designator and value texts,
/// and whole when a footprint lacks `key`.
pub fn remove_parameter_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    key: &str,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    if is_text_key(key) {
        return Err(ComponentRefusal::TextParameter(key.to_string()));
    }
    let footprints = placement::editable_footprints(board, ids)?;
    let mut commands = Vec::new();
    let mut missing = Vec::new();
    for fp in footprints {
        match fp.properties.iter().position(|p| p.key == key) {
            Some(index) => commands.push(BoardCommand::RemoveFootprintProperty {
                footprint: fp.uuid,
                index,
            }),
            None => missing.push(placement::name(fp)),
        }
    }
    if !missing.is_empty() {
        return Err(ComponentRefusal::NoParameter {
            footprints: missing,
            key: key.to_string(),
        });
    }
    Ok(commands)
}

/// Set the Comment (the value) of every footprint in `ids` to `text`, as
/// typed; footprints that differ are all overwritten.
pub fn comment_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    text: &str,
) -> Result<Vec<BoardCommand>, ComponentRefusal> {
    let footprints = placement::editable_footprints(board, ids)?;
    Ok(footprints
        .into_iter()
        .filter(|fp| fp.value != text)
        .map(|fp| BoardCommand::SetFootprintText {
            footprint: fp.uuid,
            reference: fp.reference.clone(),
            value: text.to_string(),
        })
        .collect())
}

#[cfg(test)]
#[path = "component_edit_tests.rs"]
mod tests;
