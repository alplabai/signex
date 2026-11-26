//! Per-instance pad overrides (mockup row 192, item D1): "This pad" on an edit of a
//! patterned pad stores only the changed attributes, keyed by the instance
//! `(pattern, i, j)`; the bake applies them to that one instance. See
//! docs/FOOTPRINT_SKETCH.md, "Per-pad overrides".

use serde::{Deserialize, Serialize};

use crate::sketch::array::ArrayId;
use crate::sketch::attr::{
    DrillSpec, ElectricalType, PadAttr, PadFeature, PadShape, PadStackOverrides, TestpointFlags,
};
use crate::sketch::data::SketchData;

/// The value an override gives an optional attribute: a value, or none
/// where the source has one. (A file cannot spell an absent value, so the
/// clearing is a word of its own.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OverrideValue<T> {
    Set(T),
    Cleared,
}

impl<T: Clone> OverrideValue<T> {
    fn from_option(value: &Option<T>) -> Self {
        value.clone().map_or(Self::Cleared, Self::Set)
    }

    fn to_option(&self) -> Option<T> {
        match self {
            Self::Set(v) => Some(v.clone()),
            Self::Cleared => None,
        }
    }
}

/// Generates [`PadAttrOverride`] with one `Option` per overridable
/// attribute, plus its diff / apply / merge, so the three cannot drift.
/// `plain` attributes copy as they are; `optional` ones are `Option`s in
/// [`PadAttr`] and go through [`OverrideValue`].
macro_rules! pad_attr_override {
    (plain { $($p:ident: $pt:ty),* $(,)? } optional { $($o:ident: $ot:ty),* $(,)? }) => {
        /// The attributes one pad instance holds apart from its pattern's
        /// source pad: one `Option` per attribute the Pads-mode mirror
        /// writes, `None` where the instance follows the source.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct PadAttrOverride {
            $(
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub $p: Option<$pt>,
            )*
            $(
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub $o: Option<OverrideValue<$ot>>,
            )*
        }

        impl PadAttrOverride {
            /// The attributes that differ between `before` and `after`,
            /// with their `after` values.
            pub fn between(before: &PadAttr, after: &PadAttr) -> Self {
                Self {
                    $($p: (before.$p != after.$p).then(|| after.$p.clone()),)*
                    $($o: (before.$o != after.$o).then(|| OverrideValue::from_option(&after.$o)),)*
                }
            }

            /// Write every overridden attribute onto `attr`.
            pub fn apply_to(&self, attr: &mut PadAttr) {
                $(if let Some(v) = &self.$p {
                    attr.$p = v.clone();
                })*
                $(if let Some(v) = &self.$o {
                    attr.$o = v.to_option();
                })*
            }

            /// Take every attribute `later` overrides, keeping the rest.
            pub fn merge(&mut self, later: &Self) {
                $(if later.$p.is_some() {
                    self.$p = later.$p.clone();
                })*
                $(if later.$o.is_some() {
                    self.$o = later.$o.clone();
                })*
            }

            /// Whether nothing is overridden.
            pub fn is_empty(&self) -> bool {
                $(self.$p.is_none() &&)* $(self.$o.is_none() &&)* true
            }
        }
    };
}

pad_attr_override! {
    plain {
        number: String,
        net: String,
        locked: bool,
        electrical_type: ElectricalType,
        template: String,
        library: String,
        feature_top: PadFeature,
        feature_bottom: PadFeature,
        testpoint: TestpointFlags,
        shape: PadShape,
        stack: PadStackOverrides,
    }
    optional {
        hole_tolerance_plus_mm: f64,
        hole_tolerance_minus_mm: f64,
        hole_rotation_deg: f64,
        copper_offset_x_mm: f64,
        copper_offset_y_mm: f64,
        rotation_expr: String,
        drill: DrillSpec,
        mask_margin_expr: String,
        paste_margin_expr: String,
    }
}

/// The override of instance `(i, j)` of pattern `array` (`j` is 0 for a
/// linear or polar pattern; see `signex_bake::instance_at` for the keys of
/// linked patterns and mirrors).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceOverride {
    pub array: ArrayId,
    pub i: u32,
    pub j: u32,
    pub pad: PadAttrOverride,
}

impl SketchData {
    /// The override of instance `(i, j)` of `array`, if it has one.
    pub fn override_for(&self, array: ArrayId, i: u32, j: u32) -> Option<&PadAttrOverride> {
        self.pad_overrides
            .iter()
            .find(|o| (o.array, o.i, o.j) == (array, i, j))
            .map(|o| &o.pad)
    }

    /// Give instance `(i, j)` of `array` the override `pad`, replacing
    /// any it had; an empty one clears it.
    pub fn set_override(&mut self, array: ArrayId, i: u32, j: u32, pad: PadAttrOverride) {
        self.clear_override(array, i, j);
        if !pad.is_empty() {
            self.pad_overrides
                .push(InstanceOverride { array, i, j, pad });
        }
    }

    /// Drop the override of instance `(i, j)` of `array`. Returns whether
    /// it had one.
    pub fn clear_override(&mut self, array: ArrayId, i: u32, j: u32) -> bool {
        let before = self.pad_overrides.len();
        self.pad_overrides
            .retain(|o| (o.array, o.i, o.j) != (array, i, j));
        self.pad_overrides.len() != before
    }
}
