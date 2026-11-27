//! What a placed component is beyond its geometry (#2365): its type, height,
//! 3D body opacity, primitive and string locks, swapping options, channel
//! offset and the library footprint's description. Saved in the footprint's
//! `[extras.footprints.<uuid>.component]` table, each key only when it is not
//! its default, so a board saved before the table re-saves byte-identical.

use serde::{Deserialize, Serialize};

/// What kind of part a component is, for the BOM and for connectivity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentType {
    /// An electrical part, listed in the BOM.
    #[default]
    Standard,
    /// An electrical part left out of the BOM.
    StandardNoBom,
    /// A mechanical part (a heatsink, a standoff), left out of the BOM.
    Mechanical,
    /// Board graphics only (a logo, a fiducial), left out of the BOM.
    Graphical,
    /// Pads that may short two nets, left out of the BOM.
    NetTie,
    /// Pads that may short two nets, listed in the BOM.
    NetTieInBom,
    /// A wire link between pads, listed in the BOM.
    Jumper,
}

impl ComponentType {
    /// Every type, in the order a picker lists them.
    pub const ALL: [Self; 7] = [
        Self::Standard,
        Self::StandardNoBom,
        Self::Mechanical,
        Self::Graphical,
        Self::NetTie,
        Self::NetTieInBom,
        Self::Jumper,
    ];

    /// Whether a part of this type is listed in the BOM.
    pub fn in_bom(self) -> bool {
        !matches!(
            self,
            Self::StandardNoBom | Self::Mechanical | Self::Graphical | Self::NetTie
        )
    }

    /// Whether the part's pads may short their nets on purpose: a net tie
    /// (#1277), listed in the BOM or not.
    pub fn is_net_tie(self) -> bool {
        matches!(self, Self::NetTie | Self::NetTieInBom)
    }

    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The opacity a component's 3D body is drawn with when none is set.
pub const DEFAULT_BODY_OPACITY: f64 = 1.0;

fn default_body_opacity() -> f64 {
    DEFAULT_BODY_OPACITY
}

fn is_default_body_opacity(opacity: &f64) -> bool {
    *opacity == DEFAULT_BODY_OPACITY
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !value
}

/// The component settings of a board footprint. The default is a standard
/// part with no height, an opaque body, no locks and no swapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentAttributes {
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "ComponentType::is_default"
    )]
    pub component_type: ComponentType,
    /// The part's height above the board in mm; 0 is not set.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub height_mm: f64,
    /// How opaque the 3D body is drawn, 0 (clear) to 1 (solid).
    #[serde(
        default = "default_body_opacity",
        skip_serializing_if = "is_default_body_opacity"
    )]
    pub body_opacity: f64,
    /// The footprint's pads and graphics refuse edits of their own.
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_primitives: bool,
    /// The footprint's texts refuse to move.
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_strings: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pin_swapping: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub part_swapping: bool,
    /// The offset added to the channel index of a part in a repeated sheet.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub channel_offset: u32,
    /// The library footprint's description, kept on the placed footprint.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl Default for ComponentAttributes {
    fn default() -> Self {
        Self {
            component_type: ComponentType::Standard,
            height_mm: 0.0,
            body_opacity: DEFAULT_BODY_OPACITY,
            lock_primitives: false,
            lock_strings: false,
            pin_swapping: false,
            part_swapping: false,
            channel_offset: 0,
            description: String::new(),
        }
    }
}

impl ComponentAttributes {
    /// Whether every setting is its default: the board saves no table.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Why these settings cannot be stored, if they cannot: a height that is
    /// negative or not a number, an opacity outside 0..=1.
    pub fn invalid_field(&self) -> Option<&'static str> {
        if !(self.height_mm.is_finite() && self.height_mm >= 0.0) {
            return Some("height");
        }
        if !(self.body_opacity.is_finite() && (0.0..=1.0).contains(&self.body_opacity)) {
            return Some("body_opacity");
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_no_bom_types_leave_the_bom() {
        let out: Vec<ComponentType> = ComponentType::ALL
            .into_iter()
            .filter(|t| !t.in_bom())
            .collect();
        assert_eq!(
            out,
            [
                ComponentType::StandardNoBom,
                ComponentType::Mechanical,
                ComponentType::Graphical,
                ComponentType::NetTie,
            ]
        );
    }

    #[test]
    fn only_the_two_net_tie_types_may_short_their_pads() {
        let ties: Vec<ComponentType> = ComponentType::ALL
            .into_iter()
            .filter(|t| t.is_net_tie())
            .collect();
        assert_eq!(ties, [ComponentType::NetTie, ComponentType::NetTieInBom]);
    }

    #[test]
    fn a_negative_height_or_an_opacity_past_one_is_invalid() {
        let ok = ComponentAttributes::default();
        assert_eq!(ok.invalid_field(), None);
        let low = ComponentAttributes {
            height_mm: -1.0,
            ..ComponentAttributes::default()
        };
        assert_eq!(low.invalid_field(), Some("height"));
        let clear = ComponentAttributes {
            body_opacity: 1.5,
            ..ComponentAttributes::default()
        };
        assert_eq!(clear.invalid_field(), Some("body_opacity"));
    }
}
