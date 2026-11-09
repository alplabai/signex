//! The seam between a library part and the board (#663, #1312): which
//! library footprint a board footprint was built from, how a part's pins
//! land on its pads, and why a part could not be given a footprint.

use std::collections::BTreeMap;
use std::fmt;

use uuid::Uuid;

use crate::pcb::Footprint;

/// The library footprint a board footprint was built from: the library, the
/// footprint within it and the footprint version used. Saved in the optional
/// `lib_ref` footprint column as `<library uuid>:<footprint uuid>@<version>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FootprintSourceRef {
    pub library: Uuid,
    pub footprint: Uuid,
    pub version: String,
}

impl fmt::Display for FootprintSourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}@{}", self.library, self.footprint, self.version)
    }
}

impl FootprintSourceRef {
    /// Read `<library uuid>:<footprint uuid>@<version>`. `None` unless the
    /// text is exactly what [`Display`](fmt::Display) writes for the result:
    /// hyphenated lower-case uuids and a version that is not empty.
    pub fn parse(text: &str) -> Option<Self> {
        let (ids, version) = text.split_once('@')?;
        let (library, footprint) = ids.split_once(':')?;
        let parsed = Self {
            library: Uuid::parse_str(library).ok()?,
            footprint: Uuid::parse_str(footprint).ok()?,
            version: version.to_string(),
        };
        (!parsed.version.is_empty() && parsed.to_string() == text).then_some(parsed)
    }
}

/// Which pad each symbol pin lands on. Empty is the 1:1 map: pin `n` lands
/// on pad `n`. Otherwise a listed pin lands on its pad and every other pin
/// on the pad of its own number.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinPadMap {
    pads: BTreeMap<String, String>,
}

impl PinPadMap {
    /// A map from `(pin, pad)` pairs; a later pair for the same pin wins.
    pub fn from_pairs<P: Into<String>, Q: Into<String>>(
        pairs: impl IntoIterator<Item = (P, Q)>,
    ) -> Self {
        Self {
            pads: pairs
                .into_iter()
                .map(|(pin, pad)| (pin.into(), pad.into()))
                .collect(),
        }
    }

    /// The pad `pin` lands on.
    pub fn pad_for<'a>(&'a self, pin: &'a str) -> &'a str {
        self.pads.get(pin).map_or(pin, String::as_str)
    }

    /// Map `pin` to `pad`, replacing what it mapped to before.
    pub fn set(&mut self, pin: impl Into<String>, pad: impl Into<String>) {
        self.pads.insert(pin.into(), pad.into());
    }

    pub fn is_empty(&self) -> bool {
        self.pads.is_empty()
    }
}

/// A part's footprint made ready for the board: the board footprint (top-side
/// form, at the origin, `library_ref` set), where it came from and the pin
/// map to wire it with.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFootprint {
    pub source: FootprintSourceRef,
    pub footprint: Footprint,
    pub pin_map: PinPadMap,
}

/// Why a part could not be given a board footprint. Nothing is guessed: a
/// footprint that cannot be carried over exactly is refused whole.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FootprintResolveError {
    #[error("the part binds no footprint")]
    NoFootprint,
    #[error("footprint {footprint} of library {library} is not one the part offers")]
    NotOffered { library: Uuid, footprint: Uuid },
    #[error(
        "footprint {footprint} of library {library} (version {version:?}) is not in the library"
    )]
    NotFound {
        library: Uuid,
        footprint: Uuid,
        version: String,
    },
    #[error("the part pins footprint version {wanted:?}, the library holds {found:?}")]
    VersionMismatch { wanted: String, found: String },
    #[error("{what} {value:?} has no board equivalent")]
    Unconvertible { what: &'static str, value: String },
}
