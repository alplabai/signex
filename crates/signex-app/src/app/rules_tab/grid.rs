//! The constraint grid of one domain (#2530): which rows it lists (All,
//! net classes and their nets, copper layers, scoped rules), which
//! columns (the catalog groups of the domain), and what each cell shows,
//! resolved from the rule text by `signex_drc_dsl::table::grid`. Pure
//! model: the view reads it, `update` rebuilds it. Behaviour:
//! `docs/DRC_RULES.md`, "Rules manager".

use std::collections::{BTreeMap, BTreeSet};

use signex_drc_dsl::table::catalog::{self, CatalogColumn, CatalogGroup, Domain};
use signex_drc_dsl::table::grid::{self as cells, CellSource, GridScope};
use signex_drc_dsl::table::{LimitField, RuleRow, RuleTable};
use signex_i18n::{I18n, UserText};
use signex_types::project::NamedRuleSet;
use signex_widgets::LengthFormat;
use signex_widgets::constraint_grid::{
    CellRef, CellValue, CellValues, ColumnGroup, ColumnId, ColumnKind, ConstraintGrid, GridColumn,
    GridRow, GroupId, Resolved, RowId, RowKind, ValueSource,
};

pub(crate) use super::search::FindState;

/// A heading over a group of rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum RowGroup {
    NetClasses,
    Layers,
    /// Rules the hierarchy cannot express: a pattern, an object pair, a
    /// layer on a class, a condition the grid has no column for.
    Scoped,
}

impl RowGroup {
    /// Whether the group's rows are listed on `domain`. Placement and
    /// Signal integrity have no net rows; layers carry copper limits only.
    pub fn shows_on(self, domain: Domain) -> bool {
        match self {
            RowGroup::NetClasses => !matches!(domain, Domain::Placement | Domain::SignalIntegrity),
            RowGroup::Layers => {
                matches!(domain, Domain::Clearance | Domain::Physical | Domain::Mask)
            }
            RowGroup::Scoped => domain != Domain::SignalIntegrity,
        }
    }
}

/// A row of the grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum RowRef {
    All,
    Heading(RowGroup),
    Class(String),
    Net(String),
    /// A copper layer by the token rules name it with, numbered from the
    /// top: L1 is Top, the last is Bottom.
    Layer {
        token: String,
        number: usize,
        count: usize,
    },
    /// A scoped rule, by name.
    Scoped(String),
}

impl RowRef {
    /// The hierarchy row of the rule text; `None` for a heading or a
    /// scoped rule.
    pub fn scope(&self) -> Option<GridScope> {
        match self {
            RowRef::All => Some(GridScope::All),
            RowRef::Class(class) => Some(GridScope::Class(class.clone())),
            RowRef::Net(net) => Some(GridScope::Net(net.clone())),
            RowRef::Layer { token, .. } => Some(GridScope::Layer(token.clone())),
            RowRef::Heading(_) | RowRef::Scoped(_) => None,
        }
    }

    fn kind(&self) -> RowKind {
        match self {
            RowRef::All | RowRef::Heading(_) => RowKind::All,
            RowRef::Class(_) | RowRef::Layer { .. } => RowKind::Class,
            RowRef::Net(_) | RowRef::Scoped(_) => RowKind::Net,
        }
    }

    /// The name the grid shows: user content (a class, net, layer token
    /// or rule name); headings and All are worded by the view.
    pub fn name(&self) -> &str {
        match self {
            RowRef::Class(name) | RowRef::Net(name) | RowRef::Scoped(name) => name,
            RowRef::Layer { token, .. } => token,
            RowRef::All | RowRef::Heading(_) => "",
        }
    }
}

