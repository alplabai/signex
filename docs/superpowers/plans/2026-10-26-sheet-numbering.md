# Sheet N of M (#1081) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Number every sheet placement of a project once, depth-first through the hierarchy, in a domain crate, and have every export print those numbers whatever subset it includes.

**Architecture:** A pure function `signex_net::sheet_numbering(graph, project_name)` reuses the stitcher's own occurrence walk (`walk_occurrences`, which also returns the parent→child edges) and reads manual numbers from the parent's sheet symbol (`ChildSheet.instances[]`). The app calls it once in the export scope builder beside `project_occurrences`, stamps `sheet_number` / `sheet_count` on every printed page, and turns the issues into Messages lines through the existing once-per-action `log_stitch_issues`. `sheet_cell_value` returns `sheet_number`.

**Tech Stack:** Rust 1.97.0 (pinned), edition 2024; crates `signex-net`, `signex-output`, `signex-app`; `cargo test`, `cargo clippy -D warnings`.

**Spec:** `docs/superpowers/specs/2026-10-25-sheet-numbering-design.md` (approved 2026-10-25).

## Global Constraints

- Repository text (code, comments, commits, PR) is English only; no AI attribution anywhere.
- No change to `.snxsch` / `.snxprj` bytes; every field used already exists and round-trips.
- Domain logic lives in `signex-net`; `signex-app` only reads the numbers.
- Gates (all required): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `bash scripts/check_file_size.sh` (1200 physical lines), `bash scripts/check_discarded_tasks.sh`, `bash scripts/check_lint_lockstep.sh`, `git grep -niE 'kicad' -- 'crates/**/*.rs' 'crates/**/*.toml'` prints nothing.
- House style: files ≤ ~800 lines, functions ≤ ~50 lines; comments carry the non-obvious why, long rationale goes in `docs/`.
- Tests are written first and seen to fail.
- Messages lines start `Sheet numbering:` and are logged once per export action (they appear as `Export: Sheet numbering: …`, like every other export line).

## Spec deviations (found while mapping the merged #979 code)

