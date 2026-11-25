//! The project's power rails (#2943): the nominal voltage of a power net and
//! its tolerance, by net name, in the top-level `rails` object of the
//! `.snxprj`: `{"+3V3": {"voltage": 3.3, "tolerance_percent": 5}}`. The
//! current a part draws from a rail, or can supply to it, is a parameter of
//! the part (`signex_req::rails`). Rules and shapes: `docs/CLI.md`, "Rail
//! budgets".
//!
//! The object is kept as its stored JSON value, so a save writes back every
//! rail nobody edited in the form it was written, and a project that sets no
//! rail writes no `rails` key. An older build keeps the key as one it does
//! not know. A value this build cannot read is kept as written and not
//! edited over.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use super::ProjectData;

/// The `.snxprj` key of the rails.
pub const RAILS_KEY: &str = "rails";

/// One rail: its nominal voltage and, when set, its tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RailSpec {
    /// Volts; negative for a negative rail, never 0.
    pub voltage: f64,
    /// Plus or minus this percentage of `voltage`, from 0 to below 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_percent: Option<f64>,
}

impl RailSpec {
    /// The lowest and highest voltage the rail may have, low first; the
    /// nominal voltage twice without a tolerance.
    pub fn band(&self) -> (f64, f64) {
        let spread = self.voltage.abs() * self.tolerance_percent.unwrap_or(0.0) / 100.0;
        (self.voltage - spread, self.voltage + spread)
    }

    fn is_usable(&self) -> bool {
        is_voltage(self.voltage) && self.tolerance_percent.is_none_or(is_percent)
    }
}

/// A voltage a rail may have: finite and not 0.
fn is_voltage(volts: f64) -> bool {
    volts.is_finite() && volts != 0.0
}

/// A percentage a tolerance or a margin may be: from 0 to below 100.
pub fn is_percent(value: f64) -> bool {
    value.is_finite() && (0.0..100.0).contains(&value)
}

/// What one `project_set_rail` asks, as typed. `None` leaves a value as it
/// is; an empty `tolerance_percent` clears it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RailEdit {
    pub voltage: Option<String>,
    pub tolerance_percent: Option<String>,
    /// Take the rail out of the list.
    pub remove: bool,
}

/// Why a rail edit was refused. A refused edit changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RailProblem {
    /// The stored `rails` value cannot be read: it is kept as written.
    Unreadable(String),
    NetEmpty,
    /// The net name holds a control character.
    NetInvalid,
    /// Not a number other than 0.
    NotAVoltage(String),
    /// Not a number from 0 to below 100.
    NotAPercent(String),
    /// A rail the project does not list needs a voltage.
    NoVoltage(String),
    /// The project lists no rail of this name.
    NoRail(String),
    /// Nothing to set: no voltage, no tolerance and no remove.
    NothingToSet,
    /// `remove` with a value to set.
    RemoveWithValues,
}

impl std::fmt::Display for RailProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(why) => write!(
                f,
                "the project's rails cannot be read ({why}); fix the project file first"
            ),
            Self::NetEmpty => f.write_str("a rail needs a net name"),
            Self::NetInvalid => f.write_str("a net name cannot hold a control character"),
            Self::NotAVoltage(text) => {
                write!(f, "`{text}` is not a voltage: a number other than 0")
            }
            Self::NotAPercent(text) => {
                write!(f, "`{text}` is not a percentage from 0 to below 100")
            }
            Self::NoVoltage(net) => write!(f, "the rail {net} is new and needs a `voltage`"),
            Self::NoRail(net) => write!(f, "the project lists no rail {net}"),
            Self::NothingToSet => {
                f.write_str("give a `voltage`, a `tolerance_percent` or `remove`")
            }
            Self::RemoveWithValues => {
                f.write_str("`remove` takes the rail out; it sets no voltage or tolerance")
            }
        }
    }
}

impl std::error::Error for RailProblem {}

