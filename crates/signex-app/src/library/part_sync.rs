//! A placed part follows its library row (#1867): the footprint, manufacturer
//! and MPN the row carries now, against what the symbol holds.
//!
//! Pure comparison and command building, no IO. The row is read by the drift
//! worker (`drift.rs`); the edit goes through the engine as part of the
//! "Update from library" undo step.

use std::collections::HashMap;

use signex_engine::Command;
use signex_types::schematic::Symbol;
use uuid::Uuid;

/// Field key the manufacturer is stamped under.
pub const MANUFACTURER_FIELD: &str = "Manufacturer";
/// Field key the MPN is stamped under (the first of `MPN_FIELDS`).
pub const MPN_FIELD: &str = "MPN";

/// The row-owned identity of a part: what a placed symbol carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartIdentity {
    pub footprint: String,
    pub manufacturer: String,
    pub mpn: String,
}

impl PartIdentity {
    /// What the placed `symbol` holds now.
    pub fn of_symbol(symbol: &Symbol) -> Self {
        let field = |key: &str| symbol.fields.get(key).cloned().unwrap_or_default();
        Self {
            footprint: symbol.footprint.clone(),
            manufacturer: field(MANUFACTURER_FIELD),
            mpn: field(MPN_FIELD),
        }
    }

    /// What the row offers, from its bound `footprint` name.
    pub fn of_row(row: &signex_library::ComponentRow, footprint: String) -> Self {
        Self {
            footprint,
            manufacturer: row.primary_mpn.manufacturer.clone(),
            mpn: row.primary_mpn.mpn.clone(),
        }
    }

    /// The values `row` would change on a part holding `self`. A value the
    /// row leaves empty never clears the part's own.
    pub fn changes_to(&self, row: &Self) -> PartChange {
        let differs = |have: &str, want: &str| {
            (!want.trim().is_empty() && have != want).then(|| want.to_owned())
        };
        PartChange {
            footprint: differs(&self.footprint, &row.footprint),
            manufacturer: differs(&self.manufacturer, &row.manufacturer),
            mpn: differs(&self.mpn, &row.mpn),
        }
    }
}

/// The new values for the fields where a part differs from its row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartChange {
    pub footprint: Option<String>,
    pub manufacturer: Option<String>,
    pub mpn: Option<String>,
}

impl PartChange {
    pub fn is_empty(&self) -> bool {
        self.footprint.is_none() && self.manufacturer.is_none() && self.mpn.is_none()
    }
}

/// One placed part to bring in line with its row.
#[derive(Debug, Clone)]
pub struct PartSync {
    pub symbol: Uuid,
    pub reference: String,
    pub have: PartIdentity,
    pub change: PartChange,
}

impl PartSync {
    /// The engine edits that apply the change.
    pub fn commands(&self) -> Vec<Command> {
        let field = |key: &str, value: &Option<String>| {
            value.as_ref().map(|value| Command::SetSymbolField {
                symbol_id: self.symbol,
                key: key.to_owned(),
                value: value.clone(),
            })
        };
        let footprint =
            self.change
                .footprint
                .as_ref()
                .map(|footprint| Command::UpdateSymbolFootprint {
                    symbol_id: self.symbol,
                    footprint: footprint.clone(),
                });
        footprint
            .into_iter()
            .chain(field(MANUFACTURER_FIELD, &self.change.manufacturer))
            .chain(field(MPN_FIELD, &self.change.mpn))
            .collect()
    }

    /// The Messages line: `U1: updated from library (footprint "" -> "SOIC-8")`.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        let mut push = |what: &str, have: &str, new: &Option<String>| {
            if let Some(new) = new {
                parts.push(format!("{what} {have:?} -> {new:?}"));
            }
        };
        push("footprint", &self.have.footprint, &self.change.footprint);
        push(
            "manufacturer",
            &self.have.manufacturer,
            &self.change.manufacturer,
        );
        push("MPN", &self.have.mpn, &self.change.mpn);
        format!(
            "{}: updated from library ({})",
            self.reference,
            parts.join(", ")
        )
    }
}

/// The Manufacturer and MPN a fresh placement stamps (an empty one is left
/// off rather than written blank).
pub fn identity_fields(fields: &[(String, String)]) -> HashMap<String, String> {
    fields
        .iter()
        .filter(|(key, value)| {
            (key == MANUFACTURER_FIELD || key == MPN_FIELD) && !value.trim().is_empty()
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(footprint: &str, manufacturer: &str, mpn: &str) -> PartIdentity {
        PartIdentity {
            footprint: footprint.into(),
            manufacturer: manufacturer.into(),
            mpn: mpn.into(),
        }
    }

    #[test]
    fn a_differing_row_value_is_a_change_and_an_empty_one_is_not() {
        let have = id("", "Acme", "OLD");
        let change = have.changes_to(&id("SOIC-8", "", "NEW"));
        assert_eq!(change.footprint.as_deref(), Some("SOIC-8"));
        assert_eq!(change.mpn.as_deref(), Some("NEW"));
        assert_eq!(change.manufacturer, None, "an empty row value never clears");
        assert!(have.changes_to(&have).is_empty());
    }

    #[test]
    fn commands_cover_only_the_changed_fields() {
        let sync = PartSync {
            symbol: Uuid::nil(),
            reference: "U1".into(),
            have: id("", "", ""),
            change: PartChange {
                footprint: Some("SOIC-8".into()),
                manufacturer: None,
                mpn: Some("X".into()),
            },
        };
        assert_eq!(sync.commands().len(), 2);
        assert_eq!(
            sync.describe(),
            "U1: updated from library (footprint \"\" -> \"SOIC-8\", MPN \"\" -> \"X\")"
        );
    }

    #[test]
    fn placement_stamps_only_non_empty_identity_fields() {
        let fields = vec![
            (MANUFACTURER_FIELD.to_string(), String::new()),
            (MPN_FIELD.to_string(), "X".to_string()),
            ("Tolerance".to_string(), "1%".to_string()),
        ];
        let stamped = identity_fields(&fields);
        assert_eq!(stamped.len(), 1);
        assert_eq!(stamped[MPN_FIELD], "X");
    }
}