/// What a grid's rows are built from, read in `update` from the project,
/// its board and the rule text.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct RowSource {
    /// The project's net classes, in their order.
    pub classes: Vec<String>,
    /// The board's nets with the class each falls in, in board order.
    pub nets: Vec<(String, Option<String>)>,
    /// The board's copper layer tokens, top to bottom.
    pub layers: Vec<String>,
    /// The classes, nets and layers the rule text writes rules for.
    pub written: Vec<GridScope>,
    /// The rows with a value of their own on the domain; `Some` only while
    /// Overridden rows only is on.
    pub overridden: Option<BTreeSet<RowRef>>,
}

/// The grid of one domain: the rows and columns behind the widget's ids,
/// each row's parents for inheritance, and the widget's view state.
#[derive(Debug, Clone)]
pub struct RulesGrid {
    pub(crate) view: ConstraintGrid,
    domain: Domain,
    rows: Vec<RowRef>,
    /// Each row's parents, nearest first (a net's class, then All).
    parents: Vec<Vec<GridScope>>,
    columns: Vec<&'static CatalogColumn>,
    groups: Vec<&'static CatalogGroup>,
    /// What the rows were built from, so `update` rebuilds only on change.
    built_from: Option<(RowSource, Vec<String>)>,
}

impl Default for RulesGrid {
    fn default() -> Self {
        Self::new(Domain::Clearance)
    }
}

/// A row index as the widget's id.
fn row_id(index: usize) -> RowId {
    RowId(index as u32)
}

impl RulesGrid {
    /// An empty grid on `domain`, its columns from the catalog.
    pub(crate) fn new(domain: Domain) -> Self {
        let groups: Vec<&'static CatalogGroup> = domain.groups().collect();
        let mut columns = Vec::new();
        let mut widget_groups = Vec::with_capacity(groups.len());
        for (g, group) in groups.iter().enumerate() {
            let mut cols = Vec::with_capacity(group.columns.len());
            for column in group.columns {
                cols.push(GridColumn {
                    id: ColumnId(columns.len() as u32),
                    // The view words headers itself; the widget keeps ids.
                    title: UserText::new(column.id.to_string()).into(),
                    kind: ColumnKind::Length,
                });
                columns.push(column);
            }
            widget_groups.push(ColumnGroup {
                id: GroupId(g as u32),
                title: UserText::new(group.id.to_string()).into(),
                columns: cols,
            });
        }
        Self {
            view: ConstraintGrid::new(widget_groups),
            domain,
            rows: Vec::new(),
            parents: Vec::new(),
            columns,
            groups,
            built_from: None,
        }
    }

    pub fn domain(&self) -> Domain {
        self.domain
    }

    pub(crate) fn rows(&self) -> &[RowRef] {
        &self.rows
    }

    pub(crate) fn row(&self, id: RowId) -> Option<&RowRef> {
        self.rows.get(id.0 as usize)
    }

    pub(crate) fn row_of(&self, row: &RowRef) -> Option<RowId> {
        self.rows.iter().position(|r| r == row).map(row_id)
    }

    /// The row of a hierarchy scope (a layer by its token).
    pub(crate) fn row_of_scope(&self, scope: &GridScope) -> Option<RowId> {
        self.rows
            .iter()
            .position(|r| r.scope().as_ref() == Some(scope))
            .map(row_id)
    }

    pub(crate) fn parents(&self, id: RowId) -> &[GridScope] {
        self.parents.get(id.0 as usize).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn column(&self, id: ColumnId) -> Option<&'static CatalogColumn> {
        self.columns.get(id.0 as usize).copied()
    }

    pub(crate) fn column_named(&self, id: &str) -> Option<ColumnId> {
        self.columns
            .iter()
            .position(|c| c.id == id)
            .map(|i| ColumnId(i as u32))
    }

