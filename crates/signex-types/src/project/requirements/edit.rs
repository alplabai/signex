//! Editing the requirements list (#2339): add an entry of a kind with
//! starting values, set one field from typed text, turn one off or on and
//! remove one. Every edit works on the entry's stored value and reads it
//! again, so a field nobody touched keeps the form it was written in. A
//! refused edit changes nothing and says why. Rules: `docs/CLI.md`,
//! "Requirement checks".

use serde_json::{Map, Number, Value, json};

use super::{LIFECYCLE_STATES, REQUIREMENT_KINDS, RequirementEntry};
use crate::project::ProjectData;

/// Longest id an editor accepts, in characters.
pub const MAX_REQUIREMENT_ID_CHARS: usize = 64;

/// A field an editor sets, as the file names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequirementField {
    Id,
    Severity,
    AllowWarnings,
    MaxWidthMm,
    MaxHeightMm,
    States,
    /// The variant a `parts_state` checks; cleared: the active variant.
    Variant,
    /// A `parts_state` checks only the parts its variant fits.
    FittedOnly,
    Nets,
    Class,
    MinTraceWidthMm,
    MinClearanceMm,
    Quantity,
    Currency,
    MaxUnitCost,
    /// `rail_budget` (#2943).
    Net,
    Voltage,
    TolerancePercent,
    MaxCurrentA,
    MarginPercent,
    /// `layer_count` (#2943).
    MinLayers,
    MaxLayers,
}

impl RequirementField {
    const ALL: [Self; 22] = [
        Self::Id,
        Self::Severity,
        Self::AllowWarnings,
        Self::MaxWidthMm,
        Self::MaxHeightMm,
        Self::States,
        Self::Variant,
        Self::FittedOnly,
        Self::Nets,
        Self::Class,
        Self::MinTraceWidthMm,
        Self::MinClearanceMm,
        Self::Quantity,
        Self::Currency,
        Self::MaxUnitCost,
        Self::Net,
        Self::Voltage,
        Self::TolerancePercent,
        Self::MaxCurrentA,
        Self::MarginPercent,
        Self::MinLayers,
        Self::MaxLayers,
    ];

    /// The field's key in the file.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Severity => "severity",
            Self::AllowWarnings => "allow_warnings",
            Self::MaxWidthMm => "max_width_mm",
            Self::MaxHeightMm => "max_height_mm",
            Self::States => "states",
            Self::Variant => "variant",
            Self::FittedOnly => "fitted_only",
            Self::Nets => "nets",
            Self::Class => "class",
            Self::MinTraceWidthMm => "min_trace_width_mm",
            Self::MinClearanceMm => "min_clearance_mm",
            Self::Quantity => "quantity",
            Self::Currency => "currency",
            Self::MaxUnitCost => "max_unit_cost",
            Self::Net => "net",
            Self::Voltage => "voltage",
            Self::TolerancePercent => "tolerance_percent",
            Self::MaxCurrentA => "max_current_a",
            Self::MarginPercent => "margin_percent",
            Self::MinLayers => "min_layers",
            Self::MaxLayers => "max_layers",
        }
    }

    /// The field whose key is `key`.
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.key() == key)
    }

    /// The fields a requirement of `kind` has, in the order an editor shows
    /// them (`id` and `severity` are edited apart).
    pub fn of_kind(kind: &str) -> &'static [Self] {
        match kind {
            "erc_clean" | "drc_clean" => &[Self::AllowWarnings],
            "board_size" => &[Self::MaxWidthMm, Self::MaxHeightMm],
            "parts_state" => &[Self::States, Self::Variant, Self::FittedOnly],
            "net_class" => &[
                Self::Nets,
                Self::Class,
                Self::MinTraceWidthMm,
                Self::MinClearanceMm,
            ],
            "bom_cost" => &[Self::Quantity, Self::Currency, Self::MaxUnitCost],
            "rail_budget" => &[
                Self::Net,
                Self::Voltage,
                Self::TolerancePercent,
                Self::MaxCurrentA,
                Self::MarginPercent,
            ],
            "layer_count" => &[Self::MinLayers, Self::MaxLayers],
            _ => &[],
        }
    }

    /// May the field be cleared (left out of the file)?
    pub const fn is_optional(self) -> bool {
        matches!(
            self,
            Self::Class
                | Self::MinTraceWidthMm
                | Self::MinClearanceMm
                | Self::Variant
                | Self::Voltage
                | Self::TolerancePercent
                | Self::MaxCurrentA
                | Self::MarginPercent
                | Self::MinLayers
                | Self::MaxLayers
        )
    }
}

