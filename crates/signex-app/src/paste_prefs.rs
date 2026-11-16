//! The "Designators on paste" preference: what a pasted part is called.

/// What Paste, Smart Paste, Duplicate, rubber-stamp and Paste Array do to the
/// designators of the parts they place. Persisted in `prefs.json` under
/// `paste_designators`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasteDesignators {
    /// The copy keeps its designators unless the project already uses one,
    /// and then takes the next free number of its prefix; `R?` stays `R?`.
    #[default]
    KeepIfFree,
    /// The copy keeps the designators it had, taken or not.
    Keep,
    /// Every pasted part becomes `R?`, `U?`, ... for Annotate to number.
    Reset,
    /// Every pasted part takes the next unused number of its prefix in the
    /// project.
    NextFree,
}

impl PasteDesignators {
    pub const ALL: &'static [PasteDesignators] = &[
        PasteDesignators::KeepIfFree,
        PasteDesignators::Keep,
        PasteDesignators::Reset,
        PasteDesignators::NextFree,
    ];

    /// Whether numbering a paste needs the numbers the project already uses.
    pub fn reads_project(self) -> bool {
        matches!(
            self,
            PasteDesignators::KeepIfFree | PasteDesignators::NextFree
        )
    }

    /// The `prefs.json` token.
    pub fn token(self) -> &'static str {
        match self {
            PasteDesignators::KeepIfFree => "keep_if_free",
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

    /// The name a pick list shows, rendered at view time.
    pub fn label(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            PasteDesignators::KeepIfFree => ids::PREFERENCES_PASTE_DESIGNATORS_KEEP_IF_FREE,
            PasteDesignators::Keep => ids::PREFERENCES_PASTE_DESIGNATORS_KEEP,
            PasteDesignators::Reset => ids::PREFERENCES_PASTE_DESIGNATORS_RESET,
            PasteDesignators::NextFree => ids::PREFERENCES_PASTE_DESIGNATORS_NEXT_FREE,
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
            PasteDesignators::KeepIfFree
        );
    }
}
