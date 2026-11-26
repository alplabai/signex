//! Typed block ids: `NOTE-012`, `CALC-2.1`, `MEAS-009`, `ISS-004`,
//! `DEC-002` (mockup row 198). The prefix names the block's kind; the
//! serial is one number, or dotted numbers for a block numbered within its
//! section. Reviews, the Annotations panel and the CLI cite blocks by it.

use std::cmp::Ordering;
use std::fmt;

use crate::annotation::AnnotationKind;

impl AnnotationKind {
    /// The prefix of this kind's block ids: `NOTE`, `CALC`, `MEAS`, `ISS`,
    /// `DEC`.
    pub fn id_prefix(self) -> &'static str {
        match self {
            AnnotationKind::Note => "NOTE",
            AnnotationKind::Calculation => "CALC",
            AnnotationKind::Measurement => "MEAS",
            AnnotationKind::Issue => "ISS",
            AnnotationKind::Decision => "DEC",
        }
    }

    /// The Typst function a block of this kind is written with:
    /// `#note(...)`, `#calculation(...)`, `#measurement(...)`, `#issue(...)`,
    /// `#decision(...)`. It is the kind's token.
    pub fn function(self) -> &'static str {
        self.token()
    }

    /// The kind whose block function is `name`.
    pub fn from_function(name: &str) -> Option<Self> {
        Self::parse(name)
    }
}

/// The digits a new flat id is padded to: `NOTE-001`.
pub const SERIAL_WIDTH: usize = 3;

/// The id of one annotation block. Kept as written, so `MEAS-009` stays
/// `MEAS-009` on save; compares by kind, then by the serial's numbers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId {
    kind: AnnotationKind,
    serial: String,
}

impl BlockId {
    /// The id `text` names, of whatever kind its prefix says. The prefix is
    /// upper case and exact; the serial is ASCII digits, optionally in
    /// dot-separated groups (`2.1`).
    pub fn parse(text: &str) -> Option<Self> {
        let (prefix, serial) = text.split_once('-')?;
        let kind = AnnotationKind::ALL
            .into_iter()
            .find(|kind| kind.id_prefix() == prefix)?;
        is_serial(serial).then(|| Self {
            kind,
            serial: serial.to_string(),
        })
    }

    /// The id `text` names when it is an id of `kind`; a `DEC-` id on a
    /// note is `None`.
    pub fn parse_for(kind: AnnotationKind, text: &str) -> Option<Self> {
        Self::parse(text).filter(|id| id.kind == kind)
    }

    /// The kind the prefix names.
    pub fn kind(&self) -> AnnotationKind {
        self.kind
    }

    /// The part after the dash, as written: `012`, `2.1`.
    pub fn serial(&self) -> &str {
        &self.serial
    }

    /// The serial's numbers: `[12]` for `012`, `[2, 1]` for `2.1`.
    pub fn numbers(&self) -> Vec<u64> {
        self.serial
            .split('.')
            .map(|group| group.parse().unwrap_or(u64::MAX))
            .collect()
    }

    /// The next free id of `kind` among `taken`, for the insert buttons.
    /// With no `section` it is one past the highest flat serial, padded to
    /// [`SERIAL_WIDTH`] digits (`MEAS-010` after `MEAS-009`). With a
    /// section (`[2]`) it is the next number inside it (`CALC-2.2` after
    /// `CALC-2.1`). Ids of other kinds and other sections are ignored.
    pub fn next_free<'a>(
        kind: AnnotationKind,
        section: &[u64],
        taken: impl IntoIterator<Item = &'a BlockId>,
    ) -> Self {
        let highest = taken
            .into_iter()
            .filter(|id| id.kind == kind)
            .map(BlockId::numbers)
            .filter(|numbers| numbers.len() == section.len() + 1 && numbers.starts_with(section))
            .filter_map(|numbers| numbers.last().copied())
            .max()
            .unwrap_or(0);
        let next = highest.saturating_add(1);
        let serial = if section.is_empty() {
            format!("{next:0width$}", width = SERIAL_WIDTH)
        } else {
            let mut groups: Vec<String> = section.iter().map(u64::to_string).collect();
            groups.push(next.to_string());
            groups.join(".")
        };
        Self { kind, serial }
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.kind.id_prefix(), self.serial)
    }
}

/// Written as its text (`"DEC-002"`) in a JSON result.
impl serde::Serialize for BlockId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl PartialOrd for BlockId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BlockId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind
            .cmp(&other.kind)
            .then_with(|| self.numbers().cmp(&other.numbers()))
            .then_with(|| self.serial.cmp(&other.serial))
    }
}

/// Digits in one or more dot-separated groups, none empty.
fn is_serial(text: &str) -> bool {
    !text.is_empty()
        && text
            .split('.')
            .all(|group| !group.is_empty() && group.bytes().all(|b| b.is_ascii_digit()))
}
