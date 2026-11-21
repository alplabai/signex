//! Schematic directives (#2420, #1281, #1489): a Net Blanket, a Compile Mask, a
//! Differential Pair and a Net Parameters directive. They carry no
//! connectivity of their own; the netlist and ERC read them. Design note:
//! `docs/SCHEMATIC_DIRECTIVES.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Aabb, Point};

/// The ERC exclusion that matches every rule.
pub const ALL_ERC_RULES: &str = "*";

/// The suffixes that name the two halves of a differential pair when its
/// class does not pair by every suffix style.
pub const DIFF_PAIR_SUFFIXES: [&str; 2] = ["_P", "_N"];

/// Every suffix style a pair is matched by (`positive`, `negative`, the part
/// of the suffix the pair name keeps): `USB_D_P`/`USB_D_N`,
/// `USB_DP`/`USB_DN` (pair `USB_D`) and `CLK+`/`CLK-`.
pub const DIFF_PAIR_SUFFIX_PAIRS: [(&str, &str, &str); 3] =
    [("_P", "_N", ""), ("DP", "DN", "D"), ("+", "-", "")];

fn is_false(value: &bool) -> bool {
    !*value
}

/// How copper pours treat the nets under a Net Blanket. Stored and shown;
/// the board does not read it yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlanketPour {
    #[default]
    Normal,
    KeepShort,
}

impl BlanketPour {
    pub fn is_normal(&self) -> bool {
        *self == Self::Normal
    }
}

/// The routing rules a Net Parameters directive gives its net, in mm and A.
/// A rule left `None` is not set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NetRules {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearance_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_diameter_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_hole_mm: Option<f64>,
    /// The current the net carries; information only, no rule is made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_a: Option<f64>,
}

impl NetRules {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Whether every set value is a finite number.
    pub fn is_finite(&self) -> bool {
        [
            self.width_mm,
            self.clearance_mm,
            self.via_diameter_mm,
            self.via_hole_mm,
            self.current_a,
        ]
        .into_iter()
        .flatten()
        .all(f64::is_finite)
    }
}

/// One directive placed on a sheet. Saved in `[extras.sheet]` as
/// `directives`, a list of tables tagged by `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchDirective {
    /// A Net Blanket: a drawn region whose parameters apply to every net
    /// inside it: a net class, a track width, how pours treat the nets, and
    /// the ERC rules not reported inside it.
    Blanket {
        uuid: Uuid,
        start: Point,
        end: Point,
        /// The net class every net inside the region joins; `None` sets none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        net_class: Option<String>,
        /// ERC rule ids (`builtin::unused_pin`) not reported inside the
        /// region; [`ALL_ERC_RULES`] matches every rule.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        erc_exclusions: Vec<String>,
        /// The track width, in mm, of the nets it catches.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        track_width: Option<f64>,
        #[serde(default, skip_serializing_if = "BlanketPour::is_normal")]
        pour: BlanketPour,
        /// Whether a net whose wires only cross the edge is caught too.
        #[serde(default, skip_serializing_if = "is_false")]
        include_edge_nets: bool,
    },
    /// A region whose contents ERC and the netlist skip.
    CompileMask {
        uuid: Uuid,
        start: Point,
        end: Point,
    },
    /// Marks the net it sits on and its partner (`X_P` with `X_N`) as one
    /// differential pair, of the diff-pair class `class` when set.
    DiffPair {
        uuid: Uuid,
        position: Point,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        class: Option<String>,
    },
    /// A flag on one wire giving its net a net class, a diff-pair class and
    /// routing rules.
    NetParameters {
        uuid: Uuid,
        position: Point,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        net_class: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        diff_pair_class: Option<String>,
        #[serde(default, skip_serializing_if = "NetRules::is_empty")]
        rules: NetRules,
        /// Whether the flag lists the class and the rule values.
        #[serde(default, skip_serializing_if = "is_false")]
        show_on_flag: bool,
    },
}

/// Which directive a [`SchDirective`] is, without its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectiveKind {
    Blanket,
    CompileMask,
    DiffPair,
    NetParameters,
}

impl DirectiveKind {
    /// The name the file, the operands and the agent results use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blanket => "blanket",
            Self::CompileMask => "compile_mask",
            Self::DiffPair => "diff_pair",
            Self::NetParameters => "net_parameters",
        }
    }
}

/// A class name trimmed, `None` when blank.
fn trimmed(class: &Option<String>) -> Option<&str> {
    class.as_deref().map(str::trim).filter(|c| !c.is_empty())
}

