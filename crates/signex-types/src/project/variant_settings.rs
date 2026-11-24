//! The Variant Manager's settings (mockup row 32): which variant is the
//! default, and what a variant changes on one part (its fit and its
//! parameter values) over what the sheets say. Saved under the
//! `variant_settings` key of `.snxprj`, only when something is set; see
//! `docs/architecture/snxprj-format.md`, "Variant settings".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ProjectData, VariantProblem, is_base_variant};

/// The `.snxprj` key of the settings.
pub(super) const VARIANT_SETTINGS_KEY: &str = "variant_settings";

/// The parameter name of a part's value in [`VariantPart::parameters`].
pub const VALUE_PARAMETER: &str = "Value";

/// The parameter name of a part's footprint in [`VariantPart::parameters`].
pub const FOOTPRINT_PARAMETER: &str = "Footprint";

/// What one variant changes on one part, named by the designator the BOM
/// lists it under.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantPart {
    pub variant: String,
    pub designator: String,
    /// Fitted (`true`) or not (`false`) in this variant; `None` follows the
    /// sheets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fitted: Option<bool>,
    /// Values this variant uses instead of the part's own, by parameter
    /// name: [`VALUE_PARAMETER`], [`FOOTPRINT_PARAMETER`] or a field name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, String>,
}

impl VariantPart {
    fn is_empty(&self) -> bool {
        self.fitted.is_none() && self.parameters.is_empty()
    }

    /// Whether this entry is `variant`'s (ignoring ASCII case) for the part
    /// `designator`.
    pub fn names(&self, variant: &str, designator: &str) -> bool {
        self.variant.eq_ignore_ascii_case(variant.trim()) && self.designator == designator.trim()
    }
}

/// The project's variant settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantSettings {
    /// The default variant; `None` is Base. Not the active variant, which is
    /// what the editors show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// What each variant changes on its parts, in the order set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<VariantPart>,
}

impl VariantSettings {
    /// True when nothing is set: the key is not written.
    pub fn is_empty(&self) -> bool {
        self.default.is_none() && self.parts.is_empty()
    }

    /// What `variant` changes on `designator`; Base (`None` or its name)
    /// changes nothing.
    pub fn part(&self, variant: Option<&str>, designator: &str) -> Option<&VariantPart> {
        let variant = variant.filter(|name| !is_base_variant(name))?;
        self.parts
            .iter()
            .find(|part| part.names(variant, designator))
    }
}

impl ProjectData {
    /// The file has a `variant_settings` section this build could not read:
    /// kept verbatim, never replaced, and no variant setting can be changed.
    pub fn has_unreadable_variant_settings(&self) -> bool {
        self.unknown.get(VARIANT_SETTINGS_KEY).is_some()
    }

    /// The default variant's definition; `None` is Base.
    pub fn default_variant(&self) -> Option<&str> {
        let name = self.variant_settings.default.as_deref()?;
        self.variant_definitions
            .iter()
            .find(|defined| defined.eq_ignore_ascii_case(name.trim()))
            .map(String::as_str)
    }

    /// Make `name` the default variant; `None` or Base's name is Base.
    pub fn set_default_variant(&mut self, name: Option<&str>) -> Result<(), VariantProblem> {
        self.check_settings_readable()?;
        self.variant_settings.default = match name.filter(|name| !is_base_variant(name)) {
            None => None,
            Some(name) => Some(self.defined_variant(name)?),
        };
        Ok(())
    }

    /// Fit `designator` in `variant` (`Some`), or let it follow the sheets
    /// (`None`). Base follows its sheets and is refused.
    pub fn set_variant_part_fit(
        &mut self,
        variant: &str,
        designator: &str,
        fitted: Option<bool>,
    ) -> Result<(), VariantProblem> {
        self.edit_variant_part(variant, designator, |part| part.fitted = fitted)
    }

    /// Give `designator` the value `value` for `parameter` in `variant`, or
    /// its own value again (`None`). An empty parameter name changes nothing.
    pub fn set_variant_part_parameter(
        &mut self,
        variant: &str,
        designator: &str,
        parameter: &str,
        value: Option<&str>,
    ) -> Result<(), VariantProblem> {
        let parameter = parameter.trim();
        if parameter.is_empty() {
            return Ok(());
        }
        self.edit_variant_part(variant, designator, |part| match value {
            Some(value) => {
                part.parameters
                    .insert(parameter.to_string(), value.to_string());
            }
            None => {
                part.parameters.remove(parameter);
            }
        })
    }

