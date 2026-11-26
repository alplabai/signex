//! The board's layout plan (#2942): the settings of the layout stages in
//! their order (placement, routing, pours, checks), saved with the board in
//! `[board_options.layout_plan]` and changed only through the board engine's
//! `SetBoardOptions` command, one undo step per change. A stage with nothing
//! set leaves the commands it feeds to their own defaults. The commands, the
//! file form and how a stage counts as done: `docs/CLI.md`, "Layout plan".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Why a setting of the plan was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct LayoutPlanError(pub String);

fn refused<T>(message: impl Into<String>) -> Result<T, LayoutPlanError> {
    Err(LayoutPlanError(message.into()))
}

/// A stage of the layout plan, in the order a design runs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutStage {
    Placement,
    Routing,
    Pours,
    Checks,
}

impl LayoutStage {
    pub const ALL: [LayoutStage; 4] = [
        LayoutStage::Placement,
        LayoutStage::Routing,
        LayoutStage::Pours,
        LayoutStage::Checks,
    ];

    pub const fn token(self) -> &'static str {
        match self {
            LayoutStage::Placement => "placement",
            LayoutStage::Routing => "routing",
            LayoutStage::Pours => "pours",
            LayoutStage::Checks => "checks",
        }
    }

    pub fn from_token(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|stage| stage.token() == text.trim())
    }

    /// The settings of this stage, in the order they are listed.
    pub fn settings(self) -> impl Iterator<Item = LayoutSetting> {
        LayoutSetting::ALL
            .into_iter()
            .filter(move |setting| setting.stage() == self)
    }
}

/// One setting of a stage, named by its `layout_plan_set` operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutSetting {
    /// Placement: the weight of each auto-place priority.
    Weights,
    /// Placement: the rooms in scope, by name.
    Rooms,
    /// Placement: the parts left where they are, by designator.
    Keep,
    /// Routing: the nets and net classes to route, in order.
    Nets,
    /// Routing: the nets left unrouted.
    Skip,
    /// Pours: the pours to fill, by uuid, in fill order.
    Pours,
    /// Checks: the DRC rule ids that must pass.
    Rules,
    /// Checks: the project requirement ids that must pass.
    Requirements,
}

impl LayoutSetting {
    pub const ALL: [LayoutSetting; 8] = [
        LayoutSetting::Weights,
        LayoutSetting::Rooms,
        LayoutSetting::Keep,
        LayoutSetting::Nets,
        LayoutSetting::Skip,
        LayoutSetting::Pours,
        LayoutSetting::Rules,
        LayoutSetting::Requirements,
    ];

    pub const fn operand(self) -> &'static str {
        match self {
            LayoutSetting::Weights => "weights",
            LayoutSetting::Rooms => "rooms",
            LayoutSetting::Keep => "keep",
            LayoutSetting::Nets => "nets",
            LayoutSetting::Skip => "skip",
            LayoutSetting::Pours => "pours",
            LayoutSetting::Rules => "rules",
            LayoutSetting::Requirements => "requirements",
        }
    }

    pub const fn stage(self) -> LayoutStage {
        match self {
            LayoutSetting::Weights | LayoutSetting::Rooms | LayoutSetting::Keep => {
                LayoutStage::Placement
            }
            LayoutSetting::Nets | LayoutSetting::Skip => LayoutStage::Routing,
            LayoutSetting::Pours => LayoutStage::Pours,
            LayoutSetting::Rules | LayoutSetting::Requirements => LayoutStage::Checks,
        }
    }
}

/// A priority auto-place weighs (#2941).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementPriority {
    /// Parts that share nets sit close together.
    Connection,
    /// Hot parts keep their distance.
    Thermal,
    /// Noisy parts keep away from sensitive ones.
    Separation,
    /// Connectors sit on the board edge.
    Edges,
}

impl PlacementPriority {
    pub const ALL: [PlacementPriority; 4] = [
        PlacementPriority::Connection,
        PlacementPriority::Thermal,
        PlacementPriority::Separation,
        PlacementPriority::Edges,
    ];

    pub const fn token(self) -> &'static str {
        match self {
            PlacementPriority::Connection => "connection",
            PlacementPriority::Thermal => "thermal",
            PlacementPriority::Separation => "separation",
            PlacementPriority::Edges => "edges",
        }
    }

    pub fn from_token(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|priority| priority.token() == text.trim())
    }
}

/// The text before a net class's name in a routing list.
pub const CLASS_PREFIX: &str = "class:";

/// What the routing stage routes: a net by name, or every net of a class.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum RouteTarget {
    Net(String),
    Class(String),
}

impl RouteTarget {
    /// `GND`, or `class:Power` for a class.
    pub fn token(&self) -> String {
        match self {
            RouteTarget::Net(name) => name.clone(),
            RouteTarget::Class(name) => format!("{CLASS_PREFIX}{name}"),
        }
    }

