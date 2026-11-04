//! A placed part follows its library row (#1867): the footprint, manufacturer
//! and MPN the row carries now, against what the symbol holds, decided per
//! field as a three-way merge with the value stamp (#2004 item 7).
//!
//! Pure comparison and command building, no IO. The row is read by the drift
//! worker (`drift.rs`); the edit goes through the engine as part of the
//! "Update from library" undo step.

use std::collections::HashMap;

use signex_engine::Command;
use signex_types::schematic::{
    RowStamp, StampField, StampValues, Symbol, Theirs, Verdict, base_of, resolve,
};
use uuid::Uuid;

/// Field key the manufacturer is stamped under.
pub const MANUFACTURER_FIELD: &str = "Manufacturer";
/// Field key the MPN is stamped under (the first of `MPN_FIELDS`).
pub const MPN_FIELD: &str = "MPN";

/// How the user answered a conflict in the Library Updates modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Choice {
    /// The part keeps its own value; the row's is acknowledged.
    #[default]
    KeepMine,
    /// The part takes the row's value.
    UseLibrary,
}

/// The answers to a part's conflicts, per field. A conflict with no answer
/// stays open.
pub type Choices = HashMap<StampField, Choice>;

/// The row-owned identity of a part: what a placed symbol carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartIdentity {
    pub footprint: String,
    pub manufacturer: String,
    pub mpn: String,
    /// The field the part keeps its MPN under (one of `MPN_FIELDS`); empty
    /// for a part with no MPN yet, which gets `MPN_FIELD`.
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

    fn get(&self, field: StampField) -> &str {
        match field {
            StampField::Footprint => &self.footprint,
            StampField::Manufacturer => &self.manufacturer,
            StampField::Mpn => &self.mpn,
        }
    }
}

/// What a row offers now. The footprint is `None` when it could not be read:
/// that says nothing about the row, in particular not "no footprint".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowOffer {
    pub footprint: Option<String>,
    pub manufacturer: String,
    pub mpn: String,
}

impl RowOffer {
    /// What `row` offers, from its bound footprint's name (`Err`: unreadable;
    /// an empty name: the row binds none).
    pub fn of_row(row: &signex_library::ComponentRow, footprint: Result<String, String>) -> Self {
        Self {
            footprint: footprint.ok(),
            manufacturer: row.primary_mpn.manufacturer.clone(),
            mpn: row.primary_mpn.mpn.clone(),
        }
    }

    fn theirs(&self, field: StampField) -> Theirs<'_> {
        match field {
            StampField::Footprint => self
                .footprint
                .as_deref()
                .map_or(Theirs::Unreadable, Theirs::Value),
            StampField::Manufacturer => Theirs::Value(&self.manufacturer),
            StampField::Mpn => Theirs::Value(&self.mpn),
        }
    }
}

/// The new values for the fields an update rewrites.
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

    fn set(&mut self, field: StampField, value: String) {
        match field {
            StampField::Footprint => self.footprint = Some(value),
            StampField::Manufacturer => self.manufacturer = Some(value),
            StampField::Mpn => self.mpn = Some(value),
        }
    }
}

/// One field of a part against its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEval {
    pub field: StampField,
    pub verdict: Verdict,
    /// What the part last agreed on; `None` when unknown.
    pub base: Option<String>,
}

/// A conflict the user may answer: the part and the row both moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictView {
    pub field: StampField,
    pub mine: String,
    pub library: String,
}

/// A placed part decided against its row, field by field.
#[derive(Debug, Clone)]
pub struct PartEval {
    pub symbol: Uuid,
    pub reference: String,
    pub have: PartIdentity,
    pub fields: Vec<FieldEval>,
}

/// What an update does with an evaluated part.
#[derive(Debug, Clone, Default)]
pub struct Settled {
    /// The edits, when there are any to make or conflicts to report.
    pub sync: Option<PartSync>,
    /// The bases the part holds afterwards: written with the version.
    pub values: StampValues,
}

impl PartEval {
    /// `stamp` is the part's valid stamp, if it has one; `version_unmoved`
    /// says its pinned version still equals the row's, which gives a part
    /// with no stamp a base (the row as it is now).
    pub fn of_symbol(
        symbol: &Symbol,
        stamp: Option<&RowStamp>,
        version_unmoved: bool,
        offer: &RowOffer,
    ) -> Self {
        let have = PartIdentity::of_symbol(symbol);
        Self::new(
            symbol.uuid,
            &symbol.reference,
            have,
            stamp,
            version_unmoved,
            offer,
        )
    }

    pub fn new(
        symbol: Uuid,
        reference: &str,
        have: PartIdentity,
        stamp: Option<&RowStamp>,
        version_unmoved: bool,
        offer: &RowOffer,
    ) -> Self {
        let fields = StampField::ALL
            .iter()
            .map(|&field| {
                let theirs = offer.theirs(field);
                let ours = have.get(field);
                let base = base_of(stamp, field, version_unmoved, theirs);
                // A part with no stamp whose field is empty is filled, as
                // before the stamp existed, whatever the version says.
                let base = if stamp.is_none() && ours.trim().is_empty() {
                    None
                } else {
                    base
                };
                FieldEval {
                    field,
                    verdict: resolve(base, ours, theirs),
                    base: base.map(str::to_owned),
                }
            })
            .collect();
        Self {
            symbol,
            reference: reference.to_owned(),
            have,
            fields,
        }
    }

