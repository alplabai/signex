//! `[[board_options.length_groups]]` of a `.snxpcb` (#2797): the board's
//! length-matching groups (`pcb_length`). Written only when the board has
//! one, and then the board writes `snxpcb/1.12` (`format::snxpcb_token`),
//! so a build that does not know them opens it read-only instead of
//! dropping them on save. Lengths are integer nm like every other on-disk
//! length. See `docs/PCB_LENGTH_MATCHING.md`, "File format".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::FormatError;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb_length::{LengthGroup, LengthTopology};

const BLOCK: &str = "board_options.length_groups";

/// One group as stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::format) struct LengthGroupRaw {
    uuid: Uuid,
    name: String,
    target: i64,
    tolerance: i64,
    topology: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    members: Vec<String>,
}

fn refused(row: usize, field: &str, message: impl Into<String>) -> FormatError {
    FormatError::TsvFieldParse {
        block: BLOCK.to_string(),
        row,
        field: field.to_string(),
        message: message.into(),
    }
}

/// The groups of the file; a group that cannot be read refuses the file,
/// naming its row.
pub(in crate::format) fn read(raw: Vec<LengthGroupRaw>) -> Result<Vec<LengthGroup>, FormatError> {
    raw.into_iter()
        .enumerate()
        .map(|(row, g)| read_group(row, g))
        .collect()
}

fn read_group(row: usize, g: LengthGroupRaw) -> Result<LengthGroup, FormatError> {
    if g.name.trim().is_empty() {
        return Err(refused(row, "name", "a group needs a name"));
    }
    if g.target <= 0 {
        return Err(refused(row, "target", "the target must be above zero"));
    }
    if g.tolerance < 0 {
        return Err(refused(
            row,
            "tolerance",
            "the tolerance cannot be negative",
        ));
    }
    let topology = LengthTopology::from_token(&g.topology).ok_or_else(|| {
        refused(
            row,
            "topology",
            format!("{:?} is not star, chain, fly_by or t", g.topology),
        )
    })?;
    Ok(LengthGroup {
        uuid: g.uuid,
        name: g.name,
        target_mm: nm_to_mm(g.target),
        tolerance_mm: nm_to_mm(g.tolerance),
        topology,
        members: g.members,
    })
}

/// The groups as stored; a length that is not finite refuses the save.
pub(in crate::format) fn write(groups: &[LengthGroup]) -> Result<Vec<LengthGroupRaw>, FormatError> {
    groups
        .iter()
        .enumerate()
        .map(|(row, g)| {
            write_group(g).map_err(|e| FormatError::NonFiniteField {
                block: BLOCK.to_string(),
                row,
                field: e.0.to_string(),
            })
        })
        .collect()
}

fn write_group(g: &LengthGroup) -> Result<LengthGroupRaw, NonFiniteField> {
    Ok(LengthGroupRaw {
        uuid: g.uuid,
        name: g.name.clone(),
        target: mm_to_nm(g.target_mm, "target")?,
        tolerance: mm_to_nm(g.tolerance_mm, "tolerance")?,
        topology: g.topology.token().to_string(),
        members: g.members.clone(),
    })
}