    pub fn parse(text: &str) -> Result<Self, LayoutPlanError> {
        let text = text.trim();
        let target = match text.strip_prefix(CLASS_PREFIX) {
            Some(class) => RouteTarget::Class(class.trim().to_string()),
            None => RouteTarget::Net(text.to_string()),
        };
        match &target {
            RouteTarget::Net(name) | RouteTarget::Class(name) if name.is_empty() => {
                refused(format!("`{text}` names no net or class"))
            }
            _ => Ok(target),
        }
    }
}

impl From<RouteTarget> for String {
    fn from(target: RouteTarget) -> Self {
        target.token()
    }
}

impl TryFrom<String> for RouteTarget {
    type Error = LayoutPlanError;

    fn try_from(text: String) -> Result<Self, LayoutPlanError> {
        RouteTarget::parse(&text)
    }
}

/// The placement stage: what auto-place weighs and where it works.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PlacementPlan {
    /// A priority left out keeps auto-place's own weight.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub weights: BTreeMap<PlacementPriority, f64>,
    /// The rooms whose parts are placed; none places every part.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rooms: Vec<String>,
    /// Designators of the parts left where they are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<String>,
}

/// The routing stage: which nets, in which order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RoutingPlan {
    /// Routed in this order; none routes every net.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nets: Vec<RouteTarget>,
    /// Net names left unrouted, whatever `nets` says.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
}

/// The pours stage: which pours to fill, in fill order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PourPlan {
    /// None fills every pour that is not shelved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pours: Vec<Uuid>,
}

/// The checks stage: what must pass before the layout counts as done.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CheckPlan {
    /// DRC rule ids (`builtin::clearance`, `user::<name>`); none is every rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    /// Ids of the project's requirements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<String>,
}

/// The whole plan. The default, an empty plan, is what a board saved
/// without one has, and it writes nothing.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LayoutPlan {
    #[serde(default, skip_serializing_if = "is_default")]
    pub placement: PlacementPlan,
    #[serde(default, skip_serializing_if = "is_default")]
    pub routing: RoutingPlan,
    #[serde(default, skip_serializing_if = "is_default")]
    pub pours: PourPlan,
    #[serde(default, skip_serializing_if = "is_default")]
    pub checks: CheckPlan,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// A DRC rule id: `builtin::<kind>` or `user::<name>`.
pub fn is_rule_id_shape(id: &str) -> bool {
    ["builtin::", "user::"]
        .iter()
        .any(|prefix| id.strip_prefix(prefix).is_some_and(|rest| !rest.is_empty()))
}

/// A weight auto-place can use: a finite number, zero or more.
pub fn is_weight(weight: f64) -> bool {
    weight.is_finite() && weight >= 0.0
}

