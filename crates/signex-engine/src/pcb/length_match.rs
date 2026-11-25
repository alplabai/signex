//! Length matching (#2797, mockup row 107): the edits of a board's
//! length-matching groups and the report of each member against its
//! group's band. The groups live in the board options, so an edit is the
//! options it leaves, landed by the caller as one
//! [`super::BoardCommand::SetBoardOptions`] undo step. Lengths are the
//! routed lengths `pcb_geom::net_routed_length_mm` reads, the same the
//! Track panel and the board reports show.

use pcb_geom::net_routed_length_mm;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_length::{LengthGroup, LengthTopology};
use signex_types::pcb_options::BoardOptions;
use uuid::Uuid;

/// Why a group edit changes nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LengthGroupRefusal {
    #[error("a length-matching group needs a name")]
    NoName,
    #[error("the board has no length-matching group called `{0}`")]
    NoSuchGroup(String),
    #[error("the board already has a length-matching group called `{0}`")]
    NameTaken(String),
    #[error("a new length-matching group needs a target and a tolerance")]
    NewGroupNeedsBand,
    #[error("the target must be a length above zero")]
    TargetNotPositive,
    #[error("the tolerance must be a length of zero or more")]
    ToleranceNegative,
    #[error("the board has no net called `{0}`")]
    UnknownNet(String),
}

/// One edit of the group called `name`; a group of that name is made when
/// the board has none. A field left `None` keeps its value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupEdit {
    pub name: String,
    pub rename: Option<String>,
    pub target_mm: Option<f64>,
    pub tolerance_mm: Option<f64>,
    pub topology: Option<LengthTopology>,
    /// The whole member list, by net name. A net that sits in another
    /// group moves here, in the same step.
    pub members: Option<Vec<String>>,
}

/// The options with `edit` applied to `board`'s groups. `new_id` names a
/// group made by this edit.
pub fn set_group(
    board: &PcbBoard,
    edit: &GroupEdit,
    new_id: impl FnOnce() -> Uuid,
) -> Result<BoardOptions, LengthGroupRefusal> {
    let name = edit.name.trim();
    if name.is_empty() {
        return Err(LengthGroupRefusal::NoName);
    }
    let mut options = board.options.clone();
    let groups = &mut options.length_groups;
    let at = match groups.iter().position(|g| g.name == name) {
        Some(at) => at,
        None => {
            let (Some(target_mm), Some(tolerance_mm)) = (edit.target_mm, edit.tolerance_mm) else {
                return Err(LengthGroupRefusal::NewGroupNeedsBand);
            };
            groups.push(LengthGroup {
                uuid: new_id(),
                name: name.to_string(),
                target_mm,
                tolerance_mm,
                topology: LengthTopology::default(),
                members: Vec::new(),
            });
            groups.len() - 1
        }
    };
    if let Some(rename) = edit.rename.as_deref().map(str::trim) {
        if rename.is_empty() {
            return Err(LengthGroupRefusal::NoName);
        }
        if rename != name && groups.iter().any(|g| g.name == rename) {
            return Err(LengthGroupRefusal::NameTaken(rename.to_string()));
        }
        groups[at].name = rename.to_string();
    }
    if let Some(target) = edit.target_mm {
        if !target.is_finite() || target <= 0.0 {
            return Err(LengthGroupRefusal::TargetNotPositive);
        }
        groups[at].target_mm = target;
    }
    if let Some(tolerance) = edit.tolerance_mm {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(LengthGroupRefusal::ToleranceNegative);
        }
        groups[at].tolerance_mm = tolerance;
    }
    if let Some(topology) = edit.topology {
        groups[at].topology = topology;
    }
    if let Some(members) = &edit.members {
        let members = member_list(board, members)?;
        for (i, group) in groups.iter_mut().enumerate() {
            if i != at {
                group.members.retain(|m| !members.contains(m));
            }
        }
        groups[at].members = members;
    }
    Ok(options)
}

