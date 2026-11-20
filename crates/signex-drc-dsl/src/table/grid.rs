//! The Rules constraint grid (#2530): one cell per hierarchy row (All, a
//! net class, a net, a layer) and limit column, resolved from the rule
//! text. A cell's value is the rule that would govern that row by the
//! check's own precedence (`UserRule::wins_over`): the row's own rule,
//! else a parent row's. Editing a cell rewrites or adds one rule, so the
//! grid, the rule list and the text stay three views of one text. A rule
//! the grid has no row for (an object pair, a pattern, a layer on a net
//! scope, a text-only rule) is a scoped rule, listed apart. See
//! `docs/DRC_RULES.md`, "Rules manager".

use std::collections::BTreeMap;

use signex_drc::{NetScope, is_fab_rule};
use signex_types::project::NamedRuleSet;

use super::edit::{EditError, RuleField, add_rule, remove_rule, set_field};
use super::matrix::{free_name, slug};
use super::sets::writer_of;
use super::{LimitField, RuleDraft, RuleKind, RuleRow, RuleTable, table};

/// A hierarchy row of the grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GridScope {
    /// Every net: the board-wide rules.
    All,
    Class(String),
    Net(String),
    /// Every net on one copper layer.
    Layer(String),
}

impl GridScope {
    /// The row a rule of `scope` on `layer` sits on; `None` for a rule the
    /// grid lists apart (a pattern, a class pair, a layer on a net scope).
    pub fn of(scope: &NetScope, layer: Option<&str>) -> Option<GridScope> {
        match (scope, layer) {
            (NetScope::Board, None) => Some(GridScope::All),
            (NetScope::Board, Some(layer)) => Some(GridScope::Layer(layer.to_string())),
            (NetScope::Class(class), None) => Some(GridScope::Class(class.clone())),
            (NetScope::Net(net), None) => Some(GridScope::Net(net.clone())),
            _ => None,
        }
    }

    /// Whether `row` is a plain rule of this row.
    pub fn holds(&self, row: &RuleRow) -> bool {
        !row.text_only
            && row.kind.is_some()
            && row.pair.is_none()
            && GridScope::of(&row.scope, row.layer.as_deref()).as_ref() == Some(self)
    }

    /// The net scope a new rule of this row is written with.
    pub fn net_scope(&self) -> NetScope {
        match self {
            GridScope::All | GridScope::Layer(_) => NetScope::Board,
            GridScope::Class(class) => NetScope::Class(class.clone()),
            GridScope::Net(net) => NetScope::Net(net.clone()),
        }
    }

    /// The layer condition a new rule of this row is written with.
    pub fn layer(&self) -> Option<String> {
        match self {
            GridScope::Layer(layer) => Some(layer.clone()),
            _ => None,
        }
    }

    /// The row in a new rule's name: `all`, `class_HV`, `net_GND`,
    /// `layer_L1`.
    fn slug(&self) -> String {
        match self {
            GridScope::All => "all".to_string(),
            GridScope::Class(class) => format!("class_{}", slug(class)),
            GridScope::Net(net) => format!("net_{}", slug(net)),
            GridScope::Layer(layer) => format!("layer_{}", slug(layer)),
        }
    }
}

/// Where a cell's value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellSource {
    /// A rule written for this row.
    Own { rule: String },
    /// A rule of a parent row.
    Inherited { from: GridScope, rule: String },
    /// A rule a named constraint set wrote for this row.
    Set { set: String, rule: String },
    /// The project's fab tier floor (a `fab_*` rule).
    Default,
}

impl CellSource {
    /// The governing rule's name; `None` for the tier default.
    pub fn rule(&self) -> Option<&str> {
        match self {
            CellSource::Own { rule }
            | CellSource::Inherited { rule, .. }
            | CellSource::Set { rule, .. } => Some(rule.as_str()),
            CellSource::Default => None,
        }
    }
}

/// One resolved cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCell {
    /// The length as written (`0.2mm`, `8 mil`).
    pub value: String,
    pub source: CellSource,
}