/// The comma-separated entries of `text`, trimmed; empty entries dropped.
fn entries(text: &str) -> Vec<&str> {
    text.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// `items` with every entry once; an entry given twice is refused.
fn unique<T: PartialEq>(
    items: Vec<T>,
    name: impl Fn(&T) -> String,
) -> Result<Vec<T>, LayoutPlanError> {
    for (at, item) in items.iter().enumerate() {
        if items[..at].contains(item) {
            return refused(format!("`{}` is listed twice", name(item)));
        }
    }
    Ok(items)
}

fn names(text: &str) -> Result<Vec<String>, LayoutPlanError> {
    let list = entries(text).into_iter().map(str::to_string).collect();
    unique(list, Clone::clone)
}

/// `connection:3,edges:5`: a weight per priority.
pub fn parse_weights(text: &str) -> Result<BTreeMap<PlacementPriority, f64>, LayoutPlanError> {
    let mut weights = BTreeMap::new();
    for entry in entries(text) {
        let Some((name, value)) = entry.split_once(':') else {
            return refused(format!("`{entry}` is not <priority>:<weight>"));
        };
        let Some(priority) = PlacementPriority::from_token(name) else {
            let known: Vec<&str> = PlacementPriority::ALL.iter().map(|p| p.token()).collect();
            return refused(format!(
                "`{}` is not a priority (one of {})",
                name.trim(),
                known.join(", ")
            ));
        };
        let weight = value.trim().parse::<f64>().ok().filter(|w| is_weight(*w));
        let Some(weight) = weight else {
            return refused(format!(
                "the weight of `{}` is not a number, 0 or more",
                priority.token()
            ));
        };
        if weights.insert(priority, weight).is_some() {
            return refused(format!("`{}` is listed twice", priority.token()));
        }
    }
    Ok(weights)
}

fn route_targets(text: &str) -> Result<Vec<RouteTarget>, LayoutPlanError> {
    let list = entries(text)
        .into_iter()
        .map(RouteTarget::parse)
        .collect::<Result<Vec<_>, _>>()?;
    unique(list, RouteTarget::token)
}

fn pour_ids(text: &str) -> Result<Vec<Uuid>, LayoutPlanError> {
    let list = entries(text)
        .into_iter()
        .map(|entry| {
            Uuid::parse_str(entry)
                .map_err(|_| LayoutPlanError(format!("`{entry}` is not a pour uuid")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    unique(list, Uuid::to_string)
}

fn rule_ids(text: &str) -> Result<Vec<String>, LayoutPlanError> {
    let list = names(text)?;
    match list.iter().find(|id| !is_rule_id_shape(id)) {
        Some(id) => refused(format!(
            "`{id}` is not a DRC rule id (builtin::<kind> or user::<name>)"
        )),
        None => Ok(list),
    }
}

impl LayoutPlan {
    /// Whether the plan sets nothing: the board writes no plan.
    pub fn is_empty(&self) -> bool {
        is_default(self)
    }

    /// Whether `stage` sets nothing, so its commands use their defaults.
    pub fn stage_is_empty(&self, stage: LayoutStage) -> bool {
        match stage {
            LayoutStage::Placement => is_default(&self.placement),
            LayoutStage::Routing => is_default(&self.routing),
            LayoutStage::Pours => is_default(&self.pours),
            LayoutStage::Checks => is_default(&self.checks),
        }
    }

    /// Clear every setting of `stage`; whether anything changed.
    pub fn clear(&mut self, stage: LayoutStage) -> bool {
        let had = !self.stage_is_empty(stage);
        match stage {
            LayoutStage::Placement => self.placement = PlacementPlan::default(),
            LayoutStage::Routing => self.routing = RoutingPlan::default(),
            LayoutStage::Pours => self.pours = PourPlan::default(),
            LayoutStage::Checks => self.checks = CheckPlan::default(),
        }
        had
    }

    /// Set `setting` from its operand text: a comma list (`weights` takes
    /// `<priority>:<weight>` entries, `nets` takes `class:<name>` for a net
    /// class); empty text clears it. Whether anything changed. Text that
    /// does not parse, or lists an entry twice, is refused and changes
    /// nothing.
    pub fn set(&mut self, setting: LayoutSetting, text: &str) -> Result<bool, LayoutPlanError> {
        let before = self.clone();
        match setting {
            LayoutSetting::Weights => self.placement.weights = parse_weights(text)?,
            LayoutSetting::Rooms => self.placement.rooms = names(text)?,
            LayoutSetting::Keep => self.placement.keep = names(text)?,
            LayoutSetting::Nets => self.routing.nets = route_targets(text)?,
            LayoutSetting::Skip => self.routing.skip = names(text)?,
            LayoutSetting::Pours => self.pours.pours = pour_ids(text)?,
            LayoutSetting::Rules => self.checks.rules = rule_ids(text)?,
            LayoutSetting::Requirements => self.checks.requirements = names(text)?,
        }
        Ok(*self != before)
    }

    /// The setting as `set` takes it back: the inverse of [`Self::set`].
    pub fn setting_text(&self, setting: LayoutSetting) -> String {
        match setting {
            LayoutSetting::Weights => self
                .placement
                .weights
                .iter()
                .map(|(priority, weight)| format!("{}:{weight}", priority.token()))
                .collect::<Vec<_>>()
                .join(","),
            LayoutSetting::Rooms => self.placement.rooms.join(","),
            LayoutSetting::Keep => self.placement.keep.join(","),
            LayoutSetting::Nets => self
                .routing
                .nets
                .iter()
                .map(RouteTarget::token)
                .collect::<Vec<_>>()
                .join(","),
            LayoutSetting::Skip => self.routing.skip.join(","),
            LayoutSetting::Pours => self
                .pours
                .pours
                .iter()
                .map(Uuid::to_string)
                .collect::<Vec<_>>()
                .join(","),
            LayoutSetting::Rules => self.checks.rules.join(","),
            LayoutSetting::Requirements => self.checks.requirements.join(","),
        }
    }

    /// Whether the plan holds only what [`Self::set`] accepts: finite
    /// weights of 0 or more, rule ids of the right shape, no name, net,
    /// pour or id listed twice. A file that breaks this is refused on load.
    pub fn check(&self) -> Result<(), LayoutPlanError> {
        let mut copy = LayoutPlan::default();
        for setting in LayoutSetting::ALL {
            copy.set(setting, &self.setting_text(setting))?;
        }
        if copy == *self {
            return Ok(());
        }
        // A name with a comma, spaces at its ends or none at all reads back
        // as something else.
        refused("a name in the layout plan is empty, holds a comma or has spaces at its ends")
    }
}

#[cfg(test)]
#[path = "pcb_layout_plan_tests.rs"]
mod tests;
