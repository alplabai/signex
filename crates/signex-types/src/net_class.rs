//! Project-level net classes.
//!
//! A net class groups nets by name and carries the style the schematic draws
//! them with. The table is an ordered `Vec<NetClass>` stored under the
//! `net_classes` key of the `.snxprj` ([`crate::project::ProjectData`]).
//!
//! # Membership
//!
//! A class lists member nets two ways, and a net belongs to the class when
//! either matches:
//!
//! - [`NetClass::nets`] — exact net names, compared case-sensitively;
//! - [`NetClass::patterns`] — glob patterns over the net name. Two wildcards
//!   only: `*` matches any run of characters (including none) and `?`
//!   matches exactly one. Everything else is literal and the whole name must
//!   match, so `VCC*` matches `VCC` and `VCC_3V3` but not `AVCC`.
//!
//! # Precedence
//!
//! When a net matches several classes, **the first matching class in table
//! order wins** ([`first_matching_class`]). Reorder the table to change the
//! winner; there is no specificity ranking.
//!
//! # PCB extension point
//!
//! The PCB rule fields ([`NetClass::clearance`] and the rest of the
//! `f64` millimetre block) live on this same struct so the PCB editor reads
//! the same table instead of growing a second model. `0.0` means "unset" and
//! is not written. Adding a PCB rule is one more defaulted,
//! `skip_serializing_if`-guarded field here; no format version change.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::project::UnknownJson;
use crate::theme::Color;

/// The pick-list entry that stands for "no class" in a net label's Net Class
/// picker. A class cannot take this name, so a pick is never ambiguous.
pub const NO_CLASS_ENTRY: &str = "(none)";

/// How a net's wires are stroked on the schematic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetLineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl NetLineStyle {
    pub const ALL: [NetLineStyle; 3] = [Self::Solid, Self::Dashed, Self::Dotted];

    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Dashed => "Dashed",
            Self::Dotted => "Dotted",
        }
    }
}

impl std::fmt::Display for NetLineStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The schematic look of a net class. Every field is optional/neutral so an
/// empty style changes nothing about how a wire is drawn.
///
/// Unlike [`NetClass`] this carries no unknown-key bag: the struct is `Copy`
/// and a bag of JSON values cannot be. A key a newer build adds inside
/// `style` is dropped on save, so a rule that must survive older builds goes
/// on [`NetClass`] itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct NetClassStyle {
    /// Wire colour. `None` keeps the theme's wire colour. Stored as
    /// `"#RRGGBB"` / `"#RRGGBBAA"`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "hex_color_opt"
    )]
    pub color: Option<Color>,
    /// Wire width in millimetres. `None` keeps the wire's own width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_width_mm: Option<f64>,
    /// Line style. [`NetLineStyle::Solid`] is the default and is not written.
    #[serde(default, skip_serializing_if = "is_solid")]
    pub line: NetLineStyle,
}

fn is_solid(line: &NetLineStyle) -> bool {
    *line == NetLineStyle::Solid
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

impl NetClassStyle {
    /// `true` when the style changes nothing (written as absent).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// One entry of the project's net-class table. See the module docs for the
/// membership and precedence rules.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NetClass {
    pub name: String,
    /// Exact member net names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nets: Vec<String>,
    /// Glob patterns (`*`, `?`) over net names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<String>,
    /// Schematic style applied to member nets' wires.
    #[serde(default, skip_serializing_if = "NetClassStyle::is_default")]
    pub style: NetClassStyle,
    // PCB rule block (mm). 0.0 = unset; see the module docs.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub clearance: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trace_width: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub via_diameter: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub via_drill: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub diff_pair_gap: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub diff_pair_width: f64,
    /// Keys written by a newer build, kept so a save does not drop them.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl NetClass {
    /// A new, empty class with the given name.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Whether `net_name` is a member, by exact name or by pattern.
    pub fn contains_net(&self, net_name: &str) -> bool {
        self.nets.iter().any(|n| n == net_name)
            || self.patterns.iter().any(|p| glob_matches(p, net_name))
    }
}

/// The class a net belongs to: the first entry of `classes` (table order)
/// that contains `net_name`.
pub fn first_matching_class<'a>(classes: &'a [NetClass], net_name: &str) -> Option<&'a NetClass> {
    classes.iter().find(|c| c.contains_net(net_name))
}

/// Match `text` against a `*` / `?` glob. Iterative with single backtrack
/// point, so a hostile pattern cannot go exponential.
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || (p[pi] != '*' && p[pi] == t[ti])) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Parse `#RRGGBB` / `#RRGGBBAA` (leading `#` optional). `None` on anything
/// malformed — unlike `Color::from_hex`, never panics on short input.
pub fn parse_hex_color(text: &str) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    if !(hex.len() == 6 || hex.len() == 8) || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let a = if hex.len() == 8 { byte(6)? } else { 255 };
    Some(Color::new(byte(0)?, byte(2)?, byte(4)?, a))
}

/// `#RRGGBB`, or `#RRGGBBAA` when not opaque.
pub fn format_hex_color(color: Color) -> String {
    if color.a == 255 {
        format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.r, color.g, color.b, color.a
        )
    }
}

mod hex_color_opt {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Option<Color>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(c) => s.serialize_some(&format_hex_color(*c)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Color>, D::Error> {
        let text = Option::<String>::deserialize(d)?;
        match text {
            None => Ok(None),
            Some(t) => parse_hex_color(&t)
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("invalid colour {t:?}"))),
        }
    }
}

#[cfg(test)]
mod tests;