    pub(crate) fn group(&self, id: GroupId) -> Option<&'static CatalogGroup> {
        self.groups.get(id.0 as usize).copied()
    }

    pub(crate) fn groups(&self) -> &[&'static CatalogGroup] {
        &self.groups
    }

    /// The group that holds column `id`, by its widget id.
    pub(crate) fn group_of(&self, id: ColumnId) -> Option<GroupId> {
        self.view.group_of(id)
    }

    /// Whether the rows were built from `source` with these scoped rules.
    pub(crate) fn is_built_from(&self, source: &RowSource, scoped: &[String]) -> bool {
        self.built_from
            .as_ref()
            .is_some_and(|(s, r)| s == source && r.as_slice() == scoped)
    }

    /// Lay out the rows from `source` and the domain's `scoped` rules. Rows
    /// that were collapsed stay collapsed; a class starts collapsed unless
    /// one of its nets has a rule of its own.
    pub(crate) fn build_rows(&mut self, source: RowSource, scoped: Vec<String>, i18n: &I18n) {
        let was_closed: BTreeSet<RowRef> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                self.view
                    .visible_rows()
                    .iter()
                    .any(|v| v.id == row_id(*i) && v.has_children && !v.is_open)
            })
            .map(|(_, r)| r.clone())
            .collect();
        let had_rows = !self.rows.is_empty();
        let groups = self.view.groups().to_vec();
        let mut view = ConstraintGrid::new(groups);
        let plan = plan_rows(self.domain, &source, &scoped);
        self.rows.clear();
        self.parents.clear();
        for planned in &plan {
            let parent = planned.display_parent.map(row_id);
            self.rows.push(planned.row.clone());
            self.parents.push(planned.parents.clone());
            let pushed = view.push_row(GridRow {
                id: row_id(self.rows.len() - 1),
                parent,
                kind: planned.row.kind(),
                name: planned.row.name().to_string(),
            });
            debug_assert!(pushed.is_ok(), "rows are planned parents first");
        }
        for (i, row) in self.rows.iter().enumerate() {
            let closed = if had_rows {
                was_closed.contains(row)
            } else {
                matches!(row, RowRef::Class(class) if !has_written_net(&source, class))
            };
            if closed {
                view.collapse_row(row_id(i));
            }
        }
        // Keep which column groups were collapsed.
        for g in 0..self.groups.len() {
            let group = GroupId(g as u32);
            if self.view.is_group_collapsed(group) {
                let _ = view.update(
                    signex_widgets::constraint_grid::GridMessage::GroupToggled(group),
                    &NoCells,
                    mm_format(i18n),
                );
            }
        }
        self.view = view;
        self.built_from = Some((source, scoped));
    }
}

/// The length format of the rule text and the grid: millimetres (a
/// length keeps `.` in every locale).
pub(crate) fn mm_format(i18n: &I18n) -> LengthFormat<'_> {
    LengthFormat::new(i18n, signex_types::coord::Unit::Mm)
}

/// No cell values: for view-state messages that read none.
struct NoCells;

impl CellValues for NoCells {
    fn resolve(&self, _: CellRef) -> Option<Resolved> {
        None
    }
    fn inherited(&self, _: CellRef) -> Option<Resolved> {
        None
    }
}

fn has_written_net(source: &RowSource, class: &str) -> bool {
    source.nets.iter().any(|(net, of)| {
        of.as_deref() == Some(class) && source.written.contains(&GridScope::Net(net.clone()))
    })
}

/// One row as laid out.
struct Planned {
    row: RowRef,
    /// The index of the row it is drawn under.
    display_parent: Option<usize>,
    parents: Vec<GridScope>,
}

