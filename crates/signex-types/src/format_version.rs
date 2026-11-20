//! One format-version policy for every native `.snx*` file (#2207).
//!
//! A token is `<kind>/<major>[.<minor>]` (`snxsch/1.1`); the symbol and
//! simulation files spell the major with a `v` (`snxsym/v3`). Each format
//! declares one [`FormatVersion`]: the token it writes, the tokens it reads,
//! and what it does with a newer minor and with a file that has no token.
//! The policy table per format is in `docs/FORMAT_VERSIONS.md`.

/// What a build does with a file one minor newer than any it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewerMinor {
    /// It opens for viewing and every save of it is refused
    /// ([`FormatVersion::refuse_save`]).
    ReadOnly,
    /// It is refused on load, like a newer major.
    Refused,
}

/// What a build does with a file that carries no format token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingToken {
    /// It is read as this token.
    ReadAs(&'static str),
    /// It is refused on load.
    Refused,
}

/// The version policy of one file format.
#[derive(Debug, Clone, Copy)]
pub struct FormatVersion {
    /// The newest token this build reads; every save writes it.
    pub current: &'static str,
    /// Every token this build reads in full, `current` included.
    pub readable: &'static [&'static str],
    pub newer_minor: NewerMinor,
    pub missing: MissingToken,
}

/// Why a token was refused. The caller adds the file name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatVersionError {
    #[error("the file has no format token; this Signex reads {readable}")]
    Missing { readable: String },
    #[error(
        "format {found:?} was written by a newer Signex; this build reads {readable}. \
         Open it in a newer Signex"
    )]
    Newer { found: String, readable: String },
    #[error(
        "format {found:?} was written by a newer Signex; saving it with this build \
         ({current}) would lose data. Edit it in the newer version"
    )]
    NewerReadOnly {
        found: String,
        current: &'static str,
    },
    #[error("format {found:?} is not one this Signex reads ({readable})")]
    Unsupported { found: String, readable: String },
}

impl FormatVersionError {
    /// The token the file carried, `None` when it had none.
    pub fn found(&self) -> Option<&str> {
        match self {
            Self::Missing { .. } => None,
            Self::Newer { found, .. }
            | Self::NewerReadOnly { found, .. }
            | Self::Unsupported { found, .. } => Some(found),
        }
    }
}

impl FormatVersion {
    /// Check the token a file carries and return the token it is read as.
    /// A newer minor passes only under [`NewerMinor::ReadOnly`]; the caller
    /// then marks the document with [`Self::is_newer_minor`].
    pub fn read(&self, found: Option<&str>) -> Result<String, FormatVersionError> {
        let Some(token) = found else {
            return match self.missing {
                MissingToken::ReadAs(token) => Ok(token.to_string()),
                MissingToken::Refused => Err(FormatVersionError::Missing {
                    readable: self.readable_list(),
                }),
            };
        };
        let (Some(version), Some(current)) =
            (self.version_of(token), self.version_of(self.current))
        else {
            return Err(self.unsupported(token));
        };
        if self
            .readable
            .iter()
            .any(|r| self.version_of(r) == Some(version))
        {
            return Ok(token.to_string());
        }
        if version > current {
            if version.0 == current.0 && self.newer_minor == NewerMinor::ReadOnly {
                return Ok(token.to_string());
            }
            return Err(FormatVersionError::Newer {
                found: token.to_string(),
                readable: self.readable_list(),
            });
        }
        Err(self.unsupported(token))
    }

    /// True when `token` is the current major with a newer minor.
    pub fn is_newer_minor(&self, token: &str) -> bool {
        match (self.version_of(token), self.version_of(self.current)) {
            (Some((major, minor)), Some((current_major, current_minor))) => {
                major == current_major && minor > current_minor
            }
            _ => false,
        }
    }

    /// Refuse to save a document read from a newer minor.
    pub fn refuse_save(&self, source: &str) -> Result<(), FormatVersionError> {
        if self.is_newer_minor(source) {
            return Err(FormatVersionError::NewerReadOnly {
                found: source.to_string(),
                current: self.current,
            });
        }
        Ok(())
    }

    /// `<kind>/` or `<kind>/v`: the current token without its version.
    fn prefix(&self) -> &'static str {
        self.current
            .trim_end_matches(|c: char| c.is_ascii_digit() || c == '.')
    }

    /// `(major, minor)` of `token` when it has this format's prefix.
    fn version_of(&self, token: &str) -> Option<(u32, u32)> {
        let rest = token.strip_prefix(self.prefix())?;
        let (major, minor) = rest.split_once('.').unwrap_or((rest, "0"));
        Some((number(major)?, number(minor)?))
    }

    fn readable_list(&self) -> String {
        let quoted: Vec<String> = self.readable.iter().map(|t| format!("{t:?}")).collect();
        quoted.join(", ")
    }

    fn unsupported(&self, token: &str) -> FormatVersionError {
        FormatVersionError::Unsupported {
            found: token.to_string(),
            readable: self.readable_list(),
        }
    }
}

/// ASCII digits only, no leading zero except "0" (no `+`, no `007`).
fn number(s: &str) -> Option<u32> {
    let plain =
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'));
    if plain { s.parse().ok() } else { None }
}

#[cfg(test)]
#[path = "format_version_tests.rs"]
mod tests;
