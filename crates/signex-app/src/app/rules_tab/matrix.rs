//! The class-to-class clearance matrix in the app (#2398): what a cell
//! holds or falls back to, and the one edit path the tab's cells and
//! `rules_matrix_set` share. The cells are rules of the text
//! (`signex_drc_dsl::table::matrix`); the fallbacks are resolved by
//! `signex-drc`. Behaviour: `docs/DRC_RULES.md`, "Class matrix".

use std::collections::BTreeMap;

use serde_json::{Value, json};
use signex_drc::{CopperObject, DrcRules, WrittenRuleEdit};
use signex_drc_dsl::table::matrix::{self, CellChange, CellKey, MatrixCell, OBJECT_PAIRS};
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;

use super::commands::{Ran, answered, project_text, refused, text};
use super::json::mm;
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::state::ProjectId;

/// Where a fallback is kept: the two classes in name order, and the index
/// of the object pair in [`OBJECT_PAIRS`] (`None` for the whole cell).
pub(crate) type DefaultKey = (String, String, Option<usize>);

/// What each cell falls back to without its rule, in nm.
pub(crate) type MatrixDefaults = BTreeMap<DefaultKey, i64>;

/// `key`'s place among the fallbacks.
pub(crate) fn default_key(key: &CellKey) -> DefaultKey {
    let (a, b) = if key.a <= key.b {
        (key.a.clone(), key.b.clone())
    } else {
        (key.b.clone(), key.a.clone())
    };
    let pair = key.pair.as_ref().and_then(|[x, y]| {
        OBJECT_PAIRS.iter().position(|[p, q]| {
            (x.as_str() == *p && y.as_str() == *q) || (x.as_str() == *q && y.as_str() == *p)
        })
    });
    (a, b, pair)
}

fn copper(word: &str) -> Option<CopperObject> {
    match word {
        "track" => Some(CopperObject::Track),
        "pad" => Some(CopperObject::Pad),
        "via" => Some(CopperObject::Via),
        _ => None,
    }
}

/// Each cell's fallback on the open board: what a net of one class and a
/// net of the other need with no class-pair rule, for the whole cell and
/// each object pair. A class with no net on the board has none.
pub(crate) fn board_defaults(
    rules: &DrcRules,
    board: &PcbBoard,
    classes: &[NetClass],
) -> MatrixDefaults {
    let sample = |class: &NetClass| {
        board
            .nets
            .iter()
            .find(|n| first_matching_class(classes, &n.name).is_some_and(|c| c.name == class.name))
            .map(|n| n.number)
    };
    let nets: Vec<(&str, Option<u32>)> = classes
        .iter()
        .map(|c| (c.name.as_str(), sample(c)))
        .collect();
    let mut out = MatrixDefaults::new();
    for (i, &(a, net_a)) in nets.iter().enumerate() {
        for &(b, net_b) in &nets[i..] {
            let (Some(net_a), Some(net_b)) = (net_a, net_b) else {
                continue;
            };
            let (x, y) = if a <= b { (a, b) } else { (b, a) };
            let whole = rules.pair_default(net_a, net_b, [None, None]);
            out.insert((x.to_string(), y.to_string(), None), whole);
            for (at, [p, q]) in OBJECT_PAIRS.iter().enumerate() {
                let objects = [copper(p), copper(q)];
                let value = rules.pair_default(net_a, net_b, objects);
                out.insert((x.to_string(), y.to_string(), Some(at)), value);
            }
        }
    }
    out
}

/// Each cell's fallback with no board open: the larger of the two
/// classes' clearances, where either sets one.
pub(crate) fn class_defaults(classes: &[NetClass]) -> MatrixDefaults {
    let mut out = MatrixDefaults::new();
    for (i, a) in classes.iter().enumerate() {
        for b in &classes[i..] {
            let most = a.clearance.max(b.clearance);
            let Ok(nm) = pcb_geom::mm_to_nm(most, "class clearance") else {
                continue;
            };
            if nm <= 0 {
                continue;
            }
            let (x, y) = if a.name <= b.name {
                (&a.name, &b.name)
            } else {
                (&b.name, &a.name)
            };
            out.insert((x.clone(), y.clone(), None), nm);
        }
    }
    out
}

impl Signex {
    /// The net classes of `id`, in table order.
    pub(crate) fn rules_classes(&self, id: ProjectId) -> Vec<NetClass> {
        self.document_state
            .project_by_id(id)
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default()
    }

