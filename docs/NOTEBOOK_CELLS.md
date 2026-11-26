# Design Notebook Python cells

A Python cell (mockup row 200) computes from schematic values and prints its
result into the notebook page: the values it read, each named with its
schematic source, what it printed and its figures as numbered vector plots.
The rules live in the UI-free `signex-notebook` crate, module `cell`
(`crates/signex-notebook/src/cell/`); the Python runtime is a separate piece
behind the `CellRunner` trait (see "What is not built yet").

## The cell

```text
== Ripple versus input voltage <sec:rvin>

#python(label: "ripple-vs-vin")[```python
from signex import component, units as u
L = component("L1").value          # 4.7 µH
esr = component("C15").param("ESR") # 5 mΩ
print(L)
```]
```

- The cell is a `#python` call whose body opens with a raw block of three or
  more backticks with the language `python`. The indent its lines share is
  taken off, so a cell may be indented inside the brackets.
- `label:` names the cell to the cache, the page and the run commands. A cell
  without one is not run and its page says so. Labels are unique across the
  notebook and its chapters; a second cell with a used label is not run.
- Every `#python` call that is not a runnable cell is a problem on its line:
  no label, a label that is not a quoted string, a used label, no raw block,
  a raw block in another language, an argument a cell does not take.
- The file stays plain Typst: the bundled `@signex/notebook` package defines
  `python`. A cell needs no new `.snxnbk` token.

## Values with units

`component("L1").value` reads the part's value and `.param("ESR")` a part
parameter. A value arrives as a quantity with its unit, so the arithmetic in
the cell keeps the units. The crate reads the value text as written in the
schematic: `4.7µH`, `22 uF`, `5mΩ`, `500kHz`, the letter-for-point forms
`4k7` and `2R2`, and a bare number, which takes the unit of its part kind
(`R` ohms, `C` farads, `L` henries). It prints a value in engineering form
with three significant digits at most (`4.7 µH`, `16.7 mV`). The values a
cell reads are found in its source before it runs (`value_reads`); the
runtime may report the values it actually read, which then replace them.

## When cells run

Cells run only on **Run cell** (Shift+Enter in the mockup), **Run all**
(Ctrl+Shift+Enter) and **PDF export**, never as the text is typed, and
compiling the page never runs a cell (DECISIONS, 2026-11-25).

- **Run cell** runs that cell, cached or not.
- **Run all** and export run every cell that has no result or an out-of-date
  one; every other cell keeps its cached result.
- A cell's result is kept with the hash of the source it ran, the values it
  read and the runtime it ran in. When one of them changes, the cell is
  **out of date**: its result stays on the page, greyed, with the change
  named ("C15 changed from 22 µF to 47 µF in the schematic") until it runs
  again. A change to a part the cell does not read leaves it current.
- A run that was stopped or ran past its time limit keeps the last result.
- The run answer (`RunReport`) is JSON: the trigger, the runtime, each cell
  with its state, and the counts the status bar shows (cells, ran, cached,
  out of date, blocked, failed, not run).

The states a cell shows: not run yet, ran (`ran in 212 ms · 14:20`), cached
(`cached · inputs unchanged since 2026-11-24 14:02`), out of date, blocked
by the sandbox (`blocked, line 141`), error, not run without a runtime,
waiting for permission, and not allowed.

## The cache

The project keeps the last result of each cell in
`.signex/notebook-cells.json`, beside the other files Signex keeps for a
project (`docs/FORMAT_VERSIONS.md`). It is a cache, not a document: a file
this build cannot read, or one with another format token, is set aside, and
the cells show as not run until they run. The same results write the same
file. A deleted cell's result leaves the file on the next run.

## Sandbox

Cells can read and write files inside their project only. They cannot start
programs (`subprocess`, `os.system`, `os.exec*`, `os.fork`, ...), open
network connections or look up hosts, or open, list, change or remove files
outside the project folder. The runtime's own library folder may be read,
never written. A refused attempt raises a `PermissionError` with one plain
sentence; only that cell fails and the rest of the page compiles.

The runtime reports each attempt as a Python audit event; the crate decides
(`SandboxPolicy::check_event`, `classify`). Paths are compared by their
folder names after `.` and `..` are applied, `\` is read as `/` and a drive
letter is compared in lower case; a relative path is taken from the project
folder. The runtime hands over paths with links resolved.

This guards against a cell doing these things by accident or by habit. It is
not a wall against code written to escape: in-process Python can reach the
machine through native code. Each refusal is pinned by a test.

## A notebook from outside the project

A notebook that is not inside the project folder does not run until the
person allows it. Before its first cell runs, the run answers that it needs
permission and no cell runs; the answer is kept per notebook, by its path,
with the person's preferences and never in a project (`TrustAnswers`).

## The slim install

A copy of Signex without the Python runtime runs no cell. Each cell shows
its last cached result greyed, with "Python is not installed" and the
date of that result, and the page still compiles. The app offers Install
Python runtime… and Preferences ▸ Notebook.

## In print

The Python source is hidden in print: the page shows the result only. An
export option prints the source above each result (`PageCells::print_source`,
for the Export PDF dialog of mockup row 202).

## What is not built yet

- The embedded Python runtime: the `CellRunner` that runs a cell in CPython
  with the `signex` module (`component`, `units`), the units library, the
  audit hook that hands each event to `SandboxPolicy::check_event`, figure
  capture as SVG, the time limit and Esc to stop. Until it lands, the app
  runs cells with `NoRuntime`, the slim-install state.
- The installer for the runtime and its Preferences page.
- The notebook tab's cell toolbar, status-bar line and permission prompt,
  and the catalog commands for Run cell, Run all and stop, which belong with
  the notebook tab and its menu rows.
- The dashed preview of the new value on an out-of-date plot.