/// The cell of `field` on row `scope`, whose parents (nearest first) are
/// `parents`: a net's class then All, a class's All. Only applied rules
/// take part, ranked by the check's precedence. `None` when no rule of the
/// row or its parents sets the limit. `sets` are the project's named
/// constraint sets (`DrcRuleSet::sets`).
pub fn resolve(
    table: &RuleTable,
    sets: &BTreeMap<String, NamedRuleSet>,
    scope: &GridScope,
    parents: &[GridScope],
    field: LimitField,
) -> Option<GridCell> {
    let (row, at) = winner(table, scope, parents, field)?;
    let value = row.limit(field)?.value.clone();
    let rule = row.name.clone();
    let source = if is_fab_rule(&rule) {
        CellSource::Default
    } else if at != scope {
        CellSource::Inherited {
            from: at.clone(),
            rule,
        }
    } else if let Some((set, _)) = writer_of(sets, &rule) {
        CellSource::Set {
            set: set.to_string(),
            rule,
        }
    } else {
        CellSource::Own { rule }
    };
    Some(GridCell { value, source })
}

/// The applied rule that governs `field` on `scope` (parents nearest
/// first), and the row it sits on, by the check's precedence.
fn winner<'a>(
    table: &'a RuleTable,
    scope: &'a GridScope,
    parents: &'a [GridScope],
    field: LimitField,
) -> Option<(&'a RuleRow, &'a GridScope)> {
    let kind = field.limit_kind();
    let mut best: Option<((u8, u8), i64, &RuleRow, &GridScope)> = None;
    for row in table.rows.iter().filter(|r| r.applied) {
        let Some(rule) = &row.compiled else {
            continue;
        };
        let Some(value) = rule.limit(kind) else {
            continue;
        };
        let Some(at) = std::iter::once(scope).chain(parents).find(|s| s.holds(row)) else {
            continue;
        };
        if rule.wins_over(kind, value, best.map(|(rank, v, _, _)| (rank, v))) {
            best = Some((rule.rank(), value, row, at));
        }
    }
    best.map(|(_, _, row, at)| (row, at))
}

/// The row's own rules of `kind` the grid edits: plain rules of `scope`,
/// not the tier's `fab_*` floor.
fn own_rows<'t>(
    table: &'t RuleTable,
    scope: &GridScope,
    kind: RuleKind,
) -> impl Iterator<Item = &'t RuleRow> {
    table
        .rows
        .iter()
        .filter(move |r| r.kind == Some(kind) && scope.holds(r) && !is_fab_rule(&r.name))
}

/// `text` with cell `field` of row `scope` set to `value` (a length): the
/// row's own rule the cell shows is rewritten (else the first own rule
/// that sets the limit, else the first of the kind), else a rule is added
/// for the row. An empty value resets the cell. A value the cell would
/// still not show (another rule of the row or a `fab_*` floor is
/// stricter) is refused, naming that rule. A rule a constraint set wrote
/// is edited like any other; the set writes its own value back when it
/// is next applied, so a set's value is changed in the set. `table` is
/// `text`'s table.
pub fn set_cell(
    text: &str,
    table: &RuleTable,
    scope: &GridScope,
    field: LimitField,
    value: &str,
) -> Result<String, EditError> {
    let value = value.trim();
    if value.is_empty() {
        return reset_cell(text, table, scope, field);
    }
    let kind = field.kind();
    if !kind.is_scoped() && *scope != GridScope::All {
        return Err(EditError::BadValue {
            field: "scope",
            reason: format!("a {} rule is board-wide", kind.keyword()),
        });
    }
    let shown = winner(table, scope, &[], field)
        .map(|(row, _)| row)
        .filter(|row| row.kind == Some(kind) && !is_fab_rule(&row.name));
    let own = shown
        .or_else(|| own_rows(table, scope, kind).find(|r| r.limit(field).is_some()))
        .or_else(|| own_rows(table, scope, kind).next());
    let (out, edited) = match own {
        Some(row) => (
            set_field(text, &row.name, RuleField::Limit(field), value)?,
            row.name.clone(),
        ),
        None => {
            let mut draft = RuleDraft::new(kind, "");
            draft.scope = scope.net_scope();
            draft.layer = scope.layer();
            draft.name = free_name(table, &format!("{}_{}", scope.slug(), kind.keyword()));
            draft.set(RuleField::Limit(field), value)?;
            add_rule(text, &draft)?
        }
    };
    let after = super::table(&out);
    let nm = |row: &RuleRow| row.compiled.as_ref()?.limit(field.limit_kind());
    let typed = after.row(&edited).and_then(nm);
    match winner(&after, scope, &[], field) {
        Some((row, _)) if row.name == edited => Ok(out),
        // An equal rule written first keeps the cell, at the typed value.
        Some((row, _)) if typed.is_some() && nm(row) == typed => Ok(out),
        Some((row, _)) => Err(EditError::Overruled {
            rule: edited,
            by: Some(row.name.clone()),
        }),
        None => Err(EditError::Overruled {
            rule: edited,
            by: None,
        }),
    }
}

