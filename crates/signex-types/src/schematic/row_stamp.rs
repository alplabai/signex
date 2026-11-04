//! What a placed part agreed on with its library row (#2004 item 7).
//!
//! A part stamps the row's value of footprint, Manufacturer and MPN at the
//! last point it agreed with the row (the base). Against the part's own value
//! (ours) and the row's value now (theirs) that makes a three-way merge per
//! field: [`resolve`] is the one decision, shared by the drift flag, the
//! Library Updates modal and "Update from library". The rationale and the
//! table of cases are in `docs/architecture/snxsch-library-link.md`,
//! "Value stamp".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::format::UnknownFields;

/// A field a part takes from its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StampField {
    Footprint,
    Manufacturer,
    Mpn,
}

impl StampField {
    pub const ALL: [Self; 3] = [Self::Footprint, Self::Manufacturer, Self::Mpn];

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Footprint => "footprint",
            Self::Manufacturer => "Manufacturer",
            Self::Mpn => "MPN",
        }
    }
}

/// The row values a part last agreed with, `[extras.symbols.<uuid>.library_stamp]`.
///
/// A value that is `None` is unknown; `Some("")` is "the row offered
/// nothing". Fields are declared in key order so an older build that keeps
/// the table as unknown keys re-saves the same bytes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RowStamp {
    /// The resolved NAME of the bound footprint, not its reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// The row's canonical MPN, never the alias key a part keeps it under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mpn: Option<String>,
    /// The row the values came from. A stamp whose row is not the part's
    /// current row describes nothing.
    pub row_id: Uuid,
    /// Keys a later build adds.
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl RowStamp {
    pub fn new(row_id: Uuid) -> Self {
        Self {
            footprint: None,
            manufacturer: None,
            mpn: None,
            row_id,
            unknown: UnknownFields::default(),
        }
    }

    pub fn get(&self, field: StampField) -> Option<&str> {
        match field {
            StampField::Footprint => self.footprint.as_deref(),
            StampField::Manufacturer => self.manufacturer.as_deref(),
            StampField::Mpn => self.mpn.as_deref(),
        }
    }

    /// Set one value; `values` that are `None` leave the stamp as it is.
    pub fn apply(&mut self, values: &StampValues) {
        for field in StampField::ALL {
            if let Some(value) = values.get(field) {
                *self.slot(field) = Some(value.to_owned());
            }
        }
    }

    fn slot(&mut self, field: StampField) -> &mut Option<String> {
        match field {
            StampField::Footprint => &mut self.footprint,
            StampField::Manufacturer => &mut self.manufacturer,
            StampField::Mpn => &mut self.mpn,
        }
    }
}

/// A partial update of a [`RowStamp`]: `Some` sets the value, `None` leaves
/// it alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StampValues {
    pub footprint: Option<String>,
    pub manufacturer: Option<String>,
    pub mpn: Option<String>,
}

impl StampValues {
    pub fn get(&self, field: StampField) -> Option<&str> {
        match field {
            StampField::Footprint => self.footprint.as_deref(),
            StampField::Manufacturer => self.manufacturer.as_deref(),
            StampField::Mpn => self.mpn.as_deref(),
        }
    }

    pub fn set(&mut self, field: StampField, value: Option<String>) {
        match field {
            StampField::Footprint => self.footprint = value,
            StampField::Manufacturer => self.manufacturer = value,
            StampField::Mpn => self.mpn = value,
        }
    }

    pub fn is_empty(&self) -> bool {
        StampField::ALL.iter().all(|f| self.get(*f).is_none())
    }
}

/// The version a part is pinned to and the values it last agreed on, written
/// together so one cannot move without the other.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkStamp {
    pub version: String,
    pub values: StampValues,
}

impl LinkStamp {
    /// A version with no value changes.
    pub fn version(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            values: StampValues::default(),
        }
    }
}

impl super::Symbol {
    /// The stamp that describes this part's current row; a stamp of another
    /// row (or a part with no row) is no stamp.
    pub fn valid_stamp(&self) -> Option<&RowStamp> {
        self.library_stamp
            .as_ref()
            .filter(|stamp| Some(stamp.row_id) == self.row_id)
    }

