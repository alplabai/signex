//! The simple rule file Import rules loads (#2232): a small TOML file a
//! fabricator or a team writes. Format: `docs/PROJECT_SETUP.md`.

use serde::Deserialize;

use super::setup::{COPPER_LAYERS, PcbField, PcbValueError};

/// A parsed rule file.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleTemplate {
    pub name: Option<String>,
    pub clearance_mm: f64,
    pub track_width_mm: f64,
    pub via_diameter_mm: f64,
    pub via_drill_mm: f64,
    pub copper_layers: Option<u8>,
    pub thickness_mm: Option<f64>,
}

/// Why a rule file was refused; every message names the key.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RuleFileError {
    #[error("the rule file is not valid TOML: {0}")]
    Syntax(String),
    #[error("the rule file has no `[pcb]` table")]
    NoPcbTable,
    #[error("`{0}` is missing from `[pcb]`")]
    Missing(&'static str),
    #[error("`{}` {why}", .field.key())]
    Value { field: PcbField, why: PcbValueError },
    #[error("the rule file is larger than {0} bytes")]
    TooLarge(usize),
}

/// Largest rule file read; a real one is a few hundred bytes.
pub const MAX_RULE_FILE_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    name: Option<String>,
    pcb: Option<RawPcb>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPcb {
    clearance_mm: Option<f64>,
    track_width_mm: Option<f64>,
    via_diameter_mm: Option<f64>,
    via_drill_mm: Option<f64>,
    copper_layers: Option<i64>,
    thickness_mm: Option<f64>,
}

fn value_error(field: PcbField, why: PcbValueError) -> RuleFileError {
    RuleFileError::Value { field, why }
}

impl RuleTemplate {
    /// Parse the text of a rule file.
    pub fn parse(text: &str) -> Result<Self, RuleFileError> {
        if text.len() > MAX_RULE_FILE_BYTES {
            return Err(RuleFileError::TooLarge(MAX_RULE_FILE_BYTES));
        }
        let raw: RawFile =
            toml::from_str(text).map_err(|e| RuleFileError::Syntax(e.message().to_string()))?;
        let pcb = raw.pcb.ok_or(RuleFileError::NoPcbTable)?;
        let need = |field: PcbField, value: Option<f64>| {
            let value = value.ok_or(RuleFileError::Missing(field.key()))?;
            field.check(value).map_err(|why| value_error(field, why))
        };
        let clearance_mm = need(PcbField::Clearance, pcb.clearance_mm)?;
        let track_width_mm = need(PcbField::TrackWidth, pcb.track_width_mm)?;
        let via_diameter_mm = need(PcbField::ViaSize, pcb.via_diameter_mm)?;
        let via_drill_mm = need(PcbField::ViaDrill, pcb.via_drill_mm)?;
        if via_drill_mm >= via_diameter_mm {
            return Err(value_error(
                PcbField::ViaDrill,
                PcbValueError::DrillNotSmallerThanVia,
            ));
        }
        let copper_layers = pcb
            .copper_layers
            .map(|n| {
                u8::try_from(n)
                    .ok()
                    .filter(|n| COPPER_LAYERS.contains(n))
                    .ok_or_else(|| {
                        let (min, max) = PcbField::CopperLayers.range();
                        value_error(
                            PcbField::CopperLayers,
                            PcbValueError::OutOfRange { min, max },
                        )
                    })
            })
            .transpose()?;
        let thickness_mm = pcb
            .thickness_mm
            .map(|t| {
                PcbField::Thickness
                    .check(t)
                    .map_err(|why| value_error(PcbField::Thickness, why))
            })
            .transpose()?;
        Ok(Self {
            name: raw
                .name
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty()),
            clearance_mm,
            track_width_mm,
            via_diameter_mm,
            via_drill_mm,
            copper_layers,
            thickness_mm,
        })
    }
}
