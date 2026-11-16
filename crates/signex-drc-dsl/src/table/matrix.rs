//! The class-to-class clearance matrix (#2398): one value per pair of net
//! classes instead of N² separate rules. A cell is a clearance rule scoped
//! `class == "A" and other_class == "B"`, so the matrix and the rule list
//! are two views of the same text; a sub-grid cell narrows it to an object
//! pair (and a layer). See `docs/DRC_RULES.md`, "Class matrix".

use signex_drc::NetScope;

use super::edit::{self, EditError, RuleField, add_rule, remove_rule, set_field};
use super::{LimitField, RuleDraft, RuleKind, RuleRow, RuleTable, table};

/// The object pairs of a cell's sub-grid, in sub-grid order.
pub const OBJECT_PAIRS: [[&str; 2]; 6] = [
    ["track", "track"],
    ["track", "pad"],
    ["track", "via"],
    ["pad", "pad"],
    ["pad", "via"],
    ["via", "via"],
];

/// Where one matrix cell sits: two classes (either order), and the object
/// pair and layer a sub-grid cell narrows it to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CellKey {
    pub a: String,
    pub b: String,
    pub pair: Option<[String; 2]>,
    pub layer: Option<String>,
}

impl CellKey {
    /// The cell of classes `a` and `b`, every object pair and layer.
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        CellKey {
            a: a.into(),
            b: b.into(),
            pair: None,
            layer: None,
        }
    }

    /// The same classes, narrowed to `pair` (`track-pad`; empty for none).
    pub fn with_pair(mut self, pair: &str) -> Result<Self, EditError> {
        self.pair = edit::parse_pair(pair.trim()).map_err(|reason| EditError::BadValue {
            field: "object_pair",
            reason,
        })?;
        Ok(self)
    }

    /// The same classes, narrowed to `layer` (empty for every layer).
    pub fn with_layer(mut self, layer: &str) -> Self {
        let layer = layer.trim();
        self.layer = (!layer.is_empty()).then(|| layer.to_string());
        self
    }

    /// Whether `other` is the same cell: the classes in either order, the
    /// same object pair (either order) and layer.
    pub fn same_as(&self, other: &CellKey) -> bool {
        let classes =
            (self.a == other.a && self.b == other.b) || (self.a == other.b && self.b == other.a);
        classes && same_pair(self.pair.as_ref(), other.pair.as_ref()) && self.layer == other.layer
    }

    /// Whether `row` is this cell's rule.
    pub fn matches(&self, row: &RuleRow) -> bool {
        let NetScope::ClassPair(x, y) = &row.scope else {
            return false;
        };
        let classes = (x == &self.a && y == &self.b) || (x == &self.b && y == &self.a);
        row.kind == Some(RuleKind::Clearance)
            && !row.text_only
            && classes
            && same_pair(row.pair.as_ref(), self.pair.as_ref())
            && row.layer == self.layer
    }

    /// The name a new rule for this cell starts from:
    /// `cc_<a>_<b>[_<object>_<object>][_<layer>]`.
    fn base_name(&self) -> String {
        let mut parts = vec!["cc".to_string(), slug(&self.a), slug(&self.b)];
        if let Some([x, y]) = &self.pair {
            parts.extend([slug(x), slug(y)]);
        }
        if let Some(layer) = &self.layer {
            parts.push(slug(layer));
        }
        parts.join("_")
    }
}

fn same_pair(a: Option<&[String; 2]>, b: Option<&[String; 2]>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some([a0, a1]), Some([b0, b1])) => (a0 == b0 && a1 == b1) || (a0 == b1 && a1 == b0),
        _ => false,
    }
}

/// `text` as rule-name characters: letters and digits kept, the rest `_`.
pub(crate) fn slug(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// One written matrix cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixCell {
    pub key: CellKey,
    /// The rule that holds it.
    pub rule: String,
    /// The clearance as written (`0.8mm`).
    pub value: String,
    pub enabled: bool,
}

