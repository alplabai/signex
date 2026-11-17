//! Schematic directives (#2420): a Blanket, a Compile Mask and a Differential
//! Pair directive. They carry no connectivity of their own; the netlist and
//! ERC read them. Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Aabb, Point};

/// The ERC exclusion that matches every rule.
pub const ALL_ERC_RULES: &str = "*";

/// The suffixes that name the two halves of a differential pair.
pub const DIFF_PAIR_SUFFIXES: [&str; 2] = ["_P", "_N"];

/// One directive placed on a sheet. Saved in `[extras.sheet]` as
/// `directives`, a list of tables tagged by `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchDirective {
    /// A drawn region whose parameters apply to every net inside it: a net
    /// class and the ERC rules not reported inside it.
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
    },
    /// A region whose contents ERC and the netlist skip.
    CompileMask {
        uuid: Uuid,
        start: Point,
        end: Point,
    },
    /// Marks the net it sits on and its partner (`X_P` with `X_N`) as one
    /// differential pair.
    DiffPair { uuid: Uuid, position: Point },
}

/// Which directive a [`SchDirective`] is, without its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectiveKind {
    Blanket,
    CompileMask,
    DiffPair,
}

impl DirectiveKind {
    /// The name the file, the operands and the agent results use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blanket => "blanket",
            Self::CompileMask => "compile_mask",
            Self::DiffPair => "diff_pair",
        }
    }
}

impl SchDirective {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Blanket { uuid, .. }
            | Self::CompileMask { uuid, .. }
            | Self::DiffPair { uuid, .. } => *uuid,
        }
    }

    pub fn set_uuid(&mut self, id: Uuid) {
        match self {
            Self::Blanket { uuid, .. }
            | Self::CompileMask { uuid, .. }
            | Self::DiffPair { uuid, .. } => *uuid = id,
        }
    }

    pub fn kind(&self) -> DirectiveKind {
        match self {
            Self::Blanket { .. } => DirectiveKind::Blanket,
            Self::CompileMask { .. } => DirectiveKind::CompileMask,
            Self::DiffPair { .. } => DirectiveKind::DiffPair,
        }
    }

    /// The region of a Blanket or Compile Mask; `None` for a point directive.
    pub fn region(&self) -> Option<Aabb> {
        match self {
            Self::Blanket { start, end, .. } | Self::CompileMask { start, end, .. } => {
                Some(Aabb::new(start.x, start.y, end.x, end.y))
            }
            Self::DiffPair { .. } => None,
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
            Self::DiffPair { position, .. } => *position,
        }
    }

    /// Every stored point mapped through `f`: a move, a turn or a mirror.
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        match self {
            Self::Blanket { start, end, .. } | Self::CompileMask { start, end, .. } => {
                *start = f(*start);
                *end = f(*end);
            }
            Self::DiffPair { position, .. } => *position = f(*position),
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

    /// The net class of a Blanket, trimmed, when it sets one.
    pub fn net_class(&self) -> Option<&str> {
        match self {
            Self::Blanket { net_class, .. } => net_class
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty()),
            _ => None,
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
