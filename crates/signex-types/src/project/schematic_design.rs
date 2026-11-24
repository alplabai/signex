//! Project-wide Schematic Refinement settings (#1267, #1279, #1282): how a
//! repeated channel names its parts and nets, the harness definitions and
//! the differential pair classes. Saved under the `schematic_design` key of
//! `.snxprj`, only when something differs from the defaults
//! (`docs/architecture/snxprj-format.md`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ProjectData;
use crate::diff_pair_class::DiffPairClass;
use crate::schematic::HarnessDefinition;

/// The `.snxprj` key of the settings.
pub(super) const SCHEMATIC_DESIGN_KEY: &str = "schematic_design";

/// The default name of a part in a repeated channel.
pub const DEFAULT_CHANNEL_DESIGNATOR_FORMAT: &str = "${Channel}_${Designator}";

/// The default name of a net in a repeated channel.
pub const DEFAULT_CHANNEL_NET_FORMAT: &str = "${Net}_${Channel}";

/// How the parts and nets of a repeated channel are named. `${Channel}`,
/// `${Designator}` and `${Net}` stand for the channel name, the part's
/// designator on the channel sheet and the net's name there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelNaming {
    #[serde(default = "default_designator_format")]
    pub designator_format: String,
    #[serde(default = "default_net_format")]
    pub net_format: String,
}

fn default_designator_format() -> String {
    DEFAULT_CHANNEL_DESIGNATOR_FORMAT.to_string()
}

fn default_net_format() -> String {
    DEFAULT_CHANNEL_NET_FORMAT.to_string()
}

impl Default for ChannelNaming {
    fn default() -> Self {
        Self {
            designator_format: default_designator_format(),
            net_format: default_net_format(),
        }
    }
}

impl ChannelNaming {
    /// True for the default formats, which are not written.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The project's Schematic Refinement settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SchematicDesignSettings {
    #[serde(default, skip_serializing_if = "ChannelNaming::is_default")]
    pub channel_naming: ChannelNaming,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub harness_definitions: Vec<HarnessDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diff_pair_classes: Vec<DiffPairClass>,
}

impl SchematicDesignSettings {
    /// True when nothing differs from the defaults: the key is not written.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl ProjectData {
    /// The file has a `schematic_design` section this build could not read:
    /// kept verbatim, never replaced, and its settings cannot be edited
    /// (DECISIONS 2026-11-23, #2750).
    pub fn has_unreadable_schematic_design(&self) -> bool {
        self.unknown.get(SCHEMATIC_DESIGN_KEY).is_some()
    }
}

/// Interpret the `schematic_design` section, or keep it for re-save when
/// this build cannot read it: the project opens either way and the section
/// is written back as it was.
pub(super) fn take_schematic_design(data: &mut ProjectData, raw: Value) {
    match serde_json::from_value::<SchematicDesignSettings>(raw.clone()) {
        Ok(settings) => data.schematic_design = settings,
        Err(error) => {
            tracing::warn!(
                target: "signex::format",
                "the project's {SCHEMATIC_DESIGN_KEY} section could not be read ({error}); \
                 it is kept unchanged and not applied"
            );
            data.unknown.insert(SCHEMATIC_DESIGN_KEY.into(), raw);
        }
    }
}
