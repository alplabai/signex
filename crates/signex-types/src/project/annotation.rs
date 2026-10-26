//! Annotation settings stored in a `.snxprj` (#1232): which designator
//! standard decides the reference letters of newly placed parts.
//!
//! A top-level `annotation` key, deliberately not under `rules`: a present
//! `rules` section means "this project stores its ERC rules".

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ProjectData, UnknownJson};
use crate::designator::DesignatorStandard;

/// The `.snxprj` key of the annotation settings.
pub(super) const ANNOTATION_KEY: &str = "annotation";

/// Per-project annotation settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AnnotationSettings {
    /// Absent = the library symbol's own designator letter decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designator_standard: Option<DesignatorStandard>,
    /// Keys a newer build wrote inside the section; kept for re-save.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// Interpret an `annotation` section, or keep it verbatim for re-save when
/// this build cannot: an unknown standard string, or a value of another
/// shape. The project opens either way; the section is just not applied.
pub(super) fn take_annotation(data: &mut ProjectData, raw: Value) {
    match serde_json::from_value::<AnnotationSettings>(raw.clone()) {
        Ok(settings) => data.annotation = Some(settings),
        Err(err) => {
            tracing::warn!(
                target: "signex::project",
                "The project's annotation settings are not readable by this version of Signex \
                 ({err}); they are kept unchanged and not applied."
            );
            data.unknown.insert(ANNOTATION_KEY.into(), raw);
        }
    }
}

impl ProjectData {
    /// The project's designator standard; `None` for the library default.
    pub fn designator_standard(&self) -> Option<DesignatorStandard> {
        self.annotation.as_ref()?.designator_standard
    }

    /// The file has an `annotation` section this build could not interpret:
    /// kept verbatim for re-save, not applied.
    pub fn has_unreadable_annotation(&self) -> bool {
        self.annotation.is_none() && self.unknown.get(ANNOTATION_KEY).is_some()
    }

    /// Set (or, with `None`, remove) the designator standard. An unreadable
    /// section kept from a newer build is replaced by the explicit choice.
    /// Settings left empty are dropped, so clearing the choice on a project
    /// that never had one saves byte-identical to before.
    pub fn set_designator_standard(&mut self, standard: Option<DesignatorStandard>) {
        self.unknown.remove(ANNOTATION_KEY);
        let mut settings = self.annotation.take().unwrap_or_default();
        settings.designator_standard = standard;
        let empty = settings.designator_standard.is_none() && settings.unknown.is_empty();
        self.annotation = (!empty).then_some(settings);
    }
}

#[cfg(test)]
mod tests;
