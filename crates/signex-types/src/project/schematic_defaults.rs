//! A project's overrides of the schematic primitive defaults (#2026), the
//! top-level `schematic_defaults` key of a `.snxprj`. Every field is
//! optional: a missing key falls back to the built-in table
//! ([`SchematicDefaults::BUILT_IN`]); keys a newer build wrote are kept.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ProjectData, UnknownJson};
use crate::schematic::{SCHEMATIC_DEFAULT_FONT, SchematicDefaults, is_set_width};

/// The `.snxprj` key of the schematic defaults.
pub(super) const SCHEMATIC_DEFAULTS_KEY: &str = "schematic_defaults";

/// Per-project defaults for unset schematic primitives. Widths are mm, the
/// text size is pt. A value that is not a finite number above zero is
/// treated as unset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SchematicDefaultOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_width_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus_width_mm: Option<f64>,
    /// Symbol lines and so pins; unset follows the wire width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_line_width_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape_stroke_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub junction_diameter_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_size_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// Keys a newer build wrote inside the section; kept for re-save.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// Which override a Project tab field edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchematicDefaultField {
    WireWidth,
    BusWidth,
    SymbolLineWidth,
    ShapeStroke,
    JunctionDiameter,
    TextSize,
}

impl SchematicDefaultField {
    /// Every numeric field, in display order.
    pub const ALL: [Self; 6] = [
        Self::WireWidth,
        Self::BusWidth,
        Self::SymbolLineWidth,
        Self::ShapeStroke,
        Self::JunctionDiameter,
        Self::TextSize,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::WireWidth => "Wire width (mm)",
            Self::BusWidth => "Bus width (mm)",
            Self::SymbolLineWidth => "Symbol line and pin width (mm)",
            Self::ShapeStroke => "Shape stroke (mm)",
            Self::JunctionDiameter => "Junction diameter (mm)",
            Self::TextSize => "Text size (pt)",
        }
    }

    /// The value this field resolves to in `defaults`.
    pub fn resolved(self, defaults: &SchematicDefaults) -> f64 {
        match self {
            Self::WireWidth => defaults.wire_width_mm,
            Self::BusWidth => defaults.bus_width_mm,
            Self::SymbolLineWidth => defaults.symbol_line_width_mm,
            Self::ShapeStroke => defaults.shape_stroke_mm,
            Self::JunctionDiameter => defaults.junction_diameter_mm,
            Self::TextSize => defaults.text_size_pt,
        }
    }
}

fn usable(value: Option<f64>) -> Option<f64> {
    value.filter(|v| is_set_width(*v))
}

impl SchematicDefaultOverrides {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The stored override of `field`.
    pub fn get(&self, field: SchematicDefaultField) -> Option<f64> {
        *self.slot(field)
    }

    /// This set with `field` set to `value` (`None` clears it).
    #[must_use]
    pub fn with(&self, field: SchematicDefaultField, value: Option<f64>) -> Self {
        let mut next = self.clone();
        *next.slot_mut(field) = value;
        next
    }

    fn slot(&self, field: SchematicDefaultField) -> &Option<f64> {
        match field {
            SchematicDefaultField::WireWidth => &self.wire_width_mm,
            SchematicDefaultField::BusWidth => &self.bus_width_mm,
            SchematicDefaultField::SymbolLineWidth => &self.symbol_line_width_mm,
            SchematicDefaultField::ShapeStroke => &self.shape_stroke_mm,
            SchematicDefaultField::JunctionDiameter => &self.junction_diameter_mm,
            SchematicDefaultField::TextSize => &self.text_size_pt,
        }
    }

    fn slot_mut(&mut self, field: SchematicDefaultField) -> &mut Option<f64> {
        match field {
            SchematicDefaultField::WireWidth => &mut self.wire_width_mm,
            SchematicDefaultField::BusWidth => &mut self.bus_width_mm,
            SchematicDefaultField::SymbolLineWidth => &mut self.symbol_line_width_mm,
            SchematicDefaultField::ShapeStroke => &mut self.shape_stroke_mm,
            SchematicDefaultField::JunctionDiameter => &mut self.junction_diameter_mm,
            SchematicDefaultField::TextSize => &mut self.text_size_pt,
        }
    }

    /// These overrides over the built-in table. Symbol lines (and so pins)
    /// follow the wire width unless set themselves.
    pub fn resolve(&self) -> SchematicDefaults {
        let built_in = SchematicDefaults::BUILT_IN;
        let wire = usable(self.wire_width_mm).unwrap_or(built_in.wire_width_mm);
        SchematicDefaults {
            wire_width_mm: wire,
            bus_width_mm: usable(self.bus_width_mm).unwrap_or(built_in.bus_width_mm),
            symbol_line_width_mm: usable(self.symbol_line_width_mm).unwrap_or(wire),
            shape_stroke_mm: usable(self.shape_stroke_mm).unwrap_or(built_in.shape_stroke_mm),
            junction_diameter_mm: usable(self.junction_diameter_mm)
                .unwrap_or(built_in.junction_diameter_mm),
            text_size_pt: usable(self.text_size_pt).unwrap_or(built_in.text_size_pt),
        }
    }

    /// The font new text is placed in.
    pub fn font_family(&self) -> &str {
        self.font_family
            .as_deref()
            .filter(|f| !f.trim().is_empty())
            .unwrap_or(SCHEMATIC_DEFAULT_FONT)
    }
}

/// Interpret a `schematic_defaults` section, or keep it verbatim for re-save
/// when this build cannot read it. The project opens either way.
pub(super) fn take_schematic_defaults(data: &mut ProjectData, raw: Value) {
    match serde_json::from_value::<SchematicDefaultOverrides>(raw.clone()) {
        Ok(overrides) => data.schematic_defaults = overrides,
        Err(err) => {
            tracing::warn!(
                target: "signex::project",
                "The project's schematic defaults are not readable by this version of Signex \
                 ({err}); they are kept unchanged and the built-in defaults apply."
            );
            data.unknown.insert(SCHEMATIC_DEFAULTS_KEY.into(), raw);
        }
    }
}