/// The rows of `domain`, parents first.
fn plan_rows(domain: Domain, source: &RowSource, scoped: &[String]) -> Vec<Planned> {
    let kept = |row: &RowRef| {
        source
            .overridden
            .as_ref()
            .is_none_or(|rows| rows.contains(row))
    };
    let mut out = vec![Planned {
        row: RowRef::All,
        display_parent: None,
        parents: Vec::new(),
    }];
    if RowGroup::NetClasses.shows_on(domain) {
        net_rows(source, &kept, &mut out);
    }
    if RowGroup::Layers.shows_on(domain) && !source.layers.is_empty() {
        let heading = out.len();
        let count = source.layers.len();
        let mut rows = Vec::new();
        for (i, token) in source.layers.iter().enumerate() {
            let row = RowRef::Layer {
                token: token.clone(),
                number: i + 1,
                count,
            };
            if kept(&row) {
                rows.push(Planned {
                    row,
                    display_parent: Some(heading),
                    parents: vec![GridScope::All],
                });
            }
        }
        push_group(&mut out, RowGroup::Layers, rows);
    }
    if RowGroup::Scoped.shows_on(domain) {
        let heading = out.len();
        let rows = scoped
            .iter()
            .map(|name| Planned {
                row: RowRef::Scoped(name.clone()),
                display_parent: Some(heading),
                parents: vec![GridScope::All],
            })
            .collect();
        push_group(&mut out, RowGroup::Scoped, rows);
    }
    out
}

/// A heading and its rows; no heading over no rows.
fn push_group(out: &mut Vec<Planned>, group: RowGroup, rows: Vec<Planned>) {
    if rows.is_empty() {
        return;
    }
    out.push(Planned {
        row: RowRef::Heading(group),
        display_parent: None,
        parents: Vec::new(),
    });
    out.extend(rows);
}

/// The net classes, each class's nets under it, then the nets of no class
/// that have rules of their own. Classes and nets the rule text names but
/// the project or board lacks are listed too, so no written rule hides.
fn net_rows(source: &RowSource, kept: &dyn Fn(&RowRef) -> bool, out: &mut Vec<Planned>) {
    let heading = out.len();
    let mut rows: Vec<Planned> = Vec::new();
    let mut classes = source.classes.clone();
    for scope in &source.written {
        if let GridScope::Class(class) = scope
            && !classes.contains(class)
        {
            classes.push(class.clone());
        }
    }
    for class in &classes {
        let class_row = RowRef::Class(class.clone());
        let nets: Vec<&String> = source
            .nets
            .iter()
            .filter(|(_, of)| of.as_deref() == Some(class.as_str()))
            .map(|(net, _)| net)
            .filter(|net| kept(&RowRef::Net((*net).clone())))
            .collect();
        let class_kept = kept(&class_row);
        if !class_kept && nets.is_empty() {
            continue;
        }
        let under = if class_kept {
            rows.push(Planned {
                row: class_row,
                display_parent: Some(heading),
                parents: vec![GridScope::All],
            });
            heading + rows.len()
        } else {
            heading
        };
        for net in nets {
            rows.push(Planned {
                row: RowRef::Net(net.clone()),
                display_parent: Some(under),
                parents: vec![GridScope::Class(class.clone()), GridScope::All],
            });
        }
    }
    let mut loose: Vec<String> = source
        .nets
        .iter()
        .filter(|(_, of)| of.is_none())
        .map(|(net, _)| net.clone())
        .filter(|net| source.written.contains(&GridScope::Net(net.clone())))
        .collect();
    for scope in &source.written {
        if let GridScope::Net(net) = scope
            && !source.nets.iter().any(|(n, _)| n == net)
            && !loose.contains(net)
        {
            loose.push(net.clone());
        }
    }
    for net in loose {
        let row = RowRef::Net(net);
        if kept(&row) {
            rows.push(Planned {
                row,
                display_parent: Some(heading),
                parents: vec![GridScope::All],
            });
        }
    }
    push_group(out, RowGroup::NetClasses, rows);
}

/// What one cell shows.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CellShow {
    /// The column does not apply to the row: hatched.
    NotApplicable,
    /// The check does not read the column yet: inert.
    NotChecked,
    /// No rule of the row or its parents sets it.
    Empty,
    Value {
        /// The length as written (`0.2mm`, `8 mil`).
        written: String,
        mm: Option<f64>,
        source: CellSource,
    },
}

impl CellShow {
    pub fn is_editable(&self) -> bool {
        matches!(self, CellShow::Empty | CellShow::Value { .. })
    }