    /// The fallback of every cell: resolved on the open board, else from
    /// the classes alone.
    pub(crate) fn rules_matrix_defaults(&self, id: ProjectId) -> MatrixDefaults {
        let classes = self.rules_classes(id);
        match (self.rules_resolved(id), self.rules_board(id)) {
            (Ok(rules), Some((_, board))) => board_defaults(&rules, board, &classes),
            _ => class_defaults(&classes),
        }
    }

    /// Set the cell at `key` of `id`'s matrix to `value` (empty clears it)
    /// as one undo step. Both classes must be the project's.
    pub(crate) fn set_matrix_cell(
        &mut self,
        id: ProjectId,
        key: &CellKey,
        value: &str,
    ) -> Result<CellChange, String> {
        let classes = self.rules_classes(id);
        for class in [&key.a, &key.b] {
            if !classes.iter().any(|c| &c.name == class) {
                return Err(format!("the project has no net class '{class}'"));
            }
        }
        let (src, _) = self
            .rules_text_of(id)
            .ok_or_else(|| "the project is no longer open".to_string())?;
        let (text, change) = matrix::set_cell(&src, key, value).map_err(|e| e.to_string())?;
        // A stale severity entry must not speak for a new or removed rule.
        let follows: Vec<WrittenRuleEdit<'_>> = match &change {
            CellChange::Added(name) | CellChange::Removed(name) => {
                vec![WrittenRuleEdit::Dropped(name.as_str())]
            }
            CellChange::Changed(_) | CellChange::Unchanged => Vec::new(),
        };
        if change != CellChange::Unchanged {
            self.commit_rules_text(id, text, &follows)?;
        }
        Ok(change)
    }

    /// `rules_matrix_get`: the classes, the written cells and every cell's
    /// fallback.
    pub(crate) fn rules_matrix_get(&mut self, args: &CommandArgs) -> Result<Value, String> {
        let (id, src) = project_text(self, args)?;
        let rows = signex_drc_dsl::table::table(&src);
        let cells: Vec<Value> = matrix::cells(&rows).iter().map(cell_json).collect();
        let defaults: Vec<Value> = self
            .rules_matrix_defaults(id)
            .iter()
            .map(|((a, b, pair), nm)| {
                json!({
                    "class_a": a,
                    "class_b": b,
                    "object_pair": pair.map(|at| OBJECT_PAIRS[at].join("-")),
                    "value_mm": mm(*nm),
                })
            })
            .collect();
        let classes: Vec<String> = self.rules_classes(id).into_iter().map(|c| c.name).collect();
        Ok(json!({
            "classes": classes,
            "cells": cells,
            "defaults": defaults,
            "defaults_from_board": self.rules_board(id).is_some(),
        }))
    }
}

/// One written cell.
pub(crate) fn cell_json(cell: &MatrixCell) -> Value {
    json!({
        "class_a": cell.key.a,
        "class_b": cell.key.b,
        "object_pair": cell.key.pair.as_ref().map(|[x, y]| format!("{x}-{y}")),
        "layer": cell.key.layer,
        "value": cell.value,
        "rule": cell.rule,
        "enabled": cell.enabled,
    })
}

/// The cell `rules_matrix_set` names: `class_a`, `class_b`, and an
/// optional `object_pair` and `layer`.
fn named_key(args: &CommandArgs) -> Result<CellKey, String> {
    let (Some(a), Some(b)) = (text(args, "class_a"), text(args, "class_b")) else {
        return Err("needs the operands `class_a` and `class_b`".into());
    };
    let key = CellKey::new(a.trim(), b.trim())
        .with_pair(text(args, "object_pair").unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(key.with_layer(text(args, "layer").unwrap_or_default()))
}

/// `rules_matrix_set {document, class_a, class_b, object_pair?, layer?,
/// value?}`; no `value` clears the cell back to its fallback.
pub(crate) fn rules_matrix_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let key = match named_key(args) {
        Ok(key) => key,
        Err(reason) => return refused(reason),
    };
    let value = text(args, "value").unwrap_or_default();
    match app.set_matrix_cell(id, &key, value) {
        Ok(change) => {
            let src = app.rules_text_of(id).map(|(t, _)| t).unwrap_or_default();
            let rows = signex_drc_dsl::table::table(&src);
            let cell = matrix::cells(&rows)
                .into_iter()
                .find(|c| key.same_as(&c.key))
                .map(|c| cell_json(&c));
            answered(json!({
                "change": change.as_str(),
                "rule": change.rule(),
                "cell": cell,
            }))
        }
        Err(reason) => refused(reason),
    }
}
