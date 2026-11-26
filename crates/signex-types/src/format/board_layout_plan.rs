//! `[board_options.layout_plan]` of a `.snxpcb` (#2942): the board's layout
//! plan (`pcb_layout_plan`), one sub-table per stage that sets anything, each
//! setting under its `layout_plan_set` operand name.
//!
//! Written only when the plan sets something, and such a board writes
//! `snxpcb/1.12` (`format::snxpcb_token`): a build that knows only 1.11
//! would drop the plan on save, so it opens the board read-only. A board
//! without a plan keeps its token, byte for byte. A plan that does not read
//! (an unknown priority, a weight below 0, a pour that is no uuid, a name
//! listed twice) refuses the file, naming the setting.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::FormatError;
use crate::pcb_layout_plan::{
    CheckPlan, LayoutPlan, PlacementPlan, PlacementPriority, PourPlan, RouteTarget, RoutingPlan,
};

const BLOCK: &str = "board_options.layout_plan";

/// The plan as stored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct LayoutPlanRaw {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    placement: Option<PlacementRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    routing: Option<RoutingRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pours: Option<PoursRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checks: Option<ChecksRaw>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PlacementRaw {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rooms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keep: Vec<String>,
    /// A priority token to its weight.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    weights: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RoutingRaw {
    /// Net names and `class:<name>` entries, in routing order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    nets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skip: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PoursRaw {
    /// Pour uuids in fill order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pours: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ChecksRaw {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    requirements: Vec<String>,
}

fn refused(field: &str, message: impl Into<String>) -> FormatError {
    FormatError::TsvFieldParse {
        block: BLOCK.to_string(),
        row: 0,
        field: field.to_string(),
        message: message.into(),
    }
}

/// The plan of the file; the empty plan when it has none.
pub(in crate::format) fn read(raw: Option<LayoutPlanRaw>) -> Result<LayoutPlan, FormatError> {
    let Some(raw) = raw else {
        return Ok(LayoutPlan::default());
    };
    let placement = raw.placement.unwrap_or_default();
    let routing = raw.routing.unwrap_or_default();
    let weights = placement
        .weights
        .into_iter()
        .map(|(token, weight)| {
            let priority = PlacementPriority::from_token(&token)
                .ok_or_else(|| refused("weights", format!("`{token}` is not a priority")))?;
            Ok((priority, weight))
        })
        .collect::<Result<_, FormatError>>()?;
    let nets = routing
        .nets
        .iter()
        .map(|text| RouteTarget::parse(text).map_err(|e| refused("nets", e.0)))
        .collect::<Result<_, _>>()?;
    let pours = raw
        .pours
        .unwrap_or_default()
        .pours
        .iter()
        .map(|text| {
            Uuid::parse_str(text).map_err(|_| refused("pours", format!("`{text}` is not a uuid")))
        })
        .collect::<Result<_, _>>()?;
    let checks = raw.checks.unwrap_or_default();
    let plan = LayoutPlan {
        placement: PlacementPlan {
            weights,
            rooms: placement.rooms,
            keep: placement.keep,
        },
        routing: RoutingPlan {
            nets,
            skip: routing.skip,
        },
        pours: PourPlan { pours },
        checks: CheckPlan {
            rules: checks.rules,
            requirements: checks.requirements,
        },
    };
    plan.check().map_err(|e| refused("layout_plan", e.0))?;
    Ok(plan)
}

/// The plan as stored; `None` for an empty plan. A plan `set` could not
/// have made (a weight that is not a finite number of 0 or more) is refused
/// before anything is written.
pub(in crate::format) fn write(plan: &LayoutPlan) -> Result<Option<LayoutPlanRaw>, FormatError> {
    if plan.is_empty() {
        return Ok(None);
    }
    plan.check().map_err(|e| refused("layout_plan", e.0))?;
    let placement = &plan.placement;
    let routing = &plan.routing;
    let checks = &plan.checks;
    Ok(Some(LayoutPlanRaw {
        placement: (*placement != PlacementPlan::default()).then(|| PlacementRaw {
            rooms: placement.rooms.clone(),
            keep: placement.keep.clone(),
            weights: placement
                .weights
                .iter()
                .map(|(priority, weight)| (priority.token().to_string(), *weight))
                .collect(),
        }),
        routing: (*routing != RoutingPlan::default()).then(|| RoutingRaw {
            nets: routing.nets.iter().map(RouteTarget::token).collect(),
            skip: routing.skip.clone(),
        }),
        pours: (!plan.pours.pours.is_empty()).then(|| PoursRaw {
            pours: plan.pours.pours.iter().map(Uuid::to_string).collect(),
        }),
        checks: (*checks != CheckPlan::default()).then(|| ChecksRaw {
            rules: checks.rules.clone(),
            requirements: checks.requirements.clone(),
        }),
    }))
}