    /// Whether the row sets the value itself (own, or a set on the row).
    pub fn is_own(&self) -> bool {
        matches!(
            self,
            CellShow::Value {
                source: CellSource::Own { .. } | CellSource::Set { .. },
                ..
            }
        )
    }
}

/// The rule text and sets a grid's cells resolve from.
pub(crate) struct Cells<'a> {
    pub grid: &'a RulesGrid,
    pub table: &'a RuleTable,
    pub sets: &'a BTreeMap<String, NamedRuleSet>,
}

impl Cells<'_> {
    /// What cell `at` shows.
    pub fn show(&self, at: CellRef) -> CellShow {
        let (Some(row), Some(column)) = (self.grid.row(at.row), self.grid.column(at.column)) else {
            return CellShow::NotApplicable;
        };
        let Some(field) = column.field else {
            return match row {
                RowRef::Scoped(_) | RowRef::Heading(_) => CellShow::NotApplicable,
                _ => CellShow::NotChecked,
            };
        };
        if !applies(self.table, row, field) {
            return CellShow::NotApplicable;
        }
        let found = match row {
            RowRef::Scoped(name) => self.table.row(name).and_then(|r| {
                r.limit(field).map(|l| cells::GridCell {
                    value: l.value.clone(),
                    source: CellSource::Own {
                        rule: r.name.clone(),
                    },
                })
            }),
            _ => row.scope().and_then(|scope| {
                cells::resolve(
                    self.table,
                    self.sets,
                    &scope,
                    self.grid.parents(at.row),
                    field,
                )
            }),
        };
        match found {
            Some(cell) => CellShow::Value {
                mm: length_mm(&cell.value),
                written: cell.value,
                source: cell.source,
            },
            None => CellShow::Empty,
        }
    }

    /// What the cell holds without a value of its own: its nearest
    /// parent's resolution.
    pub fn parent_value(&self, at: CellRef) -> CellShow {
        let (Some(column), parents) = (self.grid.column(at.column), self.grid.parents(at.row))
        else {
            return CellShow::Empty;
        };
        let (Some(field), Some((first, rest))) = (column.field, parents.split_first()) else {
            return CellShow::Empty;
        };
        match cells::resolve(self.table, self.sets, first, rest, field) {
            Some(cell) => CellShow::Value {
                mm: length_mm(&cell.value),
                written: cell.value,
                source: CellSource::Inherited {
                    from: first.clone(),
                    rule: cell.source.rule().unwrap_or_default().to_string(),
                },
            },
            None => CellShow::Empty,
        }
    }

    /// Whether `row` sets any checked column of the domain itself.
    pub fn has_own(&self, row: RowId) -> bool {
        (0..self.grid.columns.len())
            .any(|c| self.show(CellRef::new(row, ColumnId(c as u32))).is_own())
    }

    fn resolved(&self, show: CellShow) -> Option<Resolved> {
        let CellShow::Value { mm, source, .. } = show else {
            return None;
        };
        let source = match source {
            CellSource::Own { .. } | CellSource::Set { .. } => ValueSource::Own,
            CellSource::Inherited { from, .. } => self
                .grid
                .row_of_scope(&from)
                .map_or(ValueSource::Default, ValueSource::Inherited),
            CellSource::Default => ValueSource::Default,
        };
        Some(Resolved {
            value: CellValue::Length(mm?),
            source,
        })
    }
}

impl CellValues for Cells<'_> {
    fn resolve(&self, cell: CellRef) -> Option<Resolved> {
        self.resolved(self.show(cell))
    }

    fn inherited(&self, cell: CellRef) -> Option<Resolved> {
        self.resolved(self.parent_value(cell))
    }
}

