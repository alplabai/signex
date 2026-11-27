//! Nested grouping of the Components panel table (the group bar).

use std::cmp::Ordering;
use std::collections::HashMap;

use super::cell::{CellValue, GroupKey};
use super::order::{CellCache, compare_cells, sort_indices};
use crate::templates::{ColumnKey, SortKey};

/// Where a group sits: one `(column, key)` per level, outermost first.
/// Stable across re-sorts and filters, so the panel can keep which groups
/// are open by path.
pub type GroupPath = Vec<(ColumnKey, GroupKey)>;

/// One entry of the arranged table.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewNode {
    Group(GroupNode),
    /// A row, by its index in the caller's list.
    Row(usize),
}

/// One group: the rows (or deeper groups) sharing a value.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupNode {
    pub path: GroupPath,
    pub column: ColumnKey,
    /// The value its rows share (the first row's, for text that differs
    /// only in case). `Missing` is the "Not set" group, always last.
    pub value: CellValue,
    /// Rows under it, at every depth.
    pub count: usize,
    pub children: Vec<ViewNode>,
}

/// The table arranged: rows `0..count` grouped by `group` (outermost
/// first; each level ordered by its own direction, "Not set" last) and
/// sorted by `sort` inside the innermost groups. With no group levels the
/// result is the sorted rows. `cell(i, column)` is row `i`'s value.
pub fn arrange_rows(
    count: usize,
    sort: &[SortKey],
    group: &[SortKey],
    cell: impl Fn(usize, &ColumnKey) -> CellValue,
    text_order: impl Fn(&str, &str) -> Ordering,
) -> Vec<ViewNode> {
    let columns = sort
        .iter()
        .chain(group)
        .map(|k| &k.column)
        .chain([&ColumnKey::PartNumber]);
    let cache = CellCache::new(count, columns, &cell);
    let rows: Vec<usize> = (0..count).collect();
    let ctx = Arrange {
        sort,
        group,
        cache: &cache,
        text_order: &text_order,
    };
    ctx.level(rows, 0, &Vec::new())
}

struct Arrange<'a> {
    sort: &'a [SortKey],
    group: &'a [SortKey],
    cache: &'a CellCache,
    text_order: &'a dyn Fn(&str, &str) -> Ordering,
}

impl Arrange<'_> {
    fn level(&self, mut rows: Vec<usize>, depth: usize, path: &GroupPath) -> Vec<ViewNode> {
        let Some(level) = self.group.get(depth) else {
            sort_indices(&mut rows, self.sort, self.cache, self.text_order);
            return rows.into_iter().map(ViewNode::Row).collect();
        };
        // (key, first row, members), in first-seen order.
        let mut buckets: Vec<(GroupKey, usize, Vec<usize>)> = Vec::new();
        let mut index: HashMap<GroupKey, usize> = HashMap::new();
        for row in rows {
            let key = self.cache.get(&level.column, row).group_key();
            match index.get(&key) {
                Some(&at) => buckets[at].2.push(row),
                None => {
                    index.insert(key.clone(), buckets.len());
                    buckets.push((key, row, vec![row]));
                }
            }
        }
        let value = |first: usize| self.cache.get(&level.column, first);
        buckets.sort_by(|(_, a, _), (_, b, _)| {
            compare_cells(value(*a), value(*b), level.descending, self.text_order)
        });
        buckets
            .into_iter()
            .map(|(key, first, members)| {
                let mut path = path.clone();
                path.push((level.column.clone(), key));
                let count = members.len();
                let children = self.level(members, depth + 1, &path);
                ViewNode::Group(GroupNode {
                    path,
                    column: level.column.clone(),
                    value: value(first).clone(),
                    count,
                    children,
                })
            })
            .collect()
    }
}

/// One visible line of the table.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewLine<'a> {
    Group {
        node: &'a GroupNode,
        depth: usize,
        open: bool,
    },
    Row {
        index: usize,
        depth: usize,
    },
}

/// The lines to draw: every group header, and the rows and deeper groups
/// under the groups `is_open` says are open.
pub fn visible_lines<'a>(
    nodes: &'a [ViewNode],
    is_open: &dyn Fn(&GroupPath) -> bool,
) -> Vec<ViewLine<'a>> {
    let mut out = Vec::new();
    push_lines(nodes, 0, is_open, &mut out);
    out
}

fn push_lines<'a>(
    nodes: &'a [ViewNode],
    depth: usize,
    is_open: &dyn Fn(&GroupPath) -> bool,
    out: &mut Vec<ViewLine<'a>>,
) {
    for node in nodes {
        match node {
            ViewNode::Row(index) => out.push(ViewLine::Row {
                index: *index,
                depth,
            }),
            ViewNode::Group(group) => {
                let open = is_open(&group.path);
                out.push(ViewLine::Group {
                    node: group,
                    depth,
                    open,
                });
                if open {
                    push_lines(&group.children, depth + 1, is_open, out);
                }
            }
        }
    }
}

/// Every group path, for Expand all.
pub fn group_paths(nodes: &[ViewNode]) -> Vec<GroupPath> {
    let mut out = Vec::new();
    for node in nodes {
        if let ViewNode::Group(group) = node {
            out.push(group.path.clone());
            out.extend(group_paths(&group.children));
        }
    }
    out
}
