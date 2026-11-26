//! The Variant Manager's settings (mockup row 32): which variant is the
//! default, and what a variant changes on one part (its fit and its
//! parameter values) over what the sheets say. Saved under the
//! `variant_settings` key of `.snxprj`, only when something is set; see
//! `docs/architecture/snxprj-format.md`, "Variant settings".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ProjectData, UnknownJson, VariantProblem, is_base_variant};
use crate::pcb::SymbolLink;

/// The `.snxprj` key of the settings.
pub(super) const VARIANT_SETTINGS_KEY: &str = "variant_settings";

/// The parameter name of a part's value in [`VariantPart::parameters`].
pub const VALUE_PARAMETER: &str = "Value";

/// The parameter name of a part's footprint in [`VariantPart::parameters`].
pub const FOOTPRINT_PARAMETER: &str = "Footprint";

/// The field an alternate part's part number is set in (#3038): the BOM's
/// `MPN` field.
pub const MPN_PARAMETER: &str = "MPN";

/// One placed part as the settings find it: the designator the BOM lists it
/// under and, when known, the placed part itself, which a re-annotation or a
/// designator edit does not change.
#[derive(Debug, Clone, Copy)]
pub struct PlacedPart<'a> {
    pub designator: &'a str,
    pub link: Option<&'a SymbolLink>,
}

impl<'a> PlacedPart<'a> {
    pub fn new(designator: &'a str, link: Option<&'a SymbolLink>) -> Self {
        Self { designator, link }
    }
}

/// A part known by its designator only.
impl<'a> From<&'a str> for PlacedPart<'a> {
    fn from(designator: &'a str) -> Self {
        Self::new(designator, None)
    }
}

/// An owned [`PlacedPart`], for an edit that names several parts (every
/// channel's copy of a repeated part, or every part of one channel).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartRef {
    pub designator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<SymbolLink>,
}

impl PartRef {
    pub fn new(designator: impl Into<String>, part: Option<SymbolLink>) -> Self {
        Self {
            designator: designator.into(),
            part,
        }
    }

    pub fn placed(&self) -> PlacedPart<'_> {
        PlacedPart::new(&self.designator, self.part.as_ref())
    }
}

/// What one variant changes on one part. A channel's copy of a part in a
/// repeated sheet is a part of its own: its [`Self::part`] is placed at the
/// channel's placement, so an entry is per channel (`docs/MULTI_CHANNEL.md`,
/// "Per-channel variant state").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantPart {
    pub variant: String,
    /// The designator the part had when the entry was last set: what the
    /// entry names when it has no [`Self::part`].
    pub designator: String,
    /// The placed part the entry was set on. It decides over the designator,
    /// so the entry stays on its part through a re-annotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<SymbolLink>,
    /// Fitted (`true`) or not (`false`) in this variant; `None` follows the
    /// sheets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fitted: Option<bool>,
    /// Values this variant uses instead of the part's own, by parameter
    /// name: [`VALUE_PARAMETER`], [`FOOTPRINT_PARAMETER`] or a field name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, String>,
    /// Keys a newer build wrote here (#788), kept for re-save.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl VariantPart {
    fn is_empty(&self) -> bool {
        self.fitted.is_none() && self.parameters.is_empty() && self.unknown.is_empty()
    }

    fn is_of(&self, variant: &str) -> bool {
        self.variant.eq_ignore_ascii_case(variant.trim())
    }

    /// Whether this entry is for `placed`: by the placed part when both know
    /// it, else by the designator. An entry set on another placed part never
    /// names this one, even under its old designator.
    pub fn is_for(&self, placed: PlacedPart<'_>) -> bool {
        match (&self.part, placed.link) {
            (Some(own), Some(link)) => own.names_part(&link.sheet, &link.units),
            _ => self.designator == placed.designator.trim(),
        }
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
    /// Keys a newer build wrote here (#788), kept for re-save.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl VariantSettings {
    /// True when nothing is set: the key is not written.
    pub fn is_empty(&self) -> bool {
        self.default.is_none() && self.parts.is_empty() && self.unknown.is_empty()
    }

    /// What `variant` changes on the part `placed` names; Base (`None` or
    /// its name) changes nothing.
    pub fn part<'p>(
        &self,
        variant: Option<&str>,
        placed: impl Into<PlacedPart<'p>>,
    ) -> Option<&VariantPart> {
        let variant = variant.filter(|name| !is_base_variant(name))?;
        let placed = placed.into();
        self.parts
            .iter()
            .find(|part| part.is_of(variant) && part.is_for(placed))
    }
}