impl ProjectData {
    /// The project's rails by net name; empty when it sets none, the reason
    /// when the stored value cannot be read.
    pub fn rails(&self) -> Result<BTreeMap<String, RailSpec>, String> {
        let Some(raw) = self.unknown.get(RAILS_KEY) else {
            return Ok(BTreeMap::new());
        };
        let rails: BTreeMap<String, RailSpec> =
            serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
        for (net, spec) in &rails {
            if net.trim().is_empty() || net.trim() != net.as_str() {
                return Err(format!("the rail name `{net}` is empty or padded"));
            }
            if !spec.is_usable() {
                return Err(format!("the rail {net} has no usable voltage or tolerance"));
            }
        }
        Ok(rails)
    }

    /// The `rails` value as stored, for an undo step.
    pub fn rails_value(&self) -> Option<Value> {
        self.unknown.get(RAILS_KEY).cloned()
    }

    /// Put back a `rails` value [`Self::rails_value`] took.
    pub fn restore_rails_value(&mut self, value: Option<Value>) {
        match value {
            Some(value) => self.unknown.insert(RAILS_KEY.to_string(), value),
            None => self.unknown.remove(RAILS_KEY),
        }
    }

    /// Set, change or remove the rail `net` as `edit` asks. Every other rail
    /// keeps its stored form; the last rail removed removes the key.
    pub fn set_rail(&mut self, net: &str, edit: &RailEdit) -> Result<(), RailProblem> {
        self.rails().map_err(RailProblem::Unreadable)?;
        let net = net.trim();
        if net.is_empty() {
            return Err(RailProblem::NetEmpty);
        }
        if net.chars().any(char::is_control) {
            return Err(RailProblem::NetInvalid);
        }
        let mut all = match self.unknown.get(RAILS_KEY) {
            Some(Value::Object(map)) => map.clone(),
            _ => Map::new(),
        };
        let asks_values = edit.voltage.is_some() || edit.tolerance_percent.is_some();
        if edit.remove {
            if asks_values {
                return Err(RailProblem::RemoveWithValues);
            }
            if all.remove(net).is_none() {
                return Err(RailProblem::NoRail(net.to_string()));
            }
        } else {
            if !asks_values {
                return Err(RailProblem::NothingToSet);
            }
            let mut rail = match all.get(net) {
                Some(Value::Object(rail)) => rail.clone(),
                _ => Map::new(),
            };
            if let Some(text) = &edit.voltage {
                rail.insert("voltage".into(), voltage_value(text)?);
            }
            if !rail.contains_key("voltage") {
                return Err(RailProblem::NoVoltage(net.to_string()));
            }
            match edit.tolerance_percent.as_deref().map(str::trim) {
                Some("") => {
                    rail.remove("tolerance_percent");
                }
                Some(text) => {
                    rail.insert("tolerance_percent".into(), percent_value(text)?);
                }
                None => {}
            }
            all.insert(net.to_string(), Value::Object(rail));
        }
        let value = (!all.is_empty()).then_some(Value::Object(all));
        self.restore_rails_value(value);
        Ok(())
    }
}

/// `typed` as a voltage, in the form it was typed (`5` stays `5`).
fn voltage_value(typed: &str) -> Result<Value, RailProblem> {
    let typed = typed.trim();
    let refused = || RailProblem::NotAVoltage(typed.to_string());
    let value: f64 = typed.parse().map_err(|_| refused())?;
    if !is_voltage(value) {
        return Err(refused());
    }
    typed_number(typed, value).ok_or_else(refused)
}

/// `typed` as a percentage from 0 to below 100, in the form it was typed.
fn percent_value(typed: &str) -> Result<Value, RailProblem> {
    let typed = typed.trim();
    let refused = || RailProblem::NotAPercent(typed.to_string());
    let value: f64 = typed.parse().map_err(|_| refused())?;
    if !is_percent(value) {
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

#[cfg(test)]
mod tests;
