# Sheet N of M: project-level sheet numbering (#1081)

Status: design approved 2026-10-25. Implementation starts after #979 merges.
Part of epic #1084.

## Problem

`${SHEETNUMBER}` / `${SHEETCOUNT}` resolve only in PDF export
(`signex-output/src/substitution.rs`), and the number is the export order:
`sheet_number: i + 1` over the `.snxprj` page list in `owned_pages`
(`signex-app/src/app/handlers/menu/export/mod.rs`). The numbering does not
follow the hierarchy, a child sheet placed twice has no number per placement,
and an export of a subset of the sheets renumbers them, so "Sheet 3 of 12" on
paper need not match the project.

`sheet_cell_value` (`signex-output/src/expression.rs`), which feeds the SVG
`cell` value, prefers the per-file manifest string `root_sheet_page`. That
string defaults to `"1"` and no UI edits it, so `cell` is `1` on every sheet
today.

## Goals

- One project-level numbering, computed in a domain crate, used by every
  output.
- Hierarchy-aware: depth-first from the root, and each placement of a reused
  child is its own page.
- Independent of which sheets an export includes.
- An optional manual number per placement, with Messages-panel warnings for
  clashes, gaps and bad values.

## Non-goals

- Drawing the number on the canvas. The canvas has no title block yet; #1079
  draws it and will call the same function (see "Canvas" below).
- A title block in SVG export. SVG has no title-block substitution today; the
  number reaches SVG only through `cell`. Adding a title block there is a
  separate change.
- A numbering UI or a "Number Sheets" command. Overrides are data in the
  sheet symbol; editing them is a later UI change.
- Any change to `.snxsch` / `.snxprj` bytes. Every field used already exists
  and round-trips.

## Dependencies

- **#979** adds `InstancePath` (the chain of `ChildSheet.uuid` values from the
  walk root; the root is the empty path, written `/uuid/uuid`) and
  `signex_net::project_occurrences(graph) -> Vec<SheetOccurrence>`, a pure,
  deterministic pre-order walk over every placement. This design is built on
  both. It also rewrites the same `SheetSnapshot` constructors and
  `owned_pages` / `page_copies` lines, so implementing before #979 lands would
  duplicate the walk and conflict with it.
- **#1079** (canvas frame and title block) is the consumer on screen.

## Numbering rules

1. **Pages.** A page is a placement that the project walk reaches
   (`project_occurrences`). A child sheet file placed twice is two pages.
2. **Order.** Depth-first pre-order from the root, which is page 1. Children
   follow the parent's `child_sheets` order, which `.snxsch` saves in order.
   Pages that the root does not reach follow the hierarchy, in the walk's root
   order (the order the netlist uses).
3. **Count.** M is the number of pages in the project. It does not depend on
   which pages an export includes.
4. **Automatic number.** A page's automatic number is its position in that
   order, 1..=M.
5. **Manual number.** The override is stored in the parent sheet's sheet
   symbol, as an existing `ChildSheet.instances[]` entry (`SheetInstance {
   project, path, page }`, unused today):
   - The page placed by symbol `S` inside parent placement `P` (page path
     `P + S.uuid`) takes the entry of `S.instances` whose `path` parses
     (`InstancePath::parse`) to `P`. Parsing makes `""` and `"/"` both the
     root.
   - An entry applies only when its `project` is empty or equals the project
     name, so one `.snxsch` used by two projects keeps their numbers apart.
   - `page` must be a positive integer. It replaces that page's number only.
     Other pages keep their automatic numbers; nothing cascades.
   - The root has no parent symbol and is always 1.
6. **Unreached sheets.** A missing or cyclic child is not reached, gets no
   number and does not count toward M. The stitcher already reports it.
7. **Sheets outside a project.** Loose tabs (`loose_pages`) and an undeclared
   sheet viewed on its own (`isolate_undeclared_current`) keep today's
   numbering.
8. **`root_sheet_page`.** No longer read for numbering. The field still
   round-trips.

Example: the root places `Power.snxsch` twice (symbols `A`, `B`) and
`IO.snxsch` once (`C`); `Power.snxsch` places `LDO.snxsch` (symbol `L`).

```
/       Root    1
/A      Power   2
/A/L    LDO     3
/B      Power   4
/B/L    LDO     5
/C      IO      6      M = 6
```