/// Why a requirement edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequirementProblem {
    /// The stored `requirements` value is not a list: it is kept as written
    /// and nothing is edited over it.
    ListUnreadable,
    /// No entry at that place in the list.
    NoEntry(usize),
    /// No requirement has this id.
    UnknownId(String),
    /// More than one entry has this id.
    AmbiguousId(String),
    /// The entry cannot be read, so it can only be removed.
    EntryUnreadable(String),
    UnknownKind(String),
    /// The field is not one of this kind's.
    NotAField {
        kind: String,
        field: String,
    },
    IdEmpty,
    IdTooLong,
    /// The id holds a control character.
    IdInvalid,
    IdTaken(String),
    UnknownSeverity(String),
    /// Not `true` or `false`.
    NotYesNo(String),
    /// Not a positive finite number.
    NotPositive(String),
    /// Not a whole number of at least 1.
    NotACount(String),
    /// The field must have a value.
    Empty(&'static str),
    UnknownState(String),
    /// A `parts_state` names a variant the project does not have.
    UnknownVariant(String),
    /// A `net_class` requirement sets neither a class nor a minimum.
    NetClassAsksNothing,
    /// Not a number other than 0.
    NotAVoltage(String),
    /// Not a number from 0 to below 100.
    NotAPercent(String),
    /// A `layer_count` requirement sets neither a minimum nor a maximum.
    LayerCountAsksNothing,
}

impl std::fmt::Display for RequirementProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ListUnreadable => f.write_str(
                "the project's requirements are not a list; fix the project file first",
            ),
            Self::NoEntry(index) => write!(f, "the project has no requirement #{}", index + 1),
            Self::UnknownId(id) => write!(f, "the project has no requirement `{id}`"),
            Self::AmbiguousId(id) => write!(f, "more than one requirement has the id `{id}`"),
            Self::EntryUnreadable(label) => {
                write!(f, "requirement {label} cannot be read; it can only be removed")
            }
            Self::UnknownKind(kind) => write!(
                f,
                "`{kind}` is no requirement kind; one of {}",
                REQUIREMENT_KINDS.join(", ")
            ),
            Self::NotAField { kind, field } => {
                write!(f, "a `{kind}` requirement has no field `{field}`")
            }
            Self::IdEmpty => f.write_str("a requirement needs an id"),
            Self::IdTooLong => write!(
                f,
                "an id is at most {MAX_REQUIREMENT_ID_CHARS} characters"
            ),
            Self::IdInvalid => f.write_str("an id cannot hold a control character"),
            Self::IdTaken(id) => write!(f, "another requirement has the id `{id}`"),
            Self::UnknownSeverity(text) => {
                write!(f, "`{text}` is no severity; error, warning or info")
            }
            Self::NotYesNo(text) => write!(f, "`{text}` is not true or false"),
            Self::NotPositive(text) => write!(f, "`{text}` is not a number above 0"),
            Self::NotACount(text) => write!(f, "`{text}` is not a whole number of at least 1"),
            Self::Empty(field) => write!(f, "`{field}` needs a value"),
            Self::UnknownState(state) => write!(
                f,
                "`{state}` is no lifecycle state; one of {}",
                LIFECYCLE_STATES.join(", ")
            ),
            Self::UnknownVariant(name) => write!(
                f,
                "the project has no variant `{name}`; Base or one of its variants"
            ),
            Self::NetClassAsksNothing => f.write_str(
                "a net_class requirement needs a class, a minimum track width or a minimum clearance",
            ),
            Self::NotAVoltage(text) => write!(f, "`{text}` is not a voltage: a number other than 0"),
            Self::NotAPercent(text) => {
                write!(f, "`{text}` is not a percentage from 0 to below 100")
            }
            Self::LayerCountAsksNothing => {
                f.write_str("a layer_count requirement needs a minimum or a maximum layer count")
            }
        }
    }
}

impl std::error::Error for RequirementProblem {}