/// A part's fit in the variant Duplicate copies, as its sheets give it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetFit {
    pub designator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<SymbolLink>,
    pub fitted: bool,
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

    /// Fit the part `placed` names in `variant` (`Some`), or let it follow
    /// the sheets (`None`). Base follows its sheets and is refused.
    pub fn set_variant_part_fit<'p>(
        &mut self,
        variant: &str,
        placed: impl Into<PlacedPart<'p>>,
        fitted: Option<bool>,
    ) -> Result<(), VariantProblem> {
        self.edit_variant_part(variant, placed.into(), |part| part.fitted = fitted)
    }

    /// [`Self::set_variant_part_fit`] for each of `parts`, as one edit: every
    /// channel's copy of a repeated part, or every part of one channel. A
    /// refusal (Base, an unknown variant, unreadable settings) comes before
    /// any part changes.
    pub fn set_variant_parts_fit(
        &mut self,
        variant: &str,
        parts: &[PartRef],
        fitted: Option<bool>,
    ) -> Result<(), VariantProblem> {
        self.check_settings_readable()?;
        if is_base_variant(variant) {
            return Err(VariantProblem::Reserved);
        }
        self.defined_variant(variant)?;
        for part in parts {
            self.set_variant_part_fit(variant, part.placed(), fitted)?;
        }
        Ok(())
    }

    /// Give the part `placed` names the value `value` for `parameter` in
    /// `variant`, or its own value again (`None`). An empty parameter name
    /// changes nothing.
    pub fn set_variant_part_parameter<'p>(
        &mut self,
        variant: &str,
        placed: impl Into<PlacedPart<'p>>,
        parameter: &str,
        value: Option<&str>,
    ) -> Result<(), VariantProblem> {
        let parameter = parameter.trim();
        if parameter.is_empty() {
            return Ok(());
        }
        self.edit_variant_part(variant, placed.into(), |part| match value {
            Some(value) => {
                part.parameters
                    .insert(parameter.to_string(), value.to_string());
            }
            None => {
                part.parameters.remove(parameter);
            }
        })
    }

    /// [`Self::set_variant_part_parameter`] for each of `parameters` on one
    /// part, as one edit: an alternate part's value, footprint and part
    /// number together (#3038). A refusal comes before any value changes;
    /// an empty parameter name is skipped.
    pub fn set_variant_part_parameters<'p>(
        &mut self,
        variant: &str,
        placed: impl Into<PlacedPart<'p>>,
        parameters: &[(String, Option<String>)],
    ) -> Result<(), VariantProblem> {
        self.edit_variant_part(variant, placed.into(), |part| {
            for (parameter, value) in parameters {
                let parameter = parameter.trim();
                if parameter.is_empty() {
                    continue;
                }
                match value {
                    Some(value) => {
                        part.parameters.insert(parameter.to_string(), value.clone());
                    }
                    None => {
                        part.parameters.remove(parameter);
                    }
                }
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
        sheet_fits: &[SheetFit],
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
            .filter(|part| part.is_of(&from))
            .map(|part| VariantPart {
                variant: copy.clone(),
                ..part.clone()
            })
            .collect();
        for fit in sheet_fits {
            let placed = PlacedPart::new(&fit.designator, fit.part.as_ref());
            match copied.iter_mut().find(|part| part.is_for(placed)) {
                Some(part) => {
                    part.fitted.get_or_insert(fit.fitted);
                }
                None => copied.push(VariantPart {
                    variant: copy.clone(),
                    designator: fit.designator.clone(),
                    part: fit.part.clone(),
                    fitted: Some(fit.fitted),
                    ..VariantPart::default()
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
            if part.is_of(old) {
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
        settings.parts.retain(|part| !part.is_of(removed));
        if settings
            .default
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(removed))
        {
            settings.default = None;
        }
    }

    /// Change `variant`'s entry for `placed`, adding it when missing and
    /// dropping it when it no longer changes anything. The entry takes the
    /// part's designator now and, when given, its placed part.
    fn edit_variant_part(
        &mut self,
        variant: &str,
        placed: PlacedPart<'_>,
        edit: impl FnOnce(&mut VariantPart),
    ) -> Result<(), VariantProblem> {
        self.check_settings_readable()?;
        if is_base_variant(variant) {
            return Err(VariantProblem::Reserved);
        }
        let variant = self.defined_variant(variant)?;
        let designator = placed.designator.trim();
        if designator.is_empty() {
            return Ok(());
        }
        let parts = &mut self.variant_settings.parts;
        let index = match parts
            .iter()
            .position(|part| part.is_of(&variant) && part.is_for(placed))
        {
            Some(index) => index,
            None => {
                parts.push(VariantPart {
                    variant,
                    ..VariantPart::default()
                });
                parts.len() - 1
            }
        };
        let part = &mut parts[index];
        part.designator = designator.to_string();
        if let Some(link) = placed.link {
            part.part = Some(widened_link(part.part.as_ref(), link));
        }
        edit(part);
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

    /// Refuse a variant edit while the settings section is unreadable: the
    /// kept section names the variants as the file has them.
    pub(super) fn check_settings_readable(&self) -> Result<(), VariantProblem> {
        if self.has_unreadable_variant_settings() {
            return Err(VariantProblem::SettingsUnreadable);
        }
        Ok(())
    }
}

/// The link an edit leaves on an entry that had `own`: `link`, with the
/// units `own` already names on the same sheet kept, so an edit that knows
/// one unit of a part never narrows an entry the matrix linked to all of
/// them. A link on another sheet is replaced.
fn widened_link(own: Option<&SymbolLink>, link: &SymbolLink) -> SymbolLink {
    match own {
        Some(own) if own.sheet == link.sheet => SymbolLink::new(
            link.sheet.clone(),
            own.units.iter().chain(&link.units).copied(),
        ),
        _ => link.clone(),
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