    /// Record `link` on a part: the pinned version and the stamp values.
    /// Returns whether anything changed. A part with no row keeps no stamp.
    pub fn apply_link_stamp(&mut self, link: &LinkStamp) -> bool {
        let mut changed = self.library_version != link.version;
        self.library_version.clone_from(&link.version);
        let Some(row_id) = self.row_id else {
            return changed;
        };
        // A part with a row but no valid stamp gets one even when no value is
        // proven: it switches the legacy "version unmoved" rule off, and the
        // absent keys read as unknown (so a conflict is not cleared unasked).
        if link.values.is_empty() && self.valid_stamp().is_some() {
            return changed;
        }
        let before = self.library_stamp.clone();
        let mut stamp = self
            .valid_stamp()
            .cloned()
            .unwrap_or_else(|| RowStamp::new(row_id));
        stamp.apply(&link.values);
        changed |= before.as_ref() != Some(&stamp);
        self.library_stamp = Some(stamp);
        changed
    }
}

/// What the row offers for one field now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theirs<'a> {
    /// The row's value could not be read (a footprint library that is not
    /// open): it says nothing, in particular not "no footprint".
    Unreadable,
    /// The value; empty when the row offers nothing.
    Value(&'a str),
}

/// What one field comes to. Values are the row's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The row's value is unreadable: no flag, no change.
    Unknown,
    /// The row offers nothing and the part has a value: it is never cleared.
    RowOffersNothing,
    /// The part holds the row's value.
    InSync(String),
    /// The part still holds what it last agreed on and the row moved: it
    /// follows. The flag is up and an update applies the row's value.
    Follows(String),
    /// The part differs and the row has not moved: the part's own value.
    Override,
    /// The part and the row both moved, to different values.
    Conflict(String),
}

impl Verdict {
    /// Does the part hold something an update would change or ask about?
    pub fn is_flagged(&self) -> bool {
        matches!(self, Self::Follows(_) | Self::Conflict(_))
    }

    /// The value an update takes from the row, when it applies the update.
    pub fn follows(&self) -> Option<&str> {
        match self {
            Self::Follows(theirs) => Some(theirs),
            _ => None,
        }
    }

    /// The base after a stamping event. `settled` says the flagged field was
    /// dealt with: a follow applied, or a conflict answered either way. A
    /// field that is not settled keeps `current`, so it stays flagged.
    pub fn base_after(&self, current: Option<&str>, settled: bool) -> Option<String> {
        match self {
            Self::InSync(theirs) => Some(theirs.clone()),
            Self::Follows(theirs) | Self::Conflict(theirs) if settled => Some(theirs.clone()),
            _ => current.map(str::to_owned),
        }
    }
}

fn blank_as_empty(value: &str) -> &str {
    if value.trim().is_empty() { "" } else { value }
}

/// The three-way decision for one field. `base` is what the part last agreed
/// with, `None` when unknown.
pub fn resolve(base: Option<&str>, ours: &str, theirs: Theirs<'_>) -> Verdict {
    let Theirs::Value(theirs) = theirs else {
        return Verdict::Unknown;
    };
    let (ours, theirs) = (blank_as_empty(ours), blank_as_empty(theirs));
    if ours == theirs {
        return Verdict::InSync(theirs.to_owned());
    }
    if theirs.is_empty() {
        return Verdict::RowOffersNothing;
    }
    let follows = || Verdict::Follows(theirs.to_owned());
    match base.map(blank_as_empty) {
        Some(base) if ours == base => follows(),
        Some(base) if theirs == base => Verdict::Override,
        Some(_) => Verdict::Conflict(theirs.to_owned()),
        None if ours.is_empty() => follows(),
        None => Verdict::Conflict(theirs.to_owned()),
    }
}

/// The base of one field: the stamp's value when the part has a valid stamp
/// (an absent key is unknown, never inferred), else, for a part that predates
/// the stamp, the row's own value when its version has not moved since the
/// part was pinned to it.
pub fn base_of<'a>(
    stamp: Option<&'a RowStamp>,
    field: StampField,
    version_unmoved: bool,
    theirs: Theirs<'a>,
) -> Option<&'a str> {
    match (stamp, theirs) {
        (Some(stamp), _) => stamp.get(field),
        (None, Theirs::Value(value)) if version_unmoved => Some(value),
        (None, _) => None,
    }
}
