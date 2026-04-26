use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable internal identifier — UUIDv7 for time-orderability.
pub type ComponentId = Uuid;

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

/// Two-segment monotonic version. Minor = compatible, major = breaking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    pub fn bump_minor(self) -> Self {
        Self::new(self.major, self.minor + 1)
    }

    pub fn bump_major(self) -> Self {
        Self::new(self.major + 1, 0)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// M7: typed error for `Version::from_str` so `?` propagation keeps structure
/// instead of stringifying. `MissingDot` distinguishes a malformed input
/// (`"3"`) from a non-numeric segment (`"3.x"`); the latter wraps the
/// underlying `ParseIntError` so callers can dig into `kind()` if needed.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ParseVersionError {
    #[error("version must contain a single dot separator (e.g. \"1.2\")")]
    MissingDot,
    #[error("version segment is not a valid u32: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),
}

impl std::str::FromStr for Version {
    type Err = ParseVersionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (maj, min) = s.split_once('.').ok_or(ParseVersionError::MissingDot)?;
        let major = maj.parse::<u32>()?;
        let minor = min.parse::<u32>()?;
        Ok(Self::new(major, minor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parses_and_displays() {
        let v: Version = "3.7".parse().unwrap();
        assert_eq!(v, Version::new(3, 7));
        assert_eq!(v.to_string(), "3.7");
    }

    #[test]
    fn version_bumps_correctly() {
        assert_eq!(Version::new(1, 2).bump_minor(), Version::new(1, 3));
        assert_eq!(Version::new(1, 2).bump_major(), Version::new(2, 0));
    }

    #[test]
    fn version_orders_by_major_then_minor() {
        assert!(Version::new(1, 9) < Version::new(2, 0));
        assert!(Version::new(2, 0) < Version::new(2, 1));
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
