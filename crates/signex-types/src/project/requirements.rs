//! The project's requirements (#2339): targets this project must meet, each
//! one a deterministic check that `check_requirements` runs. Stored as the
//! top-level `requirements` list of the `.snxprj`, beside `rules`, not inside
//! it: requirements are this project's own targets, so they are not part of
//! the rule set's hash, a rule profile or Update from profile. The kinds and
//! their fields are in `docs/CLI.md`, "Requirement checks".

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::violation::Severity;

mod edit;

pub use edit::{MAX_REQUIREMENT_ID_CHARS, RequirementField, RequirementProblem};

/// The `.snxprj` key of the requirements list.
pub const REQUIREMENTS_KEY: &str = "requirements";

/// The lifecycle states a `parts_state` requirement allows when it names
/// none.
pub const DEFAULT_PART_STATES: [&str; 1] = ["released"];

/// The lifecycle state names a `parts_state` requirement may list.
pub const LIFECYCLE_STATES: [&str; 5] =
    ["draft", "in_review", "released", "deprecated", "obsolete"];

/// Every kind's name, as the file writes it, in the order an editor offers
/// them.
pub const REQUIREMENT_KINDS: [&str; 6] = [
    "erc_clean",
    "drc_clean",
    "board_size",
    "parts_state",
    "net_class",
    "bom_cost",
];

/// One requirement of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    /// Unique within the project; a finding names it as `req::<id>`.
    pub id: String,
    /// The severity of the findings this requirement raises.
    #[serde(default = "default_severity")]
    pub severity: Severity,
    /// A requirement turned off is kept in the list and not checked.
    #[serde(default = "default_enabled", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(flatten)]
    pub kind: RequirementKind,
}

fn default_severity() -> Severity {
    Severity::Error
}

fn default_enabled() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn default_part_states() -> Vec<String> {
    DEFAULT_PART_STATES.iter().map(|s| s.to_string()).collect()
}

/// What a requirement checks, with its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequirementKind {
    /// The project's ERC run has no error finding, and no warning finding
    /// unless `allow_warnings`.
    ErcClean {
        #[serde(default, skip_serializing_if = "is_false")]
        allow_warnings: bool,
    },
    /// The DRC run of the project's board has no error finding, and no
    /// warning finding unless `allow_warnings`.
    DrcClean {
        #[serde(default, skip_serializing_if = "is_false")]
        allow_warnings: bool,
    },
    /// The board outline's bounding box is at most this wide and high, in mm.
    BoardSize {
        max_width_mm: f64,
        max_height_mm: f64,
    },
    /// Every part the BOM buys has a library row in one of `states`
    /// (lifecycle state names: `draft`, `in_review`, `released`,
    /// `deprecated`, `obsolete`).
    PartsState {
        #[serde(default = "default_part_states")]
        states: Vec<String>,
    },
    /// The board nets matching the pattern `nets` (`*` and `?`) belong to
    /// `class`, and their class sets at least the given track width and
    /// clearance.
    NetClass {
        nets: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        class: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_trace_width_mm: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_clearance_mm: Option<f64>,
    },
    /// The BOM costs at most `max_unit_cost` per board in `currency` when
    /// `quantity` boards are built, from cached prices only.
    BomCost {
        quantity: u32,
        currency: String,
        max_unit_cost: f64,
    },
}

impl RequirementKind {
    /// The kind's name, as the file writes it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::ErcClean { .. } => "erc_clean",
            Self::DrcClean { .. } => "drc_clean",
            Self::BoardSize { .. } => "board_size",
            Self::PartsState { .. } => "parts_state",
            Self::NetClass { .. } => "net_class",
            Self::BomCost { .. } => "bom_cost",
        }
    }

    /// The kind's own fields, as the file names them.
    pub fn fields(&self) -> &'static [&'static str] {
        match self {
            Self::ErcClean { .. } | Self::DrcClean { .. } => &["allow_warnings"],
            Self::BoardSize { .. } => &["max_width_mm", "max_height_mm"],
            Self::PartsState { .. } => &["states"],
            Self::NetClass { .. } => &["nets", "class", "min_trace_width_mm", "min_clearance_mm"],
            Self::BomCost { .. } => &["quantity", "currency", "max_unit_cost"],
        }
    }
}

/// The fields every requirement has.
const COMMON_FIELDS: [&str; 4] = ["id", "severity", "enabled", "kind"];

/// One entry of the stored list: a requirement, or an entry this build
/// cannot read (an unknown kind, a missing or mistyped field, a key it does
/// not know). Every entry keeps the value it was read from and a save writes
/// that value back, so a hand-written entry keeps its omitted defaults and
/// its number forms; the check reports an unreadable entry as not checked.
#[derive(Debug, Clone, PartialEq)]
pub enum RequirementEntry {
    Known {
        requirement: Requirement,
        raw: Value,
    },
    Unreadable {
        raw: Value,
        reason: String,
    },
}

impl RequirementEntry {
    /// The entry `raw` reads as.
    pub fn read(raw: Value) -> Self {
        match serde_json::from_value::<Requirement>(raw.clone()) {
            Ok(requirement) => match unread_keys(&raw, &requirement) {
                Some(keys) => Self::Unreadable {
                    raw,
                    reason: format!("unknown field {keys}"),
                },
                None => Self::Known { requirement, raw },
            },
            Err(error) => Self::Unreadable {
                raw,
                reason: error.to_string(),
            },
        }
    }

    /// The entry's id, when it has a text one.
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Known { requirement, .. } => Some(&requirement.id),
            Self::Unreadable { raw, .. } => raw.get("id").and_then(Value::as_str),
        }
    }

    /// The requirement, when the entry reads as one.
    pub fn requirement(&self) -> Option<&Requirement> {
        match self {
            Self::Known { requirement, .. } => Some(requirement),
            Self::Unreadable { .. } => None,
        }
    }

    /// The value the entry was read from, as a save writes it.
    pub fn raw(&self) -> &Value {
        match self {
            Self::Known { raw, .. } | Self::Unreadable { raw, .. } => raw,
        }
    }
}

/// The keys of `raw` that are no field of `requirement`'s kind, quoted and
/// comma-separated; `None` when there are none.
fn unread_keys(raw: &Value, requirement: &Requirement) -> Option<String> {
    let Value::Object(given) = raw else {
        return None;
    };
    let known = requirement.kind.fields();
    let unread: Vec<String> = given
        .keys()
        .filter(|key| !COMMON_FIELDS.contains(&key.as_str()) && !known.contains(&key.as_str()))
        .map(|key| format!("`{key}`"))
        .collect();
    (!unread.is_empty()).then(|| unread.join(", "))
}

impl Serialize for RequirementEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RequirementEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::read)
    }
}

/// Read a stored `requirements` value: a list, entry by entry. A value that is
/// not a list never makes the project unopenable; it is kept verbatim for
/// re-save and the check reports it.
pub(crate) fn take_requirements(data: &mut crate::project::ProjectData, raw: Value) {
    match raw {
        Value::Array(entries) => {
            data.requirements = entries.into_iter().map(RequirementEntry::read).collect();
        }
        other => data.unknown.insert(REQUIREMENTS_KEY.to_string(), other),
    }
}

impl crate::project::ProjectData {
    /// The file has a `requirements` value that is not a list: kept verbatim
    /// for re-save, nothing in it is checked.
    pub fn has_unreadable_requirements(&self) -> bool {
        self.unknown.get(REQUIREMENTS_KEY).is_some()
    }
}