To number `/B/L` by hand, write the entry with `path = "/B"` on symbol `L` in
`Power.snxsch`. The entry with `path = "/A"` numbers `/A/L`.

## Domain API (`signex-net`, new `src/project/sheet_numbers.rs`)

```rust
pub struct SheetNumbering { pages: Vec<SheetPage>, count: u32 }

pub struct SheetPage {
    pub instance_path: InstancePath,
    pub key: SheetKey,
    pub number: u32,
    pub manual: bool,
}

pub enum NumberingIssue {
    Clash { number: u32, pages: Vec<InstancePath> },
    Gap { missing: Vec<u32> },
    OutOfRange { page: InstancePath, number: u32 },
    InvalidOverride { page: InstancePath, text: String },
}

pub fn sheet_numbering(graph: &ProjectGraph, project_name: &str)
    -> (SheetNumbering, Vec<NumberingIssue>);

impl SheetNumbering {
    pub fn number_of(&self, page: &InstancePath) -> Option<u32>;
    pub fn count(&self) -> u32;
}
```

The function is pure (no IO) and deterministic: the same graph gives the same
numbers and the same issues, in the same order.

## Wiring

**Export (`signex-app`, `export/mod.rs` `build_scope`).** After #979,
`build_scope` already calls `project_occurrences`. Call `sheet_numbering`
once beside it. For a project export, every `SheetSnapshot` takes
`sheet_number = number_of(&snapshot.placement)` and `sheet_count = count()`.
This replaces `i + 1` in `owned_pages` and the export-order renumbering in
#979's `page_copies`. The app reads the numbers; it does not compute them.

**Output (`signex-output`).**
- The PDF title block (`pdf/content.rs`, `SubstitutionContext`) already takes
  number and count from `SheetSnapshot`. No code change.
- `sheet_cell_value` returns `sheet_number` instead of `root_sheet_page`. That
  value feeds the SVG `cell`, the print preview and the PDF content.
- PDF bookmarks ("Sheet {n}: {name}") and the preview `page_number` read
  `sheet_number` already.
- The `physical_sheet_number` option keeps working as it does today.

**Canvas (#1079).** `sheet_numbering` needs the `ProjectGraph`, and
`assemble_project_sheets` reads disk to build it. #1079 must cache the result
(the way `navigator_ctx` caches the hierarchy), never compute it in `view`.

## Reporting

Every `NumberingIssue` becomes one `diagnostics::log_warning` line in the
Messages panel, once per export run. Lines start with `Sheet numbering:` and
stay under the panel's ~157-byte cut. Pages are named by sheet name, with the
placement ordinal when the file is placed more than once ("Power (2)"). None
of the issues stops an export.

| Issue | Effect |
|---|---|
| Clash | Both pages print the number; the warning names both. |
| Gap | The warning lists the missing numbers. |
| OutOfRange (> M) | The number is used; warning. |
| InvalidOverride (`abc`, `0`, empty) | Ignored, automatic number used; warning. |
| Stale entry (path matches no placement) | Ignored silently: unused data, not an error. |

## Tests (red first)

Domain (`signex-net`, `sheet_numbers_tests.rs`), on the example tree:
1. Numbers 1-6, count 6, order `/`, `/A`, `/A/L`, `/B`, `/B/L`, `/C`.
2. Override `/B/L` = 9: only that page changes; `OutOfRange` and `Gap` issues.
3. Override `/C` = 2: `Clash { 2, [/A, /C] }`.
4. Overrides `abc` and `0`: automatic numbers, `InvalidOverride` issues.
5. An entry with another project's name is ignored.
6. Root entry path `""` and `"/"` both match.
7. Pages that the root does not reach are numbered after the hierarchy.
8. A cyclic or missing child is not counted.

Output (`signex-output`): `sheet_cell_value` returns `sheet_number` when
`root_sheet_page` is `"1"`.

App (`tests/regression`):
- Exporting 2 of the 6 pages: the PDF title block of the `/B/L` page reads
  "Sheet 5 of 6".
- The two placements of `LDO.snxsch` export with different numbers.

## Acceptance

- `${SHEETNUMBER}` / `${SHEETCOUNT}` give the same values for a page in every
  project export, whatever subset is exported.
- Each placement of a reused child has its own number.
- A valid manual number is honoured; clashes, gaps and bad values reach the
  Messages panel.
- No `.snxsch` / `.snxprj` byte changes; round-trip corpus tests unchanged.
