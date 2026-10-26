//! Project-level sheet numbering (#1081): one number per sheet placement the
//! project walk reaches. Design and rules:
//! `docs/superpowers/specs/2026-10-25-sheet-numbering-design.md`.

use std::collections::BTreeMap;

use signex_types::schematic::{InstancePath, SheetInstance};

use super::{ProjectGraph, SheetKey, walk_occurrences};

/// One numbered page: a sheet placement the project walk reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPage {
    /// The sheet this page is a placement of.
    pub key: SheetKey,
    /// The `ChildSheet.uuid` chain from the walk root (a root page: empty).
    pub instance_path: InstancePath,
    /// The `ChildSheet.name` chain from the walk root (a root page: empty).
    pub name_chain: Vec<String>,
    /// The printed number, 1-based.
    pub number: usize,
    /// Whether `number` came from a sheet-symbol entry.
    pub manual: bool,
}

impl SheetPage {
    /// How a message names this page: its sheet-symbol name chain, or the
    /// sheet itself for a page the walk starts at.
    pub fn label(&self) -> String {
        if self.name_chain.is_empty() {
            self.key.to_string()
        } else {
            self.name_chain.join("/")
        }
    }
}

/// Every page of a project, in walk order, with its number.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetNumbering {
    pages: Vec<SheetPage>,
}

impl SheetNumbering {
    /// The pages in walk order (pre-order, document order, roots in stitcher
    /// order). [`NumberingIssue`] names pages by their index here.
    pub fn pages(&self) -> &[SheetPage] {
        &self.pages
    }

    /// M in "Sheet N of M": the project's page count, whatever subset an
    /// export prints.
    pub fn count(&self) -> usize {
        self.pages.len()
    }

    /// The number of placement `instance_path` of sheet `key`. Both are
    /// needed: every page the root does not reach is walked as a root of its
    /// own, so several pages share the empty path.
    pub fn number_of(&self, key: &SheetKey, instance_path: &InstancePath) -> Option<usize> {
        self.index_of(key, instance_path)
            .map(|index| self.pages[index].number)
    }

    /// Where placement `instance_path` of sheet `key` sits in [`Self::pages`]
    /// (its walk position, which the printed order follows).
    pub fn index_of(&self, key: &SheetKey, instance_path: &InstancePath) -> Option<usize> {
        self.pages
            .iter()
            .position(|page| &page.key == key && &page.instance_path == instance_path)
    }
}

/// Something the numbering found wrong. Pages are indices into
/// [`SheetNumbering::pages`]; none of these stops an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberingIssue {
    /// Two or more pages print `number`.
    Clash { number: usize, pages: Vec<usize> },
    /// Numbers in `1..=count` that no page prints.
    Gap { missing: Vec<usize> },
    /// A manual number above the page count; it is printed anyway.
    OutOfRange { page: usize, number: usize },
    /// A manual entry whose `page` is not a positive whole number; the page
    /// keeps its automatic number.
    InvalidOverride { page: usize, text: String },
}

/// Number every page `graph` reaches. `project_name` selects the manual
/// entries that apply (an entry with an empty `project` applies to any).
pub fn sheet_numbering(
    graph: &ProjectGraph,
    project_name: &str,
) -> (SheetNumbering, Vec<NumberingIssue>) {
    // Missing and cyclic children are the stitcher's to report; here they are
    // only placements the walk never reaches.
    let mut stitch_issues = Vec::new();
    let (occs, edges) = walk_occurrences(graph, &mut stitch_issues);
    let mut placed_by: Vec<Option<(usize, usize)>> = vec![None; occs.len()];
    for &(parent, cs_index, child) in &edges {
        placed_by[child] = Some((parent, cs_index));
    }

    let mut issues = Vec::new();
    let mut pages = Vec::with_capacity(occs.len());
    for (index, occ) in occs.iter().enumerate() {
        let entry = placed_by[index].and_then(|(parent, cs_index)| {
            let symbol = occs[parent].sheet.child_sheets.get(cs_index)?;
            manual_entry(&symbol.instances, &occs[parent].path, project_name)
        });
        let manual = entry.and_then(|text| {
            let number = parse_page_number(text);
            if number.is_none() {
                issues.push(NumberingIssue::InvalidOverride {
                    page: index,
                    text: text.to_string(),
                });
            }
            number
        });
        pages.push(SheetPage {
            key: occ.key.clone(),
            instance_path: occ.path.clone(),
            name_chain: occ.name_chain.clone(),
            number: manual.unwrap_or(index + 1),
            manual: manual.is_some(),
        });
    }
    issues.extend(consistency_issues(&pages));
    (SheetNumbering { pages }, issues)
}

/// The `page` text of the entry that numbers the placement made inside parent
/// placement `parent`: its `path` parses to `parent` (`""` is an unset path
/// and matches nothing) and its `project` is empty or `project_name`. The
/// first such entry wins.
fn manual_entry<'a>(
    instances: &'a [SheetInstance],
    parent: &InstancePath,
    project_name: &str,
) -> Option<&'a str> {
    instances
        .iter()
        .find(|entry| {
            (entry.project.is_empty() || entry.project == project_name)
                && InstancePath::parse(&entry.path).as_ref() == Some(parent)
        })
        .map(|entry| entry.page.as_str())
}

/// A manual page number: a positive whole number, spaces around it allowed.
fn parse_page_number(text: &str) -> Option<usize> {
    text.trim()
        .parse::<usize>()
        .ok()
        .filter(|number| *number > 0)
}

/// Clashes (by number), numbers above the count (by page), then the gap.
fn consistency_issues(pages: &[SheetPage]) -> Vec<NumberingIssue> {
    let mut holders: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, page) in pages.iter().enumerate() {
        holders.entry(page.number).or_default().push(index);
    }
    let mut issues: Vec<NumberingIssue> = holders
        .iter()
        .filter(|(_, holding)| holding.len() > 1)
        .map(|(number, holding)| NumberingIssue::Clash {
            number: *number,
            pages: holding.clone(),
        })
        .collect();
    let count = pages.len();
    issues.extend(
        pages
            .iter()
            .enumerate()
            .filter(|(_, page)| page.number > count)
            .map(|(index, page)| NumberingIssue::OutOfRange {
                page: index,
                number: page.number,
            }),
    );
    let missing: Vec<usize> = (1..=count)
        .filter(|number| !holders.contains_key(number))
        .collect();
    if !missing.is_empty() {
        issues.push(NumberingIssue::Gap { missing });
    }
    issues
}

#[cfg(test)]
#[path = "sheet_numbers_tests.rs"]
mod tests;