impl SchDirective {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Blanket { uuid, .. }
            | Self::CompileMask { uuid, .. }
            | Self::DiffPair { uuid, .. }
            | Self::NetParameters { uuid, .. } => *uuid,
        }
    }

    pub fn set_uuid(&mut self, id: Uuid) {
        match self {
            Self::Blanket { uuid, .. }
            | Self::CompileMask { uuid, .. }
            | Self::DiffPair { uuid, .. }
            | Self::NetParameters { uuid, .. } => *uuid = id,
        }
    }

    pub fn kind(&self) -> DirectiveKind {
        match self {
            Self::Blanket { .. } => DirectiveKind::Blanket,
            Self::CompileMask { .. } => DirectiveKind::CompileMask,
            Self::DiffPair { .. } => DirectiveKind::DiffPair,
            Self::NetParameters { .. } => DirectiveKind::NetParameters,
        }
    }

    /// The region of a Blanket or Compile Mask; `None` for a point directive.
    pub fn region(&self) -> Option<Aabb> {
        match self {
            Self::Blanket { start, end, .. } | Self::CompileMask { start, end, .. } => {
                Some(Aabb::new(start.x, start.y, end.x, end.y))
            }
            Self::DiffPair { .. } | Self::NetParameters { .. } => None,
        }
    }

    /// Whether `p` is inside the region, its edge included. A point
    /// directive contains nothing.
    pub fn region_contains(&self, p: Point) -> bool {
        self.region().is_some_and(|r| r.contains(p.x, p.y))
    }

    /// The point a move or a turn keeps the directive around: the centre of
    /// a region, the position of a point directive.
    pub fn anchor(&self) -> Point {
        match self {
            Self::Blanket { start, end, .. } | Self::CompileMask { start, end, .. } => {
                Point::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5)
            }
            Self::DiffPair { position, .. } | Self::NetParameters { position, .. } => *position,
        }
    }

    /// Every stored point mapped through `f`: a move, a turn or a mirror.
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        match self {
            Self::Blanket { start, end, .. } | Self::CompileMask { start, end, .. } => {
                *start = f(*start);
                *end = f(*end);
            }
            Self::DiffPair { position, .. } | Self::NetParameters { position, .. } => {
                *position = f(*position)
            }
        }
    }

    /// Whether this is a Blanket that does not report `rule_id` inside it.
    pub fn excludes_rule(&self, rule_id: &str) -> bool {
        match self {
            Self::Blanket { erc_exclusions, .. } => erc_exclusions
                .iter()
                .any(|r| r == rule_id || r == ALL_ERC_RULES),
            _ => false,
        }
    }

    /// The net class of a Blanket or a Net Parameters directive, trimmed,
    /// when it sets one.
    pub fn net_class(&self) -> Option<&str> {
        match self {
            Self::Blanket { net_class, .. } | Self::NetParameters { net_class, .. } => {
                trimmed(net_class)
            }
            _ => None,
        }
    }

    /// The diff-pair class a Differential Pair or Net Parameters directive
    /// names, trimmed, when it names one.
    pub fn diff_pair_class(&self) -> Option<&str> {
        match self {
            Self::DiffPair { class, .. } => trimmed(class),
            Self::NetParameters {
                diff_pair_class, ..
            } => trimmed(diff_pair_class),
            _ => None,
        }
    }

    /// Whether a sheet holding this directive needs `snxsch/1.3`: a Net
    /// Parameters directive, a Blanket using a field added with it, or a
    /// Differential Pair naming a class. An older build would drop those.
    pub fn needs_format_1_3(&self) -> bool {
        match self {
            Self::NetParameters { .. } => true,
            Self::Blanket {
                track_width,
                pour,
                include_edge_nets,
                ..
            } => track_width.is_some() || !pour.is_normal() || *include_edge_nets,
            Self::DiffPair { class, .. } => class.is_some(),
            Self::CompileMask { .. } => false,
        }
    }
}

/// The base name of a differential pair net (`USB_D` from `USB_D_P` or
/// `USB_D_N`); `None` when `net` carries neither suffix or nothing before it.
pub fn diff_pair_base(net: &str) -> Option<&str> {
    DIFF_PAIR_SUFFIXES
        .iter()
        .find_map(|suffix| net.strip_suffix(suffix))
        .filter(|base| !base.is_empty())
}

/// The two halves of a differential pair, named by a suffix style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffPairHalves {
    /// The pair's name: `USB_D` from `USB_D_P`, `USB_DP` or `USB_D+`.
    pub name: String,
    pub positive: String,
    pub negative: String,
}

/// The pair `net` belongs to by its suffix. `every_style` matches every
/// style of [`DIFF_PAIR_SUFFIX_PAIRS`]; otherwise only `_P` / `_N`. `None`
/// when `net` carries no suffix or nothing before it.
pub fn diff_pair_halves(net: &str, every_style: bool) -> Option<DiffPairHalves> {
    let styles = if every_style {
        &DIFF_PAIR_SUFFIX_PAIRS[..]
    } else {
        &DIFF_PAIR_SUFFIX_PAIRS[..1]
    };
    styles.iter().find_map(|(pos, neg, keep)| {
        let base = net
            .strip_suffix(pos)
            .or_else(|| net.strip_suffix(neg))
            .filter(|base| !base.is_empty())?;
        Some(DiffPairHalves {
            name: format!("{base}{keep}"),
            positive: format!("{base}{pos}"),
            negative: format!("{base}{neg}"),
        })
    })
}

/// The ERC rule ids of an operand or a form field: comma separated, blanks
/// dropped, each kept once in the order given.
pub fn parse_erc_exclusions(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in text.split(',').map(str::trim).filter(|id| !id.is_empty()) {
        if !out.iter().any(|seen| seen == id) {
            out.push(id.to_string());
        }
    }
    out
}