1. **A page is `(SheetKey, InstancePath)`, not `InstancePath` alone.** `walk_occurrences` walks every declared page the root does not reach as a root of its own, with the empty path, so several pages share `/`. `number_of` takes both, and issues name pages by their index into `SheetNumbering::pages()`.
2. **An entry path `""` matches nothing.** #979's `InstancePath::parse("")` is `None` (an unset path, as for a default `SymbolInstance`); only `"/"` is the root. Spec test 6 becomes "`/` matches the root, `""` is ignored".
3. **Numbers are `usize`**, matching `SheetSnapshot::sheet_number`, instead of `u32`.
4. **Messages name a page by its sheet-symbol name chain** (`B`, `A/L`), or by the sheet for a page the walk starts at, instead of "Power (2)". That is the label the page copies already carry in their titles (`child.snxsch (B)`).
5. **The app tests assert the export scope's `sheet_number` / `sheet_count`**, which the PDF title block (`SubstitutionContext`) already reads unchanged, instead of rendering a PDF in `tests/regression`.

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/signex-net/src/project/sheet_numbers.rs` | create | `SheetNumbering`, `SheetPage`, `NumberingIssue`, `sheet_numbering` |
| `crates/signex-net/src/project/sheet_numbers_tests.rs` | create | domain tests on the spec's example tree |
| `crates/signex-net/src/project/mod.rs` | modify | `mod sheet_numbers;` + re-export |
| `crates/signex-net/src/lib.rs` | modify | crate re-export |
| `crates/signex-output/src/expression.rs` | modify | `sheet_cell_value` returns `sheet_number` |
| `crates/signex-app/src/app/handlers/menu/export/sheet_numbers.rs` | create | apply numbers to pages; format Messages lines |
| `crates/signex-app/src/app/handlers/menu/export/mod.rs` | modify | call `sheet_numbering` in `build_scope`; `ExportIssues::numbering` |
| `crates/signex-app/src/app/handlers/menu/export/placements.rs` | modify | doc: `page_copies` numbering is the loose-document fallback |
| `crates/signex-app/src/app/handlers/menu/export/tests/sheet_numbering.rs` | create | export-scope tests |
| `crates/signex-app/src/app/handlers/menu/export/tests.rs` | modify | `mod sheet_numbering;` |
| `changelog.d/1081.added.md` | create | changelog fragment |

---

### Task 1: Domain numbering in `signex-net`

**Files:**
- Create: `crates/signex-net/src/project/sheet_numbers.rs`
- Create: `crates/signex-net/src/project/sheet_numbers_tests.rs`
- Modify: `crates/signex-net/src/project/mod.rs` (next to `mod designators;` / `pub use designators::instance_entry_designators;`, ~line 54-57)
- Modify: `crates/signex-net/src/lib.rs:23-26`

**Interfaces:**
- Consumes: `super::walk_occurrences(graph, &mut issues) -> (Vec<Occ>, Vec<(usize, usize, usize)>)` (edges are `(parent occ, cs index, child occ)`); `Occ { sheet, name_chain, path, key }` (private, visible to a child module); `InstancePath::parse`, `InstancePath::from_chain`; `SheetInstance { project, path, page }`.
- Produces (re-exported from `signex_net`):
  - `pub struct SheetPage { pub key: SheetKey, pub instance_path: InstancePath, pub name_chain: Vec<String>, pub number: usize, pub manual: bool }` with `pub fn label(&self) -> String`
  - `pub struct SheetNumbering` with `pub fn pages(&self) -> &[SheetPage]`, `pub fn count(&self) -> usize`, `pub fn number_of(&self, key: &SheetKey, instance_path: &InstancePath) -> Option<usize>`
  - `pub enum NumberingIssue { Clash { number: usize, pages: Vec<usize> }, Gap { missing: Vec<usize> }, OutOfRange { page: usize, number: usize }, InvalidOverride { page: usize, text: String } }`
  - `pub fn sheet_numbering(graph: &ProjectGraph, project_name: &str) -> (SheetNumbering, Vec<NumberingIssue>)`

- [ ] **Step 1: Wire the module and write the failing tests**

In `crates/signex-net/src/project/mod.rs`, next to the existing lines:

```rust
mod designators;
mod root_order;
mod sheet_numbers;

pub use designators::instance_entry_designators;
pub use sheet_numbers::{NumberingIssue, SheetNumbering, SheetPage, sheet_numbering};
```

In `crates/signex-net/src/lib.rs`, replace the `pub use project::{…}` block with:

```rust
pub use project::{
    NumberingIssue, ProjectGraph, ProjectNetlist, ProjectRoot, SheetKey, SheetNumbering,
    SheetOccurrence, SheetPage, StitchIssue, build_project_netlist, instance_entry_designators,
    project_occurrences, sheet_numbering,
};
```

Create `crates/signex-net/src/project/sheet_numbers.rs` with the types and a stub, so the tests compile and fail:

```rust
//! Project-level sheet numbering (#1081): one number per sheet placement the
//! project walk reaches. Design and rules:
//! `docs/superpowers/specs/2026-10-25-sheet-numbering-design.md`.

use signex_types::schematic::InstancePath;

use super::{ProjectGraph, SheetKey};

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
        self.pages
            .iter()
            .find(|page| &page.key == key && &page.instance_path == instance_path)
            .map(|page| page.number)
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
    _graph: &ProjectGraph,
    _project_name: &str,
) -> (SheetNumbering, Vec<NumberingIssue>) {
    (SheetNumbering::default(), Vec::new())
}

#[cfg(test)]
#[path = "sheet_numbers_tests.rs"]
mod tests;
```

Create `crates/signex-net/src/project/sheet_numbers_tests.rs`:

```rust
//! Sheet numbering on the spec's example tree (#1081): the root places Power
//! twice (A, B) and IO once (C); Power places LDO (L).

use std::collections::HashMap;

use signex_types::schematic::{
    ChildSheet, FillType, InstancePath, Point, SchematicSheet, SheetInstance,
};
use uuid::Uuid;

