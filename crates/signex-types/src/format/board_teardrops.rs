//! The `[board_options.teardrops]` table of a `.snxpcb`: the board's
//! teardrop rule and its per-pad overrides
//! (`signex_types::pcb_teardrop_rule`). Written only when the rule is not
//! the default; a board carrying one writes `snxpcb/1.12`. The shares are
//! plain numbers, not lengths; a value the rule refuses refuses the file.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::FormatError;
use crate::pcb_teardrop_rule::TeardropRule;

const TEARDROPS: &str = "board_options.teardrops";

/// `[board_options.teardrops]` as stored. A key left out reads as the
/// default rule's.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::format) struct TeardropsRaw {
    pads: bool,
    vias: bool,
    width_share: f64,
    length_share: f64,
    /// Pad uuid to `true` (always) or `false` (never).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pad_overrides: BTreeMap<String, bool>,
}

impl Default for TeardropsRaw {
    fn default() -> Self {
        let rule = TeardropRule::default();
        Self {
            pads: rule.pads,
            vias: rule.vias,
            width_share: rule.width_share,
            length_share: rule.length_share,
            pad_overrides: BTreeMap::new(),
        }
    }
}

fn refused(field: &str, message: String) -> FormatError {
    FormatError::TsvFieldParse {
        block: TEARDROPS.to_string(),
        row: 0,
        field: field.to_string(),
        message,
    }
}

/// The rule of the file; the default when it has no table.
pub(in crate::format) fn read(raw: Option<TeardropsRaw>) -> Result<TeardropRule, FormatError> {
    let Some(raw) = raw else {
        return Ok(TeardropRule::default());
    };
    let pad_overrides = raw
        .pad_overrides
        .into_iter()
        .map(|(key, on)| {
            Uuid::parse_str(&key)
                .map(|pad| (pad, on))
                .map_err(|_| refused("pad_overrides", format!("`{key}` is not a pad uuid")))
        })
        .collect::<Result<_, _>>()?;
    let rule = TeardropRule {
        pads: raw.pads,
        vias: raw.vias,
        width_share: raw.width_share,
        length_share: raw.length_share,
        pad_overrides,
    };
    rule.check()
        .map_err(|why| refused("width_share or length_share", why.to_string()))?;
    Ok(rule)
}

/// The table to write; `None` for the default rule.
pub(in crate::format) fn write(rule: &TeardropRule) -> Result<Option<TeardropsRaw>, FormatError> {
    if rule.is_default() {
        return Ok(None);
    }
    rule.check()
        .map_err(|why| refused("width_share or length_share", why.to_string()))?;
    Ok(Some(TeardropsRaw {
        pads: rule.pads,
        vias: rule.vias,
        width_share: rule.width_share,
        length_share: rule.length_share,
        pad_overrides: rule
            .pad_overrides
            .iter()
            .map(|(pad, on)| (pad.hyphenated().to_string(), *on))
            .collect(),
    }))
}
