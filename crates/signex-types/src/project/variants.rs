//! The project's variants (#1690): `variant_definitions` and
//! `active_variant`, edited in the Project tab's Variants section.
//!
//! Every project has the Base variant (every part fitted). It is not stored:
//! `active_variant` `None` is Base, and no definition may take its name.
//! Names are compared without regard to ASCII case, as the BOM and PDF
//! exports compare `Base`.

use super::ProjectData;

/// The variant every project has; never stored as a definition.
pub const BASE_VARIANT: &str = "Base";

/// The longest variant name, in characters.
pub const MAX_VARIANT_NAME_CHARS: usize = 64;

/// Why a variant edit is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantProblem {
    /// The name is empty or only spaces.
    Empty,
    /// The name is longer than [`MAX_VARIANT_NAME_CHARS`].
    TooLong,
    /// The name holds a line break or another control character.
    Invalid,
    /// The name is Base's.
    Reserved,
    /// Another variant has the name.
    Taken(String),
    /// No variant has the name.
    Unknown(String),
    /// The file's `variant_settings` section could not be read, so no
    /// variant setting can be changed (mockup row 32).
    SettingsUnreadable,
}

impl std::fmt::Display for VariantProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("a variant name cannot be empty"),
            Self::TooLong => write!(
                f,
                "a variant name is at most {MAX_VARIANT_NAME_CHARS} characters"
            ),
            Self::Invalid => {
                f.write_str("a variant name cannot hold line breaks or control characters")
            }
            Self::Reserved => write!(f, "`{BASE_VARIANT}` is the variant every project has"),
            Self::Taken(name) => write!(f, "the project already has a variant named `{name}`"),
            Self::Unknown(name) => write!(f, "the project has no variant named `{name}`"),
            Self::SettingsUnreadable => {
                f.write_str("the project's variant settings could not be read")
            }
        }
    }
}

/// Whether `name` names Base.
pub fn is_base_variant(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case(BASE_VARIANT)
}

impl ProjectData {
    /// The index of the variant named `name`, ignoring ASCII case.
    fn variant_index(&self, name: &str) -> Option<usize> {
        let name = name.trim();
        self.variant_definitions
            .iter()
            .position(|defined| defined.eq_ignore_ascii_case(name))
    }

    /// `name` trimmed, when it may name a new variant (or `renaming`'s new
    /// name: a case-only rename of the same variant is allowed).
    pub fn check_variant_name(
        &self,
        name: &str,
        renaming: Option<&str>,
    ) -> Result<String, VariantProblem> {
        let name = name.trim();
        if name.is_empty() {
            return Err(VariantProblem::Empty);
        }
        if name.chars().count() > MAX_VARIANT_NAME_CHARS {
            return Err(VariantProblem::TooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(VariantProblem::Invalid);
        }
        if is_base_variant(name) {
            return Err(VariantProblem::Reserved);
        }
        let own = renaming.and_then(|from| self.variant_index(from));
        match self.variant_index(name) {
            Some(index) if Some(index) != own => Err(VariantProblem::Taken(
                self.variant_definitions[index].clone(),
            )),
            _ => Ok(name.to_string()),
        }
    }

    /// Add a variant after the others.
    pub fn add_variant(&mut self, name: &str) -> Result<(), VariantProblem> {
        let name = self.check_variant_name(name, None)?;
        self.variant_definitions.push(name);
        Ok(())
    }

    /// Rename a variant; the active variant and every Part lifecycle
    /// requirement checking it follow its new name. Refused while the
    /// variant settings are unreadable: the kept section would still name
    /// the old variant.
    pub fn rename_variant(&mut self, from: &str, to: &str) -> Result<(), VariantProblem> {
        self.check_settings_readable()?;
        let index = self
            .variant_index(from)
            .ok_or_else(|| VariantProblem::Unknown(from.trim().to_string()))?;
        let to = self.check_variant_name(to, Some(from))?;
        let old = std::mem::replace(&mut self.variant_definitions[index], to.clone());
        self.rename_requirement_variant(&old, &to);
        self.rename_variant_settings(&old, &to);
        if self
            .active_variant
            .as_deref()
            .is_some_and(|active| active.eq_ignore_ascii_case(&old))
        {
            self.active_variant = Some(to);
        }
        Ok(())
    }

    /// Remove a variant; when it was the active one, Base becomes active.
    /// Refused while the variant settings are unreadable, as a rename is.
    pub fn remove_variant(&mut self, name: &str) -> Result<(), VariantProblem> {
        self.check_settings_readable()?;
        let index = self
            .variant_index(name)
            .ok_or_else(|| VariantProblem::Unknown(name.trim().to_string()))?;
        let removed = self.variant_definitions.remove(index);
        self.remove_variant_settings(&removed);
        if self
            .active_variant
            .as_deref()
            .is_some_and(|active| active.eq_ignore_ascii_case(&removed))
        {
            self.active_variant = None;
        }
        Ok(())
    }

    /// Make `name` the active variant; `None` or Base's name is Base. The
    /// stored name is the definition's own spelling.
    pub fn set_active_variant(&mut self, name: Option<&str>) -> Result<(), VariantProblem> {
        let name = name.filter(|name| !is_base_variant(name));
        self.active_variant = match name {
            None => None,
            Some(name) => {
                let index = self
                    .variant_index(name)
                    .ok_or_else(|| VariantProblem::Unknown(name.trim().to_string()))?;
                Some(self.variant_definitions[index].clone())
            }
        };
        Ok(())
    }

    /// The active variant's name, Base when none is.
    pub fn active_variant_name(&self) -> &str {
        self.active_variant
            .as_deref()
            .filter(|name| !is_base_variant(name))
            .unwrap_or(BASE_VARIANT)
    }
}
