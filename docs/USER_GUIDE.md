# Signex user guide

This guide takes you from a downloaded Signex package to a saved schematic
and an exported PDF. It assumes you have never used an EDA tool and have never
installed Rust. You do not need either.

It describes the application as it is on the `trunk` branch. Where a step
depends on work that has not shipped yet, the guide says so and links the
tracking issue. Menu paths are written `Menu ▸ Item`.

Contents:

1. [Install](#1-install)
2. [First launch](#2-first-launch)
3. [Create a project](#3-create-a-project)
4. [Canvas basics](#4-canvas-basics)
5. [Place a symbol](#5-place-a-symbol)
6. [Wire](#6-wire)
7. [Label a net](#7-label-a-net)
8. [Annotate](#8-annotate)
9. [Run ERC](#9-run-erc)
10. [Save](#10-save)
11. [Export PDF and BOM](#11-export-pdf-and-bom)
12. [File types](#12-file-types)
13. [Keyboard shortcuts](#13-keyboard-shortcuts)
14. [Where to get help](#14-where-to-get-help)

## 1. Install

Follow the [Download section of the README](../README.md#download). It lists
the package for each platform, the first-launch warnings on Windows and macOS,
and how to verify a download. This guide does not repeat it.

## 2. First launch

Start Signex. With no document open you see a start screen with three
buttons:

- **New Project**
- **Open Project…** (with its keyboard shortcut shown next to it)
- **Keyboard Shortcuts (F1)**

On your very first launch a small card titled **Welcome to Signex** also
appears in the bottom-right corner of the window. It lists the three gestures
you need immediately:

- Right-click + drag to pan
- Scroll to zoom
- F1 for keyboard shortcuts

Close it with the ✕ in its corner or with `Esc`. It is a reminder only; nothing
else changes when you dismiss it.

`F1` (or **Help ▸ Keyboard Shortcuts**) opens a reference of every binding in
the active shortcut profile.

## 3. Create a project

Choose **File ▸ New Project** (or press **New Project** on the start screen).

A save dialog titled **New Signex Project** opens with the name
`Untitled.snxprj`. Pick a folder and keep the `.snxprj` ending. Use a folder
you create for this project.

Signex then writes two files into that folder:

| File | What it is |
|---|---|
| `<name>.snxprj` | The project file. It is created empty; Signex builds the project from the files in the folder. |
| `<name>.snxsch` | A blank schematic sheet, A4 paper size. |

The project loads and the blank schematic opens in a tab, ready to draw on.
If a non-empty `.snxprj` already exists at the path you chose, Signex refuses
to overwrite it; open it instead with **File ▸ Open…** (the dialog accepts
`.snxprj` and `.snxsch`).

To come back later use **File ▸ Open…** or **File ▸ Open Recent**.

> **Sample project.** A bundled sample project (**Help ▸ Open Sample
> Project**) is proposed in [#787](https://github.com/alplabai/signex/pull/787)
> and is not available yet.

## 4. Canvas basics

- **Pan:** hold the right mouse button and drag. The middle button also pans.
  A right-click without dragging opens the context menu, unless you are in the
  middle of drawing a wire, where it cancels the wire.
- **Zoom:** scroll the mouse wheel. Zoom is centred on the pointer.
- **Fit the sheet:** **View ▸ Fit All**.
- **Grid:** the status bar at the bottom shows the grid size and whether
  snapping is on (**Snap**) or off (**Free**). Click the grid value to show or
  hide the grid, and click **Snap/Free** to switch snapping. **View ▸ Toggle
  Grid** does the same as the grid click. Placed objects snap to the grid
  while snapping is on.

Changing the grid size from the keyboard or from **View ▸ Cycle Grid Size** has
no effect on a schematic yet. **View ▸ Zoom In** and **Zoom Out** are also not
wired; use the scroll wheel.

## 5. Place a symbol

A symbol is a part on the sheet: a resistor, a connector, a chip. Symbols come
from libraries. The Components panel (**View ▸ Components**) lists the
libraries that are mounted, and **File ▸ Library ▸ Open Library…** mounts a
`.snxlib` library.

**Placing a library symbol onto a sheet does not work yet.** The component
picker (**File ▸ Library ▸ Place Component…**) and the Components panel
list and select parts, but the **Place** step closes the dialog without putting
anything on the sheet. This is tracked in
[#364](https://github.com/alplabai/signex/issues/364). Until it lands, a
schematic made by following this guide contains wires and labels, not parts.

A bundled stock library that mounts on first launch is also not shipped yet
([#328](https://github.com/alplabai/signex/issues/328)). On a fresh install the
Components panel is empty and says "No libraries mounted."

When #364 lands, this section will describe the placement steps.

## 6. Wire

Wires connect pins and each other.

1. Choose the wire tool: **Place ▸ Wire**, or the keyboard shortcut. In the
   default profile (see [section 13](#13-keyboard-shortcuts)) press `P` then
   `W`, one after the other, or press `Ctrl+W`. With the Classic profile press
   `W`.
2. Click on the sheet to start a wire.
3. Click again to end the segment. Each click commits a segment and starts the
   next from the same point.
4. Double-click to stop drawing. `Esc` or a right-click cancels the wire you
   are drawing.

Wires snap to the grid while snapping is on. The status bar shows the active
tool.

A schematic made by following this guide has no placed parts (section 5), so
there are no pins to connect the wires to yet.

## 7. Label a net

A net label gives a wire a name. Wires with the same label are connected, even
if they are not drawn touching.

1. Choose **Place ▸ Net Label**, or press `P` then `N` in the default profile
   (`L` in the Classic profile).
2. Click where you want the label. It is placed with the default text `NET`.
3. Press `Esc` to leave the tool, then double-click the label, type the new
   name, and press `Enter`.

## 8. Annotate

Annotation gives every symbol a unique reference designator such as `R1`, `R2`.
It acts on placed symbols, so with none on the sheet there is nothing to
annotate (section 5).

- **Design ▸ Annotation ▸ Annotate Schematics…** opens the annotation dialog.
  Its right side lists the proposed changes (current and proposed designator
  for each symbol). Press **Accept Changes** to apply them, then **Close**.
- **Design ▸ Annotation ▸ Annotate Schematics Quietly** (`Alt+A`) applies the
  changes without opening the dialog.
- **Reset & Renumber** in the dialog, or **Force Annotate All Schematics**
  (`Shift+Alt+A`), renumbers everything from scratch.

Parts of the dialog are marked in the dialog itself as not active yet (the
multi-sheet columns and the parameter-matching checkboxes).

## 9. Run ERC

ERC (electrical rules check) scans the design for wiring mistakes, such as
pins that connect nothing or two outputs wired together.

1. Press `F8`. This runs the check on the project immediately.
   Alternatively choose **Design ▸ Electrical Rules Check**, which opens a
   dialog where you can set the severity of each rule and press **Run ERC**.
2. The **ERC** panel opens with the results. Its header shows the number of
   errors, warnings and info items, and `<` and `>` step through them.
3. Each row shows a severity letter (`E` error, `W` warning, `I` info), the
   rule, the message, and the sheet and position. Click a row to jump to the
   item on the canvas. The button at the end of a row either says **Show on
   Canvas** or, for an unused pin, **Add No-Connect**, which places a
   no-connect marker on that pin.
4. **Clear** empties the list.

The **Messages** panel (**View ▸ Messages**) is the application's runtime
log, not the ERC results. Read ERC results in the ERC panel.

Before you run the check the panel shows "No ERC diagnostics yet". Run `F8`
again after each round of edits.

## 10. Save

Press `Ctrl+S` or choose **File ▸ Save**. Signex writes the sheet to its
`.snxsch` file. **File ▸ Save As…** saves a copy under a new name. Both are
greyed out when no schematic is open.

Edits are held in memory until you save. Undo is `Ctrl+Z` and redo is
`Ctrl+Y`.

## 11. Export PDF and BOM

Both exports are under **File ▸ Export** and need a schematic open.

**PDF**

1. Choose **File ▸ Export ▸ PDF…** (or press `Ctrl+P`).
2. A preview window opens with **Preview** and **Settings** tabs. Choose colour
   or gray, which pages, and the other options.
3. Press **Export PDF**. A save dialog titled **Export PDF** opens with the
   name `schematic.pdf`. Choose where to save.

**BOM (bill of materials)**

1. Choose **File ▸ Export ▸ Bill of Materials…**.
2. A preview of the table opens. Adjust the options and press **Export…**.
3. In the save dialog, name the file. The format can be CSV (`.csv`), Excel
   (`.xlsx`) or HTML (`.html`).

A schematic without placed parts produces an empty BOM.

The same menu also offers **Netlist (Protel .NET)…** and **ERC Report…**
(saved as `erc-report.txt` by default).

## 12. File types

| Extension | What it is |
|---|---|
| `.snxprj` | A Signex project. Open this to load a project. |
| `.snxsch` | One schematic sheet. This is what you edit and save. |
| `.snxlib` | A component library. It is a folder, not a single file. |
| `.snxsym` | A schematic symbol, edited in the symbol editor. |
| `.snxfpt` | A PCB footprint, edited in the footprint editor. |

## 13. Keyboard shortcuts

Signex ships two shortcut profiles, **Altium** and **Classic**. Altium is
the default. Change the profile, or edit any binding, in **Tools ▸
Preferences…** (`Ctrl+,`) under the keyboard shortcuts page.

A binding written with spaces, such as `P W`, is a sequence: press `P`, release
it, then press `W`. A binding written with `+`, such as `Ctrl+S`, is pressed
together.

Shortcuts used in this guide, in the default Altium profile:

| Action | Shortcut |
|---|---|
| Open | `Ctrl+O` |
| Save | `Ctrl+S` |
| Print / export PDF | `Ctrl+P` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Place wire | `P W` or `Ctrl+W` |
| Place net label | `P N` |
| Cancel the current tool | `Esc` |
| Rotate selection | `Space` |
| Mirror selection | `X` / `Y` |
| Delete selection | `Delete` |
| Run ERC | `F8` |
| Annotate | `T A` |
| Annotate quietly | `Alt+A` |
| Toggle visible grid | `Ctrl+Shift+G` |
| Keyboard shortcut reference | `F1` |
| Preferences | `Ctrl+,` |

The Classic profile uses single keys, for example `W` for wire and `L` for
label. The complete lists are in the
[command reference](COMMANDS.md), generated from the shipped profiles.

There is no default shortcut for **File ▸ New Project** in the Altium profile.

## 14. Where to get help

- Ask a question or share an idea in
  [Discussions](https://github.com/alplabai/signex/discussions).
- Report a bug or request a feature in the
  [issue tracker](https://github.com/alplabai/signex/issues). **Help ▸ Report
  an Issue** opens it from inside the application.
