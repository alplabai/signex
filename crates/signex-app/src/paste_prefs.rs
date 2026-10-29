//! The "Designators on paste" preference: what a pasted part is called.

/// What Paste, Smart Paste, Duplicate, rubber-stamp and Paste Array do to the
/// designators of the parts they place. Persisted in `prefs.json` under
/// `paste_designators`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasteDesignators {
    /// The copy keeps the designators it had.
    #[default]
    Keep,
    /// Every pasted part becomes `R?`, `U?`, ... for Annotate to number.
    Reset,
    /// Every pasted part takes the next unused number of its prefix in the
    /// project.
    NextFree,
}

impl PasteDesignators {
    pub const ALL: &'static [PasteDesignators] = &[
        PasteDesignators::Keep,
        PasteDesignators::Reset,
        PasteDesignators::NextFree,
    ];

    /// The `prefs.json` token.
    pub fn token(self) -> &'static str {
        match self {
            PasteDesignators::Keep => "keep",
            PasteDesignators::Reset => "reset",
            PasteDesignators::NextFree => "next_free",
        }
    }

    /// Parse a `prefs.json` token; anything unknown is the default.
    pub fn from_token(token: &str) -> Self {
        Self::ALL
            .iter()
            .copied()
            .find(|mode| token.eq_ignore_ascii_case(mode.token()))
            .unwrap_or_default()
    }
}

impl std::fmt::Display for PasteDesignators {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasteDesignators::Keep => write!(f, "Keep"),
            PasteDesignators::Reset => write!(f, "Reset (R?, U?, ...)"),
            PasteDesignators::NextFree => write!(f, "Next free number"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_round_trips_through_its_token() {
        for mode in PasteDesignators::ALL {
            assert_eq!(PasteDesignators::from_token(mode.token()), *mode);
        }
        assert_eq!(
            PasteDesignators::from_token("bogus"),
            PasteDesignators::Keep
        );
    }
}