use super::*;
use crate::project::{ProjectGraph, ProjectRoot, SheetKey};

const A: u128 = 0xA;
const B: u128 = 0xB;
const C: u128 = 0xC;
const L: u128 = 0x1;

fn child(name: &str, filename: &str, uuid: u128) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::from_u128(uuid),
        name: name.to_string(),
        filename: filename.to_string(),
        position: Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn sheet(children: Vec<ChildSheet>) -> SchematicSheet {
    SchematicSheet {
        child_sheets: children,
        ..Default::default()
    }
}

fn path(chain: &[u128]) -> InstancePath {
    InstancePath::from_chain(chain.iter().map(|n| Uuid::from_u128(*n)).collect())
}

fn entry(path: &str, page: &str) -> SheetInstance {
    SheetInstance {
        project: String::new(),
        path: path.to_string(),
        page: page.to_string(),
    }
}

fn example() -> HashMap<String, SchematicSheet> {
    HashMap::from([
        (
            "root.snxsch".to_string(),
            sheet(vec![
                child("A", "power.snxsch", A),
                child("B", "power.snxsch", B),
                child("C", "io.snxsch", C),
            ]),
        ),
        (
            "power.snxsch".to_string(),
            sheet(vec![child("L", "ldo.snxsch", L)]),
        ),
        ("ldo.snxsch".to_string(), sheet(Vec::new())),
        ("io.snxsch".to_string(), sheet(Vec::new())),
    ])
}

/// Put `entry` on sheet symbol `symbol` of `file`.
fn with_entry(
    files: &mut HashMap<String, SchematicSheet>,
    file: &str,
    symbol: usize,
    entry: SheetInstance,
) {
    files.get_mut(file).expect("fixture file").child_sheets[symbol]
        .instances
        .push(entry);
}

/// Number `files` walked from `roots` (the first is the project root); each
/// sheet resolves its children by bare filename.
fn number(
    files: &HashMap<String, SchematicSheet>,
    roots: &[&str],
) -> (SheetNumbering, Vec<NumberingIssue>) {
    let sheets: HashMap<SheetKey, SchematicSheet> = files
        .iter()
        .map(|(name, sheet)| (SheetKey::new(name.clone()), sheet.clone()))
        .collect();
    let resolved: HashMap<SheetKey, HashMap<String, SheetKey>> = sheets
        .iter()
        .map(|(key, sheet)| {
            let submap = sheet
                .child_sheets
                .iter()
                .map(|cs| (cs.filename.clone(), SheetKey::new(cs.filename.clone())))
                .collect();
            (key.clone(), submap)
        })
        .collect();
    let roots: Vec<ProjectRoot> = roots
        .iter()
        .map(|root| ProjectRoot {
            key: SheetKey::new(*root),
        })
        .collect();
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    sheet_numbering(&graph, "p")
}

fn numbers(numbering: &SheetNumbering) -> Vec<usize> {
    numbering.pages().iter().map(|page| page.number).collect()
}

#[test]
fn pages_are_numbered_depth_first_from_the_root() {
    let (numbering, issues) = number(&example(), &["root.snxsch"]);
    let order: Vec<(String, InstancePath)> = numbering
        .pages()
        .iter()
        .map(|page| (page.key.to_string(), page.instance_path.clone()))
        .collect();
    assert_eq!(
        order,
        vec![
            ("root.snxsch".to_string(), path(&[])),
            ("power.snxsch".to_string(), path(&[A])),
            ("ldo.snxsch".to_string(), path(&[A, L])),
            ("power.snxsch".to_string(), path(&[B])),
            ("ldo.snxsch".to_string(), path(&[B, L])),
            ("io.snxsch".to_string(), path(&[C])),
        ]
    );
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert_eq!(numbering.count(), 6);
    assert_eq!(
        numbering.number_of(&SheetKey::new("ldo.snxsch"), &path(&[B, L])),
        Some(5)
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn a_manual_number_changes_only_its_own_page() {
    let mut files = example();
    // On L inside Power, for the Power placement B: numbers /B/L.
    with_entry(&mut files, "power.snxsch", 0, entry(&path(&[B]).to_string(), "9"));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 9, 6]);
    assert!(numbering.pages()[4].manual);
    assert!(!numbering.pages()[2].manual, "/A/L keeps its automatic number");
    assert_eq!(
        issues,
        vec![
            NumberingIssue::OutOfRange { page: 4, number: 9 },
            NumberingIssue::Gap { missing: vec![5] },
        ]
    );
}