/// Whether a value of `field` can be set on `row`. A board-wide kind
/// (silkscreen, mask, paste, courtyard) has only the All row; a scoped
/// rule takes the limits of its own kind.
pub(crate) fn applies(table: &RuleTable, row: &RowRef, field: LimitField) -> bool {
    match row {
        RowRef::All => true,
        RowRef::Heading(_) => false,
        RowRef::Class(_) | RowRef::Net(_) | RowRef::Layer { .. } => field.kind().is_scoped(),
        RowRef::Scoped(name) => table
            .row(name)
            .is_some_and(|r| r.kind == Some(field.kind()) && !r.text_only),
    }
}

/// A written length in mm: a number and an optional unit (mm, mil, um,
/// nm; mm when none). `None` for text that is not a length.
pub(crate) fn length_mm(written: &str) -> Option<f64> {
    let text = written.trim();
    let split = text
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number.trim().parse().ok()?;
    let per_mm = match unit.trim() {
        "" | "mm" => 1.0,
        "mil" => 0.0254,
        "um" => 0.001,
        "nm" => 0.000_001,
        _ => return None,
    };
    let mm = value * per_mm;
    mm.is_finite().then_some(mm)
}

/// A value typed in a cell as rule text: a bare number is millimetres.
pub(crate) fn typed_length(text: &str) -> String {
    let text = text.trim();
    if !text.is_empty() && text.parse::<f64>().is_ok() {
        format!("{text}mm")
    } else {
        text.to_string()
    }
}

/// The domain a rule shows in: its first limit's column group's, else
/// its kind's first limit's.
pub(crate) fn domain_of(row: &RuleRow) -> Option<Domain> {
    let field = row
        .limits
        .first()
        .map(|l| l.field)
        .or_else(|| row.kind.and_then(|k| k.fields().first().copied()))?;
    catalog::group_of(field).map(|g| g.domain)
}

/// The domains a rule has a column in: one per limit it sets.
pub(crate) fn domains_of(row: &RuleRow) -> BTreeSet<Domain> {
    let mut out: BTreeSet<Domain> = row
        .limits
        .iter()
        .filter_map(|l| catalog::group_of(l.field).map(|g| g.domain))
        .collect();
    out.extend(domain_of(row));
    out
}

/// The scoped rules listed on `domain`, in written order.
pub(crate) fn scoped_on(table: &RuleTable, domain: Domain) -> Vec<String> {
    cells::scoped_rules(table)
        .into_iter()
        .filter(|row| domains_of(row).contains(&domain))
        .map(|row| row.name.clone())
        .collect()
}

/// The constraint sets applied to `row`'s scope, by name.
pub(crate) fn sets_on(row: &RowRef, sets: &BTreeMap<String, NamedRuleSet>) -> Vec<String> {
    let Some(scope) = row.scope().filter(|s| *s != GridScope::All) else {
        return Vec::new();
    };
    let text = signex_drc_dsl::table::scope_text(&scope.net_scope());
    sets.iter()
        .filter(|(_, set)| set.uses.iter().any(|u| u.scope.trim() == text))
        .map(|(name, _)| name.clone())
        .collect()
}

/// The findings of the last run per rule name, summed over the rules that
/// set a limit of `group`.
pub(crate) fn group_findings(
    table: &RuleTable,
    results: &BTreeMap<String, signex_drc::RuleResult>,
    group: &CatalogGroup,
) -> usize {
    table
        .rows
        .iter()
        .filter(|row| {
            row.limits
                .iter()
                .any(|l| group.columns.iter().any(|c| c.field == Some(l.field)))
        })
        .filter_map(|row| results.get(&row.id()))
        .map(|r| r.violations)
        .sum()
}

/// The findings of the last run of the rules with a column on `domain`.
pub(crate) fn domain_findings(
    table: &RuleTable,
    results: &BTreeMap<String, signex_drc::RuleResult>,
    domain: Domain,
) -> usize {
    table
        .rows
        .iter()
        .filter(|row| domains_of(row).contains(&domain))
        .filter_map(|row| results.get(&row.id()))
        .map(|r| r.violations)
        .sum()
}
