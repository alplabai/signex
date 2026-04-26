use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable internal identifier — UUIDv7 for time-orderability.
pub type ComponentId = Uuid;

/// Library-internal part number. Unique within a library; user-renameable.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl std::str::FromStr for Version {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (maj, min) = s
            .split_once('.')
            .ok_or_else(|| format!("bad version: {s}"))?;
        let major = maj.parse::<u32>().map_err(|e| e.to_string())?;
        let minor = min.parse::<u32>().map_err(|e| e.to_string())?;
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
}