#[test]
fn two_pages_with_one_number_clash() {
    let mut files = example();
    with_entry(&mut files, "root.snxsch", 2, entry("/", "2"));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 2]);
    assert_eq!(
        issues,
        vec![
            NumberingIssue::Clash {
                number: 2,
                pages: vec![1, 5],
            },
            NumberingIssue::Gap { missing: vec![6] },
        ]
    );
}

#[test]
fn an_unusable_manual_number_is_reported_and_ignored() {
    let mut files = example();
    with_entry(&mut files, "root.snxsch", 0, entry("/", "abc"));
    with_entry(&mut files, "root.snxsch", 1, entry("/", ""));
    with_entry(&mut files, "root.snxsch", 2, entry("/", "0"));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert_eq!(
        issues,
        vec![
            NumberingIssue::InvalidOverride {
                page: 1,
                text: "abc".to_string(),
            },
            NumberingIssue::InvalidOverride {
                page: 3,
                text: String::new(),
            },
            NumberingIssue::InvalidOverride {
                page: 5,
                text: "0".to_string(),
            },
        ]
    );
}

#[test]
fn an_entry_for_another_project_is_ignored() {
    let mut files = example();
    let mut other = entry("/", "2");
    other.project = "other".to_string();
    with_entry(&mut files, "root.snxsch", 2, other);
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert!(issues.is_empty(), "{issues:?}");

    let mut files = example();
    let mut ours = entry("/", "7");
    ours.project = "p".to_string();
    with_entry(&mut files, "root.snxsch", 2, ours);
    let (numbering, _) = number(&files, &["root.snxsch"]);
    assert_eq!(numbering.pages()[5].number, 7);
}