/// Every matrix cell the text holds, in written order: the clearance rules
/// scoped to two classes the table can show.
pub fn cells(rows: &RuleTable) -> Vec<MatrixCell> {
    rows.rows
        .iter()
        .filter(|row| row.kind == Some(RuleKind::Clearance) && !row.text_only)
        .filter_map(|row| {
            let NetScope::ClassPair(a, b) = &row.scope else {
                return None;
            };
            let value = row.limit(LimitField::MinClearance)?.value.clone();
            Some(MatrixCell {
                key: CellKey {
                    a: a.clone(),
                    b: b.clone(),
                    pair: row.pair.clone(),
                    layer: row.layer.clone(),
                },
                rule: row.name.clone(),
                value,
                enabled: row.enabled,
            })
        })
        .collect()
}

/// The first rule that holds the cell at `key`.
pub fn cell<'t>(rows: &'t RuleTable, key: &CellKey) -> Option<&'t RuleRow> {
    rows.rows.iter().find(|row| key.matches(row))
}

/// What setting a cell did, with the rule it touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellChange {
    Added(String),
    Changed(String),
    Removed(String),
    Unchanged,
}

impl CellChange {
    /// Stable name in answers.
    pub const fn as_str(&self) -> &'static str {
        match self {
            CellChange::Added(_) => "added",
            CellChange::Changed(_) => "changed",
            CellChange::Removed(_) => "removed",
            CellChange::Unchanged => "unchanged",
        }
    }

    /// The rule it touched.
    pub fn rule(&self) -> Option<&str> {
        match self {
            CellChange::Added(name) | CellChange::Changed(name) | CellChange::Removed(name) => {
                Some(name)
            }
            CellChange::Unchanged => None,
        }
    }
}

/// `src` with the cell at `key` set to `value`, a length (`0.8mm`); an
/// empty value removes the cell's rule, so the cell falls back to what the
/// two classes ask without it. Only the cell's rule is written.
pub fn set_cell(src: &str, key: &CellKey, value: &str) -> Result<(String, CellChange), EditError> {
    for class in [&key.a, &key.b] {
        if !edit::is_plain(class) {
            return Err(EditError::BadValue {
                field: "class",
                reason: "a class name with no double quote or line break".into(),
            });
        }
    }
    let rows = table(src);
    if rows.is_unparsed() {
        return Err(EditError::Unparsed);
    }
    let value = value.trim();
    let field = RuleField::Limit(LimitField::MinClearance);
    match (cell(&rows, key), value.is_empty()) {
        (None, true) => Ok((src.to_string(), CellChange::Unchanged)),
        (Some(row), true) => {
            let name = row.name.clone();
            Ok((remove_rule(src, &name)?, CellChange::Removed(name)))
        }
        (Some(row), false) => {
            if row
                .limit(LimitField::MinClearance)
                .map(|l| l.value.as_str())
                == Some(value)
            {
                return Ok((src.to_string(), CellChange::Unchanged));
            }
            let name = row.name.clone();
            Ok((
                set_field(src, &name, field, value)?,
                CellChange::Changed(name),
            ))
        }
        (None, false) => {
            let mut draft = RuleDraft::new(RuleKind::Clearance, free_name(&rows, &key.base_name()));
            draft.scope = NetScope::ClassPair(key.a.clone(), key.b.clone());
            draft.pair = key.pair.clone();
            draft.layer = key.layer.clone();
            draft.set(field, value)?;
            let (text, name) = add_rule(src, &draft)?;
            Ok((text, CellChange::Added(name)))
        }
    }
}

/// `base`, or `base_<n>` with the first free `n`.
pub(crate) fn free_name(rows: &RuleTable, base: &str) -> String {
    let taken = |name: &str| rows.rows.iter().any(|r| r.name == name);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|name| !taken(name))
        .unwrap_or_default()
}