    /// Add `name` as a copy of `from`: what `from` changes on its parts,
    /// then each of `sheet_fits` (a part's fit in `from` as its sheets say,
    /// where that differs from Base) that the copy does not set itself.
    /// Copying Base adds a variant that changes nothing.
    pub fn duplicate_variant(
        &mut self,
        from: &str,
        name: &str,
        sheet_fits: &BTreeMap<String, bool>,
    ) -> Result<(), VariantProblem> {
        self.check_settings_readable()?;
        if is_base_variant(from) {
            return self.add_variant(name);
        }
        let from = self.defined_variant(from)?;
        self.add_variant(name)?;
        let copy = self.variant_definitions.last().cloned().unwrap_or_default();
        let mut copied: Vec<VariantPart> = self
            .variant_settings
            .parts
            .iter()
            .filter(|part| part.variant.eq_ignore_ascii_case(&from))
            .map(|part| VariantPart {
                variant: copy.clone(),
                ..part.clone()
            })
            .collect();
        for (designator, fitted) in sheet_fits {
            match copied
                .iter_mut()
                .find(|part| part.designator == *designator)
            {
                Some(part) => {
                    part.fitted.get_or_insert(*fitted);
                }
                None => copied.push(VariantPart {
                    variant: copy.clone(),
                    designator: designator.clone(),
                    fitted: Some(*fitted),
                    parameters: BTreeMap::new(),
                }),
            }
        }
        self.variant_settings.parts.extend(copied);
        Ok(())
    }

    /// After a rename: the settings follow the variant's new name.
    pub(super) fn rename_variant_settings(&mut self, old: &str, new: &str) {
        let settings = &mut self.variant_settings;
        for part in &mut settings.parts {
            if part.variant.eq_ignore_ascii_case(old) {
                part.variant = new.to_string();
            }
        }
        if settings
            .default
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(old))
        {
            settings.default = Some(new.to_string());
        }
    }

    /// After a removal: the variant's settings go; the default falls back
    /// to Base when it was the default.
    pub(super) fn remove_variant_settings(&mut self, removed: &str) {
        let settings = &mut self.variant_settings;
        settings
            .parts
            .retain(|part| !part.variant.eq_ignore_ascii_case(removed));
        if settings
            .default
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(removed))
        {
            settings.default = None;
        }
    }

    /// Change `variant`'s entry for `designator`, adding it when missing and
    /// dropping it when it no longer changes anything.
    fn edit_variant_part(
        &mut self,
        variant: &str,
        designator: &str,
        edit: impl FnOnce(&mut VariantPart),
    ) -> Result<(), VariantProblem> {
        self.check_settings_readable()?;
        if is_base_variant(variant) {
            return Err(VariantProblem::Reserved);
        }
        let variant = self.defined_variant(variant)?;
        let designator = designator.trim();
        if designator.is_empty() {
            return Ok(());
        }
        let parts = &mut self.variant_settings.parts;
        let index = match parts
            .iter()
            .position(|part| part.names(&variant, designator))
        {
            Some(index) => index,
            None => {
                parts.push(VariantPart {
                    variant,
                    designator: designator.to_string(),
                    ..VariantPart::default()
                });
                parts.len() - 1
            }
        };
        edit(&mut parts[index]);
        if parts[index].is_empty() {
            parts.remove(index);
        }
        Ok(())
    }

    /// The definition's own spelling of `name`.
    fn defined_variant(&self, name: &str) -> Result<String, VariantProblem> {
        self.variant_definitions
            .iter()
            .find(|defined| defined.eq_ignore_ascii_case(name.trim()))
            .cloned()
            .ok_or_else(|| VariantProblem::Unknown(name.trim().to_string()))
    }

    fn check_settings_readable(&self) -> Result<(), VariantProblem> {
        if self.has_unreadable_variant_settings() {
            return Err(VariantProblem::SettingsUnreadable);
        }
        Ok(())
    }
}

/// Interpret the `variant_settings` section, or keep it for re-save when
/// this build cannot read it: the project opens either way and the section
/// is written back as it was.
pub(super) fn take_variant_settings(data: &mut ProjectData, raw: Value) {
    match serde_json::from_value::<VariantSettings>(raw.clone()) {
        Ok(settings) => data.variant_settings = settings,
        Err(error) => {
            tracing::warn!(
                target: "signex::format",
                "the project's {VARIANT_SETTINGS_KEY} section could not be read ({error}); \
                 it is kept unchanged and not applied"
            );
            data.unknown.insert(VARIANT_SETTINGS_KEY.into(), raw);
        }
    }
}
