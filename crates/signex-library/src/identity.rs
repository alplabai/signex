use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable row identifier — UUIDv7 for time-orderability. Newtype wraps
/// `Uuid` so the type system can distinguish a `RowId` from a generic UUID
/// used elsewhere (library_id, primitive uuid, etc).
///
/// Per `v0.9-refactor-2-plan.md` §6 step 1.10, this replaces the previous
/// `ComponentId = Uuid` type alias from the v0.9-original layout.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RowId(pub Uuid);

impl RowId {
    /// Construct a new time-ordered RowId.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wrap an existing UUID — used when reading a row back from disk.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Borrow the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for RowId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for RowId {
    fn from(u: Uuid) -> Self {
        Self(u)
    }
}

impl From<RowId> for Uuid {
    fn from(r: RowId) -> Self {
        r.0
    }
}

impl std::fmt::Display for RowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::str::FromStr for RowId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Library-internal part number. Unique within a library; user-renameable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InternalPn(pub String);

impl InternalPn {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InternalPn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `InternalPn::from_str` is infallible — every string is a valid PN — but
/// the `parse()` ergonomics require a `FromStr` impl so the test fixtures
/// in `v0.9-refactor-2-plan.md` (`"R0805_10k".parse().unwrap()`) work
/// without a special-case helper.
impl std::str::FromStr for InternalPn {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// Manufacturer part number — moves between revisions when vendor reissues.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Mpn(pub String);

impl Mpn {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Component class — picks the parameter template ("resistor", "opamp", …).
///
/// Open string per `v0.9-library-refactor-plan.md` §4.1: users may add custom
/// classes; templates resolve dynamically through [`crate::TemplateRegistry`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentClass(pub String);

impl ComponentClass {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Default class — applied when the user hasn't picked one yet.
    pub fn generic() -> Self {
        Self("generic".into())
    }
}

impl std::fmt::Display for ComponentClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Default for ComponentClass {
    fn default() -> Self {
        Self::generic()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_id_round_trip() {
        let r = RowId::new();
        let s = serde_json::to_string(&r).unwrap();
        let back: RowId = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn row_id_display_and_fromstr_match() {
        let r = RowId::new();
        let s = r.to_string();
        let back: RowId = s.parse().unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn row_id_fromstr_rejects_garbage() {
        let bad: Result<RowId, _> = "not-a-uuid".parse();
        assert!(bad.is_err());
    }

    #[test]
    fn internal_pn_round_trip() {
        let pn = InternalPn::new("R0805_10k");
        let s = serde_json::to_string(&pn).unwrap();
        assert_eq!(s, "\"R0805_10k\"");
        let back: InternalPn = serde_json::from_str(&s).unwrap();
        assert_eq!(pn, back);
    }

    #[test]
    fn internal_pn_parses_via_fromstr() {
        let pn: InternalPn = "R0805_10k".parse().unwrap();
        assert_eq!(pn.as_str(), "R0805_10k");
    }

    #[test]
    fn component_class_round_trip_and_default_is_generic() {
        let c = ComponentClass::new("opamp");
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(s, "\"opamp\"");
        let back: ComponentClass = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
        assert_eq!(ComponentClass::default(), ComponentClass::generic());
        assert_eq!(ComponentClass::generic().as_str(), "generic");
    }
}