    /// Does an update have something to apply or ask about?
    pub fn is_flagged(&self) -> bool {
        self.fields.iter().any(|f| f.verdict.is_flagged())
    }

    /// The conflicts the user may answer.
    pub fn conflicts(&self) -> Vec<ConflictView> {
        self.fields
            .iter()
            .filter_map(|f| match &f.verdict {
                Verdict::Conflict(theirs) => Some(ConflictView {
                    field: f.field,
                    mine: self.have.get(f.field).to_owned(),
                    library: theirs.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    /// Decide what an update does. `apply` is false for the silent restamp,
    /// which edits nothing: a follow stays pending and keeps its base. A
    /// conflict is settled only by an answer in `choices`; with none the
    /// part keeps its value and a line says so.
    pub fn settle(&self, apply: bool, choices: &Choices) -> Settled {
        let mut change = PartChange::default();
        let mut kept = Vec::new();
        let mut values = StampValues::default();
        for f in &self.fields {
            let mut settled = false;
            match &f.verdict {
                Verdict::Follows(theirs) if apply => {
                    change.set(f.field, theirs.clone());
                    settled = true;
                }
                Verdict::Conflict(theirs) if apply => match choices.get(&f.field) {
                    Some(Choice::UseLibrary) => {
                        change.set(f.field, theirs.clone());
                        settled = true;
                    }
                    Some(Choice::KeepMine) => settled = true,
                    None => kept.push(format!(
                        "{}: kept your {} {:?}; library now {:?}",
                        self.reference,
                        f.field.label(),
                        self.have.get(f.field),
                        theirs
                    )),
                },
                _ => {}
            }
            values.set(f.field, f.verdict.base_after(f.base.as_deref(), settled));
        }
        let sync = (!change.is_empty() || !kept.is_empty()).then(|| PartSync {
            symbol: self.symbol,
            reference: self.reference.clone(),
            have: self.have.clone(),
            change,
            kept,
        });
        Settled { sync, values }
    }
}

/// One placed part to bring in line with its row.
#[derive(Debug, Clone)]
pub struct PartSync {
    pub symbol: Uuid,
    pub reference: String,
    pub have: PartIdentity,
    pub change: PartChange,
    /// One line per conflict the update left as it was.
    pub kept: Vec<String>,
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

    /// This update checked against the part as it is now (#2070): the worker
    /// decided against `have`, captured when the user clicked. A field the
    /// user edited since is not rewritten; the edit stays and a line says so.
    pub fn revalidated(mut self, live: &PartIdentity) -> Self {
        let edited = |field: StampField| {
            self.have.get(field) != live.get(field)
                || (field == StampField::Mpn && self.have.mpn_key != live.mpn_key)
        };
        let skip: Vec<StampField> = StampField::ALL
            .iter()
            .copied()
            .filter(|&field| edited(field))
            .collect();
        for field in skip {
            let slot = match field {
                StampField::Footprint => &mut self.change.footprint,
                StampField::Manufacturer => &mut self.change.manufacturer,
                StampField::Mpn => &mut self.change.mpn,
            };
            if let Some(theirs) = slot.take() {
                self.kept.push(format!(
                    "{}: kept your {} {:?}; it changed while the update ran (library {:?})",
                    self.reference,
                    field.label(),
                    live.get(field),
                    theirs
                ));
            }
        }
        self.have = live.clone();
        self
    }

    /// The field the new MPN goes into: the alias the part already uses.
    fn mpn_key(&self) -> &str {
        if self.have.mpn_key.is_empty() {
            MPN_FIELD
        } else {
            &self.have.mpn_key
        }
    }

    /// The Messages lines: `U1: updated from library (footprint "" ->
    /// "SOIC-8")`, then one per conflict left as it was.
    pub fn lines(&self) -> Vec<String> {
        let updated = (!self.change.is_empty()).then(|| self.describe());
        updated
            .into_iter()
            .chain(self.kept.iter().cloned())
            .collect()
    }

    fn describe(&self) -> String {
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

    fn part(footprint: &str, manufacturer: &str, mpn: &str) -> Symbol {
        let mut fields = HashMap::new();
        for (key, value) in [(MANUFACTURER_FIELD, manufacturer), (MPN_FIELD, mpn)] {
            if !value.is_empty() {
                fields.insert(key.to_owned(), value.to_owned());
            }
        }
        Symbol {
            uuid: Uuid::nil(),
            reference: "U1".into(),
            footprint: footprint.into(),
            fields,
            ..Default::default()
        }
    }

    fn offer(footprint: Option<&str>, manufacturer: &str, mpn: &str) -> RowOffer {
        RowOffer {
            footprint: footprint.map(str::to_owned),
            manufacturer: manufacturer.into(),
            mpn: mpn.into(),
        }
    }

    fn stamp(mpn: &str) -> RowStamp {
        RowStamp {
            footprint: Some(String::new()),
            manufacturer: Some(String::new()),
            mpn: Some(mpn.into()),
            ..RowStamp::new(Uuid::nil())
        }
    }

    #[test]
    fn an_mpn_kept_under_an_alias_is_read_and_updated_there() {
        let fields = HashMap::from([("Manufacturer Part Number".to_owned(), "OLD".to_owned())]);
        let have = PartIdentity::of_fields("", &fields);
        assert_eq!(have.mpn, "OLD");
        let mut symbol = part("", "", "");
        symbol.fields = fields;
        let eval = PartEval::of_symbol(
            &symbol,
            Some(&stamp("OLD")),
            false,
            &offer(Some(""), "", "NEW"),
        );
        let sync = eval.settle(true, &Choices::new()).sync.expect("a follow");
        let Some(Command::SetSymbolField { key, value, .. }) = sync.commands().pop() else {
            panic!("an MPN edit");
        };
        assert_eq!(
            (key.as_str(), value.as_str()),
            ("Manufacturer Part Number", "NEW")
        );
    }

    #[test]
    fn a_legacy_part_is_filled_where_empty_and_overrides_stay_when_the_version_is_unmoved() {
        // No stamp, version unmoved: the base is the row as it is.
        let eval = PartEval::of_symbol(
            &part("", "Acme", "OLD"),
            None,
            true,
            &offer(Some("SOIC-8"), "Other", "NEW"),
        );
        let change = eval
            .settle(true, &Choices::new())
            .sync
            .expect("the empty footprint")
            .change;
        assert_eq!(
            change.footprint.as_deref(),
            Some("SOIC-8"),
            "empty is filled"
        );
        assert_eq!(change.manufacturer, None, "an override stays");
        assert_eq!(change.mpn, None, "an override stays");
        assert!(eval.conflicts().is_empty());
    }

    #[test]
    fn commands_cover_only_the_changed_fields_and_say_what_they_did() {
        let eval = PartEval::of_symbol(
            &part("", "", ""),
            None,
            false,
            &offer(Some("SOIC-8"), "", "X"),
        );
        let sync = eval.settle(true, &Choices::new()).sync.expect("two fills");
        assert_eq!(sync.commands().len(), 2);
        assert_eq!(
            sync.lines(),
            ["U1: updated from library (footprint \"\" -> \"SOIC-8\", MPN \"\" -> \"X\")"]
        );
    }

    #[test]
    fn an_update_keeps_a_conflict_and_says_so_but_a_choice_settles_it() {
        let eval = PartEval::of_symbol(
            &part("", "", "MINE"),
            Some(&stamp("OLD")),
            false,
            &offer(Some(""), "", "LIB"),
        );
        assert_eq!(eval.conflicts().len(), 1);
        let kept = eval.settle(true, &Choices::new());
        let sync = kept.sync.expect("a kept line");
        assert!(sync.change.is_empty());
        assert_eq!(
            sync.lines(),
            ["U1: kept your MPN \"MINE\"; library now \"LIB\""]
        );
        assert_eq!(kept.values.mpn.as_deref(), Some("OLD"), "the base stays");

        let mine = eval.settle(true, &Choices::from([(StampField::Mpn, Choice::KeepMine)]));
        assert!(mine.sync.is_none(), "nothing to change");
        assert_eq!(mine.values.mpn.as_deref(), Some("LIB"), "acknowledged");

        let library = eval.settle(
            true,
            &Choices::from([(StampField::Mpn, Choice::UseLibrary)]),
        );
        assert_eq!(
            library.sync.expect("an edit").change.mpn.as_deref(),
            Some("LIB")
        );
        assert_eq!(library.values.mpn.as_deref(), Some("LIB"));
    }

    #[test]
    fn the_silent_restamp_never_advances_the_base_of_a_pending_follow() {
        let eval = PartEval::of_symbol(
            &part("", "", "OLD"),
            Some(&stamp("OLD")),
            false,
            &offer(Some(""), "", "NEW"),
        );
        let silent = eval.settle(false, &Choices::new());
        assert!(silent.sync.is_none());
        assert_eq!(silent.values.mpn.as_deref(), Some("OLD"));
    }

    #[test]
    fn a_field_edited_since_the_capture_is_kept_and_named() {
        let eval = PartEval::of_symbol(
            &part("", "Acme", "OLD"),
            Some(&stamp("OLD")),
            false,
            &offer(Some("SOIC-8"), "Acme", "NEW"),
        );
        let sync = eval
            .settle(true, &Choices::new())
            .sync
            .expect("two follows");
        let live = PartIdentity::of_symbol(&part("", "Acme", "EDITED"));
        let sync = sync.revalidated(&live);
        assert_eq!(sync.change.mpn, None, "the edited MPN is not rewritten");
        assert_eq!(sync.change.footprint.as_deref(), Some("SOIC-8"));
        assert_eq!(
            sync.kept,
            ["U1: kept your MPN \"EDITED\"; it changed while the update ran (library \"NEW\")"]
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