/// The values a new requirement of `kind` starts with, before the id.
fn starting_fields(kind: &str) -> Option<Map<String, Value>> {
    let fields = match kind {
        "erc_clean" | "drc_clean" | "parts_state" => json!({}),
        "board_size" => json!({"max_width_mm": 100, "max_height_mm": 100}),
        "net_class" => json!({"nets": "*", "min_trace_width_mm": 0.15, "min_clearance_mm": 0.15}),
        "bom_cost" => json!({"quantity": 100, "currency": "USD", "max_unit_cost": 10}),
        "rail_budget" => json!({"net": "*"}),
        "layer_count" => json!({"max_layers": 4}),
        _ => return None,
    };
    match fields {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

impl ProjectData {
    /// The place in the list of the entry with id `id` (trimmed).
    pub fn requirement_index(&self, id: &str) -> Result<usize, RequirementProblem> {
        let id = id.trim();
        let mut found = self
            .requirements
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.id().map(str::trim) == Some(id))
            .map(|(index, _)| index);
        match (found.next(), found.next()) {
            (Some(index), None) => Ok(index),
            (Some(_), Some(_)) => Err(RequirementProblem::AmbiguousId(id.to_string())),
            (None, _) => Err(RequirementProblem::UnknownId(id.to_string())),
        }
    }

    /// Add a requirement of `kind` after the others, with the kind's
    /// starting values and `id`, or the kind's name made unique when none is
    /// given. Answers its place in the list.
    pub fn add_requirement(
        &mut self,
        kind: &str,
        id: Option<&str>,
    ) -> Result<usize, RequirementProblem> {
        self.check_requirements_editable()?;
        let mut fields = starting_fields(kind)
            .ok_or_else(|| RequirementProblem::UnknownKind(kind.to_string()))?;
        let id = match id {
            Some(id) => self.check_requirement_id(id, None)?,
            None => self.unused_requirement_id(kind),
        };
        fields.insert("id".into(), Value::String(id));
        fields.insert("kind".into(), Value::String(kind.to_string()));
        let entry = read_known(Value::Object(fields))?;
        self.requirements.push(entry);
        Ok(self.requirements.len() - 1)
    }

    /// Set `field` of the entry at `index` from `text`, as typed. An empty
    /// text clears an optional field.
    pub fn set_requirement_field(
        &mut self,
        index: usize,
        field: RequirementField,
        text: &str,
    ) -> Result<(), RequirementProblem> {
        let (kind, mut raw) = self.editable_entry(index)?;
        let own = RequirementField::of_kind(&kind).contains(&field);
        if !own && !matches!(field, RequirementField::Id | RequirementField::Severity) {
            return Err(RequirementProblem::NotAField {
                kind,
                field: field.key().to_string(),
            });
        }
        let value = match field {
            RequirementField::Id => {
                Some(Value::String(self.check_requirement_id(text, Some(index))?))
            }
            RequirementField::Variant => self.requirement_variant(text)?,
            _ => field_value(field, text)?,
        };
        match value {
            Some(value) => raw.insert(field.key().to_string(), value),
            None => raw.remove(field.key()),
        };
        if kind == "net_class"
            && ["class", "min_trace_width_mm", "min_clearance_mm"]
                .iter()
                .all(|key| raw.get(*key).is_none_or(Value::is_null))
        {
            return Err(RequirementProblem::NetClassAsksNothing);
        }
        if kind == "layer_count"
            && ["min_layers", "max_layers"]
                .iter()
                .all(|key| raw.get(*key).is_none_or(Value::is_null))
        {
            return Err(RequirementProblem::LayerCountAsksNothing);
        }
        self.requirements[index] = read_known(Value::Object(raw))?;
        Ok(())
    }

    /// Turn the entry at `index` on or off; on is left out of the file.
    pub fn set_requirement_enabled(
        &mut self,
        index: usize,
        on: bool,
    ) -> Result<(), RequirementProblem> {
        let (_, mut raw) = self.editable_entry(index)?;
        if on {
            raw.remove("enabled");
        } else {
            raw.insert("enabled".into(), Value::Bool(false));
        }
        self.requirements[index] = read_known(Value::Object(raw))?;
        Ok(())
    }

    /// Remove the entry at `index`, readable or not.
    pub fn remove_requirement(&mut self, index: usize) -> Result<(), RequirementProblem> {
        self.check_requirements_editable()?;
        if index >= self.requirements.len() {
            return Err(RequirementProblem::NoEntry(index));
        }
        self.requirements.remove(index);
        Ok(())
    }

    fn check_requirements_editable(&self) -> Result<(), RequirementProblem> {
        if self.has_unreadable_requirements() {
            return Err(RequirementProblem::ListUnreadable);
        }
        Ok(())
    }

    /// The kind and stored value of the readable entry at `index`.
    fn editable_entry(
        &self,
        index: usize,
    ) -> Result<(String, Map<String, Value>), RequirementProblem> {
        self.check_requirements_editable()?;
        let entry = self
            .requirements
            .get(index)
            .ok_or(RequirementProblem::NoEntry(index))?;
        let requirement = entry.requirement().ok_or_else(|| {
            let label = entry
                .id()
                .map_or_else(|| format!("#{}", index + 1), str::to_string);
            RequirementProblem::EntryUnreadable(label)
        })?;
        match entry.raw() {
            Value::Object(raw) => Ok((requirement.kind.name().to_string(), raw.clone())),
            _ => Err(RequirementProblem::EntryUnreadable(requirement.id.clone())),
        }
    }

    /// `text` as an id no other entry (than the one at `own`) has.
    fn check_requirement_id(
        &self,
        text: &str,
        own: Option<usize>,
    ) -> Result<String, RequirementProblem> {
        let id = text.trim();
        if id.is_empty() {
            return Err(RequirementProblem::IdEmpty);
        }
        if id.chars().count() > MAX_REQUIREMENT_ID_CHARS {
            return Err(RequirementProblem::IdTooLong);
        }
        if id.chars().any(char::is_control) {
            return Err(RequirementProblem::IdInvalid);
        }
        let taken = self
            .requirements
            .iter()
            .enumerate()
            .any(|(index, entry)| Some(index) != own && entry.id().map(str::trim) == Some(id));
        if taken {
            return Err(RequirementProblem::IdTaken(id.to_string()));
        }
        Ok(id.to_string())
    }

    /// The stored `variant` that `text` names: Base, or a variant's own
    /// spelling (names compare without ASCII case, as the BOM does). Empty
    /// clears it: the active variant.
    fn requirement_variant(&self, text: &str) -> Result<Option<Value>, RequirementProblem> {
        let name = text.trim();
        if name.is_empty() {
            return Ok(None);
        }
        if super::super::is_base_variant(name) {
            return Ok(Some(Value::String(super::super::BASE_VARIANT.to_string())));
        }
        self.variant_definitions
            .iter()
            .find(|defined| defined.eq_ignore_ascii_case(name))
            .map(|defined| Some(Value::String(defined.clone())))
            .ok_or_else(|| RequirementProblem::UnknownVariant(name.to_string()))
    }

    /// A variant renamed from `old` to `new`: every `parts_state` that
    /// checks it follows the new name, so the check keeps its target.
    pub(crate) fn rename_requirement_variant(&mut self, old: &str, new: &str) {
        for entry in &mut self.requirements {
            let names_old = matches!(
                entry.requirement().map(|r| &r.kind),
                Some(super::RequirementKind::PartsState { variant: Some(v), .. })
                    if v.trim().eq_ignore_ascii_case(old)
            );
            let Value::Object(raw) = entry.raw() else {
                continue;
            };
            if !names_old {
                continue;
            }
            let mut raw = raw.clone();
            raw.insert("variant".into(), Value::String(new.to_string()));
            if let Ok(renamed) = read_known(Value::Object(raw)) {
                *entry = renamed;
            }
        }
    }

    /// `kind`, or `kind_2`, `kind_3`, ...: the first no entry has.
    fn unused_requirement_id(&self, kind: &str) -> String {
        let taken = |id: &str| {
            self.requirements
                .iter()
                .any(|entry| entry.id().map(str::trim) == Some(id))
        };
        if !taken(kind) {
            return kind.to_string();
        }
        (2..)
            .map(|n| format!("{kind}_{n}"))
            .find(|id| !taken(id))
            .unwrap_or_else(|| kind.to_string())
    }
}

/// `raw` read as an entry; an edit never leaves an unreadable one.
fn read_known(raw: Value) -> Result<RequirementEntry, RequirementProblem> {
    match RequirementEntry::read(raw) {
        entry @ RequirementEntry::Known { .. } => Ok(entry),
        RequirementEntry::Unreadable { reason, .. } => {
            Err(RequirementProblem::EntryUnreadable(reason))
        }
    }
}

/// The stored value `text` gives `field`; `None` clears an optional field.
fn field_value(field: RequirementField, text: &str) -> Result<Option<Value>, RequirementProblem> {
    let typed = text.trim();
    if typed.is_empty() {
        let clears = field.is_optional()
            || matches!(
                field,
                RequirementField::AllowWarnings | RequirementField::FittedOnly
            );
        return if clears {
            Ok(None)
        } else {
            Err(RequirementProblem::Empty(field.key()))
        };
    }
    let value = match field {
        RequirementField::Id => Value::String(typed.to_string()),
        RequirementField::Severity => match typed.to_ascii_lowercase().as_str() {
            token @ ("error" | "warning" | "info") => Value::String(token.to_string()),
            _ => return Err(RequirementProblem::UnknownSeverity(typed.to_string())),
        },
        RequirementField::AllowWarnings => match typed.to_ascii_lowercase().as_str() {
            "true" => Value::Bool(true),
            "false" => return Ok(None),
            _ => return Err(RequirementProblem::NotYesNo(typed.to_string())),
        },
        // On by default, so only off is written.
        RequirementField::FittedOnly => match typed.to_ascii_lowercase().as_str() {
            "true" => return Ok(None),
            "false" => Value::Bool(false),
            _ => return Err(RequirementProblem::NotYesNo(typed.to_string())),
        },
        RequirementField::MaxWidthMm
        | RequirementField::MaxHeightMm
        | RequirementField::MinTraceWidthMm
        | RequirementField::MinClearanceMm
        | RequirementField::MaxUnitCost
        | RequirementField::MaxCurrentA => positive_number(typed)?,
        RequirementField::Quantity | RequirementField::MinLayers | RequirementField::MaxLayers => {
            match typed.parse::<u32>() {
                Ok(count) if count >= 1 => Value::from(count),
                _ => return Err(RequirementProblem::NotACount(typed.to_string())),
            }
        }
        RequirementField::Voltage => voltage(typed)?,
        RequirementField::TolerancePercent | RequirementField::MarginPercent => percent(typed)?,
        RequirementField::States => states(typed)?,
        RequirementField::Nets
        | RequirementField::Class
        | RequirementField::Currency
        | RequirementField::Net
        | RequirementField::Variant => Value::String(typed.to_string()),
    };
    Ok(Some(value))
}

/// A positive finite number, in the form it was typed (`100` stays `100`).
fn positive_number(typed: &str) -> Result<Value, RequirementProblem> {
    let refused = || RequirementProblem::NotPositive(typed.to_string());
    let value: f64 = typed.parse().map_err(|_| refused())?;
    if !value.is_finite() || value <= 0.0 {
        return Err(refused());
    }
    match serde_json::from_str::<Number>(typed) {
        Ok(number) => Ok(Value::Number(number)),
        Err(_) => Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(refused),
    }
}

/// A voltage: a finite number other than 0, in the form it was typed.
fn voltage(typed: &str) -> Result<Value, RequirementProblem> {
    let refused = || RequirementProblem::NotAVoltage(typed.to_string());
    let value: f64 = typed.parse().map_err(|_| refused())?;
    if !value.is_finite() || value == 0.0 {
        return Err(refused());
    }
    typed_number(typed, value).ok_or_else(refused)
}

/// A percentage from 0 to below 100, in the form it was typed.
fn percent(typed: &str) -> Result<Value, RequirementProblem> {
    let refused = || RequirementProblem::NotAPercent(typed.to_string());
    let value: f64 = typed.parse().map_err(|_| refused())?;
    if !crate::project::is_percent(value) {
        return Err(refused());
    }
    typed_number(typed, value).ok_or_else(refused)
}

/// The JSON number `typed` reads as, else `value`'s.
fn typed_number(typed: &str, value: f64) -> Option<Value> {
    match serde_json::from_str::<Number>(typed) {
        Ok(number) => Some(Value::Number(number)),
        Err(_) => Number::from_f64(value).map(Value::Number),
    }
}

/// A comma-separated list of lifecycle states, each named once.
fn states(typed: &str) -> Result<Value, RequirementProblem> {
    let mut list: Vec<String> = Vec::new();
    for state in typed.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if !LIFECYCLE_STATES.contains(&state) {
            return Err(RequirementProblem::UnknownState(state.to_string()));
        }
        if !list.iter().any(|known| known == state) {
            list.push(state.to_string());
        }
    }
    if list.is_empty() {
        return Err(RequirementProblem::Empty("states"));
    }
    Ok(json!(list))
}

#[cfg(test)]
mod tests;
