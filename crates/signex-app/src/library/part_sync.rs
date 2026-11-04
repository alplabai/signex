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
    /// The field the part keeps its MPN under (one of `MPN_FIELDS`); empty
    /// for a row, and for a part with no MPN yet, which gets `MPN_FIELD`.
    pub mpn_key: String,
}

impl PartIdentity {
    /// What the placed `symbol` holds now.
    pub fn of_symbol(symbol: &Symbol) -> Self {
        Self::of_fields(&symbol.footprint, &symbol.fields)
    }

    /// What a part with `footprint` and `fields` holds.
    fn of_fields(footprint: &str, fields: &HashMap<String, String>) -> Self {
        let field = |key: &str| fields.get(key).cloned().unwrap_or_default();
        // ERC and the BOM read an MPN under any of its aliases; so does this.
        let (mpn_key, mpn) = signex_types::schematic::MPN_FIELDS
            .iter()
            .find_map(|key| {
                fields
                    .get(*key)
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| ((*key).to_owned(), v.clone()))
            })
            .unwrap_or_default();
        Self {
            footprint: footprint.to_owned(),
            manufacturer: field(MANUFACTURER_FIELD),
            mpn,
            mpn_key,
        }
    }

    /// What the row offers, from its bound `footprint` name.
    pub fn of_row(row: &signex_library::ComponentRow, footprint: String) -> Self {
        Self {
            footprint,
            manufacturer: row.primary_mpn.manufacturer.clone(),
            mpn: row.primary_mpn.mpn.clone(),
            mpn_key: String::new(),
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
    /// Only the values for fields `have` holds nothing in.
    pub fn only_empty_in(self, have: &PartIdentity) -> Self {
        let blank = |v: &str| v.trim().is_empty();
        Self {
            footprint: self.footprint.filter(|_| blank(&have.footprint)),
            manufacturer: self.manufacturer.filter(|_| blank(&have.manufacturer)),
            mpn: self.mpn.filter(|_| blank(&have.mpn)),
        }
    }

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
            .chain(field(self.mpn_key(), &self.change.mpn))
            .collect()
    }

    /// The field the new MPN goes into: the alias the part already uses.
    fn mpn_key(&self) -> &str {
        if self.have.mpn_key.is_empty() {
            MPN_FIELD
        } else {
            &self.have.mpn_key
        }
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

/// Drop the part syncs of locked parts, and the version stamps of the parts
/// whose sync was dropped, so the lock-exempt library replace cannot restamp
/// a part whose footprint / MPN the lock kept (that would hide the drift). A
/// part is held whole: one locked unit holds back every unit's sync and
/// stamp. A locked part with nothing to sync keeps its stamp, since nothing
/// was held back. Returns the stamps and syncs left and the references held
/// back.
pub fn hold_back_locked(
    engine: &signex_engine::Engine,
    stamps: Vec<(Uuid, String)>,
    part_syncs: Vec<PartSync>,
) -> (Vec<(Uuid, String)>, Vec<PartSync>, Vec<String>) {
    let locked = engine.edit_locked_part_units();
    let (held, kept): (Vec<PartSync>, Vec<PartSync>) = part_syncs
        .into_iter()
        .partition(|p| locked.contains(&p.symbol));
    let held_units: std::collections::HashSet<Uuid> = held
        .iter()
        .flat_map(|p| signex_engine::placed_unit_ids(engine.document(), p.symbol))
        .collect();
    let stamps = stamps
        .into_iter()
        .filter(|(id, _)| !held_units.contains(id))
        .collect();
    let mut references: Vec<String> = held.into_iter().map(|p| p.reference).collect();
    references.sort();
    references.dedup();
    (stamps, kept, references)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(footprint: &str, manufacturer: &str, mpn: &str) -> PartIdentity {
        PartIdentity {
            footprint: footprint.into(),
            manufacturer: manufacturer.into(),
            mpn: mpn.into(),
            mpn_key: String::new(),
        }
    }

    #[test]
    fn an_mpn_kept_under_an_alias_is_read_and_updated_there() {
        let fields = HashMap::from([("Manufacturer Part Number".to_owned(), "OLD".to_owned())]);
        let have = PartIdentity::of_fields("", &fields);
        assert_eq!(have.mpn, "OLD");
        let sync = PartSync {
            symbol: Uuid::nil(),
            reference: "U1".into(),
            change: have.changes_to(&id("", "", "NEW")),
            have,
        };
        let Some(Command::SetSymbolField { key, value, .. }) = sync.commands().pop() else {
            panic!("an MPN edit");
        };
        assert_eq!(
            (key.as_str(), value.as_str()),
            ("Manufacturer Part Number", "NEW")
        );
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

    /// U1A (unit 1, free) and U1B (unit 2, locked), and R1 (free), as the
    /// engine sees them.
    fn two_unit_engine(locked_unit: u32) -> signex_engine::Engine {
        use signex_types::schematic::{Point, SchematicSheet};
        let symbol = |n: u128, reference: &str, unit: u32| Symbol {
            uuid: Uuid::from_u128(n),
            reference: reference.into(),
            lib_id: "L:U".into(),
            position: Point::new(10.0 * n as f64, 10.0),
            unit,
            locked: unit == locked_unit,
            ..Default::default()
        };
        let sheet = SchematicSheet {
            symbols: vec![symbol(1, "U1", 1), symbol(2, "U1", 2), symbol(3, "R1", 3)],
            ..Default::default()
        };
        signex_engine::Engine::new(sheet).expect("engine")
    }

    fn footprint_sync(n: u128, reference: &str) -> PartSync {
        let have = id("", "", "");
        PartSync {
            symbol: Uuid::from_u128(n),
            reference: reference.into(),
            change: have.changes_to(&id("SOIC-8", "", "")),
            have,
        }
    }

    #[test]
    fn one_locked_unit_holds_back_every_unit_sync_and_stamp_of_its_part() {
        let engine = two_unit_engine(2);
        let stamps = vec![
            (Uuid::from_u128(1), "2".to_owned()),
            (Uuid::from_u128(2), "2".to_owned()),
            (Uuid::from_u128(3), "2".to_owned()),
        ];
        let syncs = vec![footprint_sync(1, "U1"), footprint_sync(2, "U1")];

        let (stamps, syncs, held) = hold_back_locked(&engine, stamps, syncs);

        assert!(syncs.is_empty(), "neither unit changes");
        assert_eq!(held, ["U1"]);
        let stamped: Vec<Uuid> = stamps.into_iter().map(|(id, _)| id).collect();
        assert_eq!(
            stamped,
            [Uuid::from_u128(3)],
            "only the free part is stamped"
        );
    }

    #[test]
    fn a_locked_part_with_nothing_to_sync_keeps_its_stamp() {
        let engine = two_unit_engine(2);
        let (stamps, syncs, held) = hold_back_locked(
            &engine,
            vec![(Uuid::from_u128(2), "2".to_owned())],
            Vec::new(),
        );
        assert_eq!((stamps.len(), syncs.len(), held.len()), (1, 0, 0));
    }

    #[test]
    fn an_unlocked_part_is_not_held_back() {
        let engine = two_unit_engine(9);
        let (stamps, syncs, held) = hold_back_locked(
            &engine,
            vec![(Uuid::from_u128(1), "2".to_owned())],
            vec![footprint_sync(1, "U1")],
        );
        assert_eq!((stamps.len(), syncs.len(), held.len()), (1, 1, 0));
    }
}