#[test]
fn only_a_slash_path_names_the_root_placement() {
    let mut files = example();
    with_entry(&mut files, "root.snxsch", 2, entry("", "7"));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6], "\"\" is an unset path");
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn a_page_the_root_does_not_reach_follows_the_hierarchy() {
    let mut files = example();
    files.insert("notes.snxsch".to_string(), sheet(Vec::new()));
    let (numbering, issues) = number(&files, &["root.snxsch", "notes.snxsch"]);
    assert_eq!(numbering.count(), 7);
    // Both sit at the empty path; the key tells them apart.
    assert_eq!(
        numbering.number_of(&SheetKey::new("notes.snxsch"), &path(&[])),
        Some(7)
    );
    assert_eq!(
        numbering.number_of(&SheetKey::new("root.snxsch"), &path(&[])),
        Some(1)
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn a_missing_or_cyclic_child_is_not_a_page() {
    let mut files = example();
    files
        .get_mut("ldo.snxsch")
        .expect("fixture file")
        .child_sheets
        .push(child("Back", "power.snxsch", 0xD));
    files
        .get_mut("io.snxsch")
        .expect("fixture file")
        .child_sheets
        .push(child("Gone", "gone.snxsch", 0xE));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert!(issues.is_empty(), "{issues:?}");
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p signex-net sheet_numbers`
Expected: FAIL, all 8 tests, on the stub's empty numbering (e.g. `left: [] right: [1, 2, 3, 4, 5, 6]`).

- [ ] **Step 3: Implement `sheet_numbering`**

In `sheet_numbers.rs`, change the imports and replace the stub with:

```rust
use std::collections::BTreeMap;

use signex_types::schematic::{InstancePath, SheetInstance};

use super::{ProjectGraph, SheetKey, walk_occurrences};
```

```rust
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
    text.trim().parse::<usize>().ok().filter(|number| *number > 0)
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
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p signex-net sheet_numbers`
Expected: PASS, 8 tests.

Run: `cargo clippy -p signex-net --all-targets --all-features -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 5: Commit**

```bash
git add crates/signex-net/src/project/sheet_numbers.rs crates/signex-net/src/project/sheet_numbers_tests.rs crates/signex-net/src/project/mod.rs crates/signex-net/src/lib.rs
git commit -m "feat(net): number sheet placements through the hierarchy (#1081)"
```

---

### Task 2: `sheet_cell_value` prints the page number

**Files:**
- Modify: `crates/signex-output/src/expression.rs:29-36` (`sheet_cell_value`) and its `#[cfg(test)] mod tests` (helper `snapshot`, ~line 172)

**Interfaces:**
- Consumes: `SheetSnapshot::sheet_number`.
- Produces: `pub fn sheet_cell_value(sheet: &SheetSnapshot) -> String` (signature unchanged) returning `sheet.sheet_number.to_string()`. `SchematicSheet::root_sheet_page` is no longer read for numbering; it still round-trips.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module of `crates/signex-output/src/expression.rs`:

```rust
#[test]
fn the_sheet_cell_is_the_page_number_not_the_stored_root_page() {
    let mut sheet = snapshot(Vec::new());
    sheet.schematic.root_sheet_page = "1".to_string();
    sheet.sheet_number = 5;
    assert_eq!(sheet_cell_value(&sheet), "5");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p signex-output the_sheet_cell_is_the_page_number`
Expected: FAIL with `left: "1"` / `right: "5"`.

- [ ] **Step 3: Implement**

Replace `sheet_cell_value` with:

```rust
/// The page's `cell` value (SVG, print preview, PDF content): its project
/// sheet number (#1081). The per-file `root_sheet_page` is not read; no UI
/// edits it, so it is `"1"` on every sheet.
pub fn sheet_cell_value(sheet: &SheetSnapshot) -> String {
    sheet.sheet_number.to_string()
}
```

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p signex-output`
Expected: PASS (no other test reads `root_sheet_page` through the cell).

- [ ] **Step 5: Commit**

```bash
git add crates/signex-output/src/expression.rs
git commit -m "feat(output): the sheet cell prints the page's sheet number (#1081)"
```

---

### Task 3: The export scope prints the project numbering

**Files:**
- Create: `crates/signex-app/src/app/handlers/menu/export/sheet_numbers.rs`
- Create: `crates/signex-app/src/app/handlers/menu/export/tests/sheet_numbering.rs`
- Modify: `crates/signex-app/src/app/handlers/menu/export/mod.rs` (module list ~line 7-16; `ExportIssues` ~line 78-95; `messages()` ~line 172-195; `build_scope` ~line 422-540)
- Modify: `crates/signex-app/src/app/handlers/menu/export/placements.rs` (doc comment of `page_copies`)
- Modify: `crates/signex-app/src/app/handlers/menu/export/tests.rs` (after `mod bom_hierarchy;`, ~line 1002)
- Create: `changelog.d/1081.added.md`

**Interfaces:**
- Consumes: `signex_net::{sheet_numbering, SheetNumbering, SheetPage, NumberingIssue}` (Task 1); `crate::app::project_sheets::sheet_key_with(path: &Path, base: Option<&Path>, fold: CaseFold) -> SheetKey`; `placements::page_copies`; fixtures `super::protel_hierarchy::placed_app(dir, listed, places, entries)`, `super::{open_with, sheet_with_net}`, `crate::app::handlers::menu::export::build_export_scope`.
- Produces:
  - `pub(super) fn numbered_pages(pages: Vec<SheetSnapshot>, numbering: &SheetNumbering, base_dir: Option<&Path>, fold: CaseFold) -> Vec<SheetSnapshot>`
  - `pub(super) fn numbering_messages(numbering: &SheetNumbering, issues: &[NumberingIssue]) -> Vec<String>`
  - `ExportIssues::numbering: Vec<String>`, appended by `ExportIssues::messages()`.

- [ ] **Step 1: Write the failing tests**

Create `crates/signex-app/src/app/handlers/menu/export/tests/sheet_numbering.rs`:

```rust
//! Project sheet numbering in the export scope (#1081): one number per sheet
//! placement, from the hierarchy, the same whichever pages an export prints.

use std::path::PathBuf;

use signex_types::schematic::SheetInstance;

use super::protel_hierarchy::placed_app;
use super::{open_with, sheet_with_net};
use crate::app::handlers::menu::export::build_export_scope;

/// `(page name, sheet number, sheet count)` for every printed page.
fn printed(app: &crate::app::Signex) -> Vec<(String, usize, usize)> {
    let (ctx, _) = build_export_scope(&app.document_state).expect("export scope");
    ctx.sheets
        .iter()
        .map(|page| (page.sheet_name.clone(), page.sheet_number, page.sheet_count))
        .collect()
}

/// The export's `Sheet numbering:` Messages lines (the stitcher's own lines,
/// such as the twice-placed child's shared designator, are left out).
fn numbering_lines(app: &crate::app::Signex) -> Vec<String> {
    let (_, issues) = build_export_scope(&app.document_state).expect("export scope");
    issues
        .messages()
        .into_iter()
        .filter(|line| line.starts_with("Sheet numbering:"))
        .collect()
}

/// Put `entry` on `top.snxsch`'s sheet symbol `symbol`.
fn with_top_entry(app: &mut crate::app::Signex, dir: &str, symbol: usize, entry: SheetInstance) {
    let top_path = PathBuf::from(dir).join("top.snxsch");
    let mut top = app.document_state.engines[&top_path].document().clone();
    top.child_sheets[symbol].instances.push(entry);
    open_with(&mut app.document_state, &top_path, top);
}

fn root_entry(page: &str) -> SheetInstance {
    SheetInstance {
        project: String::new(),
        path: "/".to_string(),
        page: page.to_string(),
    }
}

#[test]
fn pages_take_their_number_from_the_hierarchy_not_the_declared_list() {
    let dir = "/w/numbering-order";
    let mut app = placed_app(
        dir,
        &["top.snxsch", "other.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("other.snxsch"),
        sheet_with_net("R_OTHER", "OTHER_NET", &[]),
    );
    assert_eq!(
        printed(&app),
        vec![
            ("top.snxsch".to_string(), 1, 4),
            ("other.snxsch".to_string(), 4, 4),
            ("child.snxsch (A)".to_string(), 2, 4),
            ("child.snxsch (B)".to_string(), 3, 4),
        ]
    );
}

#[test]
fn the_count_includes_placements_that_are_not_printed_pages() {
    let app = placed_app("/w/numbering-count", &["top.snxsch"], &["A", "B"], Vec::new());
    assert_eq!(printed(&app), vec![("top.snxsch".to_string(), 1, 3)]);
}

#[test]
fn a_subset_export_keeps_the_project_numbers() {
    let app = placed_app(
        "/w/numbering-subset",
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    let (mut ctx, _) = build_export_scope(&app.document_state).expect("export scope");
    ctx.sheets.retain(|page| page.sheet_name == "child.snxsch (B)");
    let only: Vec<(usize, usize)> = ctx
        .sheets
        .iter()
        .map(|page| (page.sheet_number, page.sheet_count))
        .collect();
    assert_eq!(only, [(3, 3)]);
}

#[test]
fn a_manual_number_is_printed_and_reported() {
    let dir = "/w/numbering-manual";
    let mut app = placed_app(dir, &["top.snxsch", "child.snxsch"], &["A", "B"], Vec::new());
    with_top_entry(&mut app, dir, 1, root_entry("7"));
    assert_eq!(
        printed(&app),
        vec![
            ("top.snxsch".to_string(), 1, 3),
            ("child.snxsch (A)".to_string(), 2, 3),
            ("child.snxsch (B)".to_string(), 7, 3),
        ]
    );
    assert_eq!(
        numbering_lines(&app),
        vec![
            "Sheet numbering: page B is numbered 7, above the page count 3".to_string(),
            "Sheet numbering: no page prints number 3".to_string(),
        ]
    );
}

#[test]
fn an_unusable_manual_number_is_reported_and_ignored() {
    let dir = "/w/numbering-invalid";
    let mut app = placed_app(dir, &["top.snxsch", "child.snxsch"], &["A", "B"], Vec::new());
    with_top_entry(&mut app, dir, 0, root_entry("abc"));
    assert_eq!(printed(&app)[1], ("child.snxsch (A)".to_string(), 2, 3));
    assert_eq!(
        numbering_lines(&app),
        vec![
            "Sheet numbering: page A has the manual number 'abc', which is not a positive whole \
             number; it is numbered 2"
                .to_string()
        ]
    );
}
```

In `crates/signex-app/src/app/handlers/menu/export/tests.rs`, after `mod bom_hierarchy;`:

```rust
mod sheet_numbering;
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p signex-app --lib sheet_numbering`
Expected: FAIL. `pages_take_their_number…` gets export-order numbers (`("other.snxsch", 2, 4)`); `the_count_includes…` gets `("top.snxsch", 1, 1)`; the two manual-number tests get no `Sheet numbering:` lines. `a_subset_export_keeps_the_project_numbers` already passes: it pins that the subset filter runs after numbering.

- [ ] **Step 3: Add the app-side numbering module**

Create `crates/signex-app/src/app/handlers/menu/export/sheet_numbers.rs`:

```rust
//! The project's sheet numbering (#1081) applied to the printed pages, and its
//! issues as Messages lines. The numbers come from `signex_net::sheet_numbering`.

use std::path::Path;

use signex_net::{NumberingIssue, SheetNumbering, SheetPage};
use signex_output::SheetSnapshot;

use crate::app::state::scope::CaseFold;

/// `pages` with the project's numbers: each page takes its placement's number
/// and the project's page count, so a page prints the same "Sheet N of M"
/// whichever pages an export includes. A page the numbering does not know
/// keeps its number.
pub(super) fn numbered_pages(
    pages: Vec<SheetSnapshot>,
    numbering: &SheetNumbering,
    base_dir: Option<&Path>,
    fold: CaseFold,
) -> Vec<SheetSnapshot> {
    pages
        .into_iter()
        .map(|page| {
            let key = crate::app::project_sheets::sheet_key_with(&page.path, base_dir, fold);
            match numbering.number_of(&key, &page.placement) {
                Some(number) => SheetSnapshot {
                    sheet_number: number,
                    sheet_count: numbering.count(),
                    ..page
                },
                None => page,
            }
        })
        .collect()
}

/// One Messages line per issue, each starting `Sheet numbering:`.
pub(super) fn numbering_messages(
    numbering: &SheetNumbering,
    issues: &[NumberingIssue],
) -> Vec<String> {
    let page = |index: usize| numbering.pages().get(index);
    let label = |index: usize| page(index).map(SheetPage::label).unwrap_or_default();
    issues
        .iter()
        .map(|issue| match issue {
            NumberingIssue::Clash { number, pages } => {
                let names: Vec<String> = pages.iter().map(|index| label(*index)).collect();
                format!(
                    "Sheet numbering: pages {} all print number {number}",
                    names.join(", ")
                )
            }
            NumberingIssue::Gap { missing } => {
                let noun = if missing.len() == 1 { "number" } else { "numbers" };
                let list: Vec<String> = missing.iter().map(usize::to_string).collect();
                format!("Sheet numbering: no page prints {noun} {}", list.join(", "))
            }
            NumberingIssue::OutOfRange { page: index, number } => format!(
                "Sheet numbering: page {} is numbered {number}, above the page count {}",
                label(*index),
                numbering.count()
            ),
            NumberingIssue::InvalidOverride { page: index, text } => format!(
                "Sheet numbering: page {} has the manual number '{text}', which is not a \
                 positive whole number; it is numbered {}",
                label(*index),
                page(*index).map_or(0, |found| found.number)
            ),
        })
        .collect()
}
```

- [ ] **Step 4: Wire it into `export/mod.rs`**

Module list (after `mod print_preview;`):

```rust
mod sheet_numbers;
```

`ExportIssues`, after the `unreadable` field:

```rust
    /// Sheet-numbering problems (#1081), one Messages line each. They never
    /// stop an export.
    pub(crate) numbering: Vec<String>,
```

End of `ExportIssues::messages()`, just before the final `out`:

```rust
        out.extend(self.numbering.iter().cloned());
```

In `build_scope`, next to `let mut page_keying …`:

```rust
    let mut page_numbering: Option<signex_net::SheetNumbering> = None;
```

Inside the netlist closure, right after `page_keying = Some((base_dir.clone(), fold));` (the `project` graph is still borrowed there; `graph.sheets` is moved only further down):

```rust
        // One project numbering (#1081), from the same walk; loose documents
        // keep the export-order numbers.
        if let Some(owner) = owning_project {
            let (numbering, numbering_issues) =
                signex_net::sheet_numbering(&project, &owner.data.name);
            issues.numbering = sheet_numbers::numbering_messages(&numbering, &numbering_issues);
            page_numbering = Some(numbering);
        }
```

Replace the `let sheets = match page_keying { … };` block with:

```rust
    let sheets = match page_keying {
        Some((base_dir, fold)) => {
            let copies =
                placements::page_copies(sheets, &page_occurrences, base_dir.as_deref(), fold);
            match &page_numbering {
                Some(numbering) => {
                    sheet_numbers::numbered_pages(copies, numbering, base_dir.as_deref(), fold)
                }
                None => copies,
            }
        }
        None => sheets,
    };
```

In `placements.rs`, in the doc comment of `page_copies`, replace the last sentence ("Numbering is only redone when a page gained copies, so a project of singly placed pages keeps its numbers.") with:

```rust
/// When a page gained copies the pages are renumbered in order; that is the
/// numbering a loose document keeps. A project's pages are then renumbered
/// from the hierarchy (`sheet_numbers::numbered_pages`, #1081).
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p signex-app --lib export`
Expected: PASS, including the 5 `sheet_numbering` tests and every existing export test (`current_page_tests`, `protel_hierarchy`, `bom_hierarchy`, `source_tests`).

If an existing test asserted export-order numbers for a project page, read it before changing it: a project page's number now comes from the hierarchy, so the expectation changes only where a page's hierarchy position differs from its list position, or where the project has placements that are not printed pages. Name each such change in the commit message.

- [ ] **Step 6: Changelog fragment**

Create `changelog.d/1081.added.md`:

```markdown
- **Sheet N of M follows the hierarchy (#1081)** — every placement of a sheet in a project gets one number, depth-first from the root, and `${SHEETNUMBER}` / `${SHEETCOUNT}` and the sheet cell print it in every export, whichever pages it includes. A manual number in a sheet symbol's placement entry overrides it; clashes, gaps and invalid numbers are reported in the Messages panel.
```

- [ ] **Step 7: Commit**

```bash
git add crates/signex-app/src/app/handlers/menu/export changelog.d/1081.added.md
git commit -m "feat(app): exports print the project's sheet numbering (#1081)"
```

---

### Task 4: Full gate run and PR

**Files:** none new.

- [ ] **Step 1: Bring the branch up to date**

```bash
git fetch origin trunk && git merge --no-edit origin/trunk
```

- [ ] **Step 2: Run every required gate**

```bash
git grep -niE 'kicad' -- 'crates/**/*.rs' 'crates/**/*.toml'   # prints nothing
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
bash scripts/check_file_size.sh
bash scripts/check_discarded_tasks.sh
bash scripts/check_lint_lockstep.sh
```

Expected: every command exits 0; `cargo test` reports 0 failed.

- [ ] **Step 3: Open the PR**

Push `feat/1081-sheet-numbering` and open a PR against `trunk`: title `feat: number sheets through the hierarchy, the same in every export (#1081)`, body with `Closes #1081`, the three spec deviations above, the test list and the gate results; labels `type: feature`, `area: output`, `area: app`; milestone as on #1081. No AI attribution.