/// `wanted` trimmed, without empty names and repeats, each a net of the
/// board.
fn member_list(board: &PcbBoard, wanted: &[String]) -> Result<Vec<String>, LengthGroupRefusal> {
    let mut members: Vec<String> = Vec::new();
    for name in wanted.iter().map(|m| m.trim()).filter(|m| !m.is_empty()) {
        if net_number(board, name).is_none() {
            return Err(LengthGroupRefusal::UnknownNet(name.to_string()));
        }
        if !members.iter().any(|m| m == name) {
            members.push(name.to_string());
        }
    }
    Ok(members)
}

/// The options without the group called `name`. Its nets stay on the
/// board; only the group goes.
pub fn remove_group(board: &PcbBoard, name: &str) -> Result<BoardOptions, LengthGroupRefusal> {
    let mut options = board.options.clone();
    let before = options.length_groups.len();
    options.length_groups.retain(|g| g.name != name.trim());
    if options.length_groups.len() == before {
        return Err(LengthGroupRefusal::NoSuchGroup(name.trim().to_string()));
    }
    Ok(options)
}

/// Where a member stands against its group's band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberStatus {
    InBand,
    /// Longer than the band: a reroute must shorten it.
    Over,
    /// Shorter than the band: tuning can lengthen it.
    Short,
    /// The net has no routed copper yet.
    Unrouted,
    /// The board has no net of that name.
    Missing,
}

impl MemberStatus {
    /// The word the JSON answers use.
    pub const fn token(self) -> &'static str {
        match self {
            MemberStatus::InBand => "in_band",
            MemberStatus::Over => "over",
            MemberStatus::Short => "short",
            MemberStatus::Unrouted => "unrouted",
            MemberStatus::Missing => "missing",
        }
    }

    pub const fn is_out_of_band(self) -> bool {
        matches!(self, MemberStatus::Over | MemberStatus::Short)
    }
}

/// One member's routed length against its group's target.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberReport {
    pub net: String,
    /// `None` for a missing net or a length the geometry cannot hold.
    pub length_mm: Option<f64>,
    /// Length minus target, mm; `None` with no length.
    pub delta_mm: Option<f64>,
    pub status: MemberStatus,
}

/// One group with its members' lengths.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupReport {
    pub group: LengthGroup,
    pub members: Vec<MemberReport>,
}

impl GroupReport {
    /// Members longer or shorter than the band.
    pub fn out_of_band(&self) -> usize {
        self.members
            .iter()
            .filter(|m| m.status.is_out_of_band())
            .count()
    }

    /// Members with no routed copper yet.
    pub fn unrouted(&self) -> usize {
        self.members
            .iter()
            .filter(|m| m.status == MemberStatus::Unrouted)
            .count()
    }
}

/// Every group of `board` with its members' lengths, in group order.
pub fn report(board: &PcbBoard) -> Vec<GroupReport> {
    board
        .options
        .length_groups
        .iter()
        .map(|group| GroupReport {
            members: group
                .members
                .iter()
                .map(|net| member_report(board, group, net))
                .collect(),
            group: group.clone(),
        })
        .collect()
}

fn member_report(board: &PcbBoard, group: &LengthGroup, net: &str) -> MemberReport {
    let length = net_number(board, net).map(|number| net_routed_length_mm(board, number));
    let (length_mm, status) = match length {
        None => (None, MemberStatus::Missing),
        Some(None) => (None, MemberStatus::Unrouted),
        Some(Some(mm)) if mm <= 0.0 => (Some(mm), MemberStatus::Unrouted),
        Some(Some(mm)) if !group.is_out_of_band(mm) => (Some(mm), MemberStatus::InBand),
        Some(Some(mm)) if mm > group.target_mm => (Some(mm), MemberStatus::Over),
        Some(Some(mm)) => (Some(mm), MemberStatus::Short),
    };
    MemberReport {
        net: net.to_string(),
        delta_mm: length_mm
            .filter(|_| status != MemberStatus::Unrouted)
            .map(|mm| mm - group.target_mm),
        length_mm,
        status,
    }
}

/// The number of the net called `name`; `None` for no such net or net 0.
fn net_number(board: &PcbBoard, name: &str) -> Option<u32> {
    board
        .nets
        .iter()
        .find(|n| n.name == name && n.number != 0)
        .map(|n| n.number)
}

#[cfg(test)]
#[path = "length_match_tests.rs"]
mod tests;