/// `text` with cell `field` of row `scope` back to what the row inherits:
/// the limit leaves each of the row's own rules that set it, and a rule
/// left with no limit is removed. Every other rule stays byte for byte.
pub fn reset_cell(
    text: &str,
    table: &RuleTable,
    scope: &GridScope,
    field: LimitField,
) -> Result<String, EditError> {
    let mut out = text.to_string();
    let mut rows = table.clone();
    // Each pass takes the limit off one rule, so the loop ends.
    loop {
        let next = own_rows(&rows, scope, field.kind())
            .find(|r| r.limit(field).is_some())
            .map(|r| (r.name.clone(), r.limits.len()));
        let Some((name, limits)) = next else {
            return Ok(out);
        };
        out = if limits == 1 {
            remove_rule(&out, &name)?
        } else {
            set_field(&out, &name, RuleField::Limit(field), "")?
        };
        rows = super::table(&out);
    }
}

/// Whether `row` is a cell of the class-to-class matrix.
fn is_matrix_cell(row: &RuleRow) -> bool {
    !row.text_only && row.kind == Some(RuleKind::Clearance) && row.scope.classes().len() == 2
}

/// The rules the grid has no row for, in written order: an object pair, a
/// pattern, a layer on a net or class scope, or a condition or limit the
/// table cannot show. A rule no cell shows is listed too, so its On and
/// Remove stay in reach: one that is not applied (off, or refused by the
/// compiler), and one another rule of its row beats on every limit it
/// sets. Class-matrix cells are the matrix's.
pub fn scoped_rules(table: &RuleTable) -> Vec<&RuleRow> {
    table
        .rows
        .iter()
        .filter(|r| !is_matrix_cell(r))
        .filter(|r| {
            match GridScope::of(&r.scope, r.layer.as_deref()).filter(|scope| scope.holds(r)) {
                None => true,
                Some(_) if !r.applied => true,
                Some(scope) => is_losing_duplicate(table, &scope, r),
            }
        })
        .collect()
}

/// Whether `row`, an applied rule of grid row `scope`, governs none of the
/// limits it sets there. A `fab_*` floor is not listed: the cell names it
/// when it governs.
fn is_losing_duplicate(table: &RuleTable, scope: &GridScope, row: &RuleRow) -> bool {
    row.applied
        && !is_fab_rule(&row.name)
        && !row.limits.is_empty()
        && row.limits.iter().all(|limit| {
            winner(table, scope, &[], limit.field).is_some_and(|(won, _)| won.name != row.name)
        })
}

/// The rows the text writes rules for, in first-written order: the
/// classes, nets and layers a grid shows beside the project's own.
pub fn written_scopes(table: &RuleTable) -> Vec<GridScope> {
    let mut out: Vec<GridScope> = Vec::new();
    for row in &table.rows {
        let Some(scope) = GridScope::of(&row.scope, row.layer.as_deref()) else {
            continue;
        };
        if scope.holds(row) && !out.contains(&scope) {
            out.push(scope);
        }
    }
    out
}

/// [`resolve`] over `text` directly.
pub fn resolve_text(
    text: &str,
    sets: &BTreeMap<String, NamedRuleSet>,
    scope: &GridScope,
    parents: &[GridScope],
    field: LimitField,
) -> Option<GridCell> {
    resolve(&table(text), sets, scope, parents, field)
}
