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

For the whole flow in one pass, see the
[walkthrough](#walkthrough-a-saved-schematic-with-a-wired-symbol).

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
from libraries.

**Libraries.** The Components panel (**View ▸ Components**) lists mounted
libraries in four sections: **Project**, **Installed**, **Global** and
**Stock**. The **Stock** section is the library that ships with Signex. It is
mounted on every launch; you do not add it. If the installer's copy cannot be
found, the section says "Stock library not found. Reinstall Signex to restore
the bundled parts." The **Installed** and **Global** sections have a
**+ Add Library…** button, and **File ▸ Library ▸ Open Library…** mounts a
`.snxlib` library. Type in **Filter components…** to narrow the list. Clicking
a part row opens its Component Preview tab.

To place a part, open a schematic sheet, then either:

- choose **File ▸ Library ▸ Place Component…**, select a part and press
  **Place**; or
- press **Place** at the end of a part's row in the Components panel.

The part is dropped near the centre of the view with the next free
designator, and one **Undo** removes it. With no schematic open, Signex shows
a **Place Failed** notice instead.

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

Once you have placed parts (section 5), connect the wires to their pins.

## 7. Label a net

A net label gives a wire a name. Wires with the same net label on the same
sheet are connected, even if they are not drawn touching.

A net label connects only on its own sheet. Two net labels named `SDA` on two
sheets are two separate nets. To connect across sheets, use a Port (a global
name, joined everywhere in the project) or a sheet entry wired to a pin on the
parent's sheet symbol. This is fixed; there is no setting for it. ERC warns
with "Local label name on other sheets" when the same net label name appears on
more than one sheet.

1. Choose **Place ▸ Net Label**, or press `P` then `N` in the default profile
   (`L` in the Classic profile).
2. Click where you want the label. It is placed with the default text `NET`.
3. Press `Esc` to leave the tool, then double-click the label, type the new
   name, and press `Enter`.

### Bus labels

A bus carries the signals its label names. Put a label on the bus (anywhere
along it) with one of these forms:

| Label | Signals it carries |
|---|---|
| `D[0..7]` | `D0`, `D1`, ... `D7` |
| `D[7..0]` or `D[7:0]` | the same eight signals, written high to low |
| `{SDA SCL}` | `SDA` and `SCL` (a group; items are separated by blanks) |
| `{A[0..1] CLK}` | `A0`, `A1` and `CLK` (a group may hold ranges) |

Numbers are plain unsigned integers without leading zeros (`D03` is not the same
signal as `D3`), and matching is case-sensitive (`d3` is not `D3`). There is no
space around the numbers or around
the label, a range needs a name before the `[`, and nothing may follow the `]`
(`D[0..7]A` is refused). A bare name such as `VCC`
is a net label, not a bus label.

Unlike wires, which need a junction dot to join at a T, bus segments join wherever
one ends on another (at its end or along it), dot or not. A bus never joins its
signals together. A wire that leaves the bus through a
bus entry connects to a signal only through its own net label, so label each
entry wire with one signal of the bus (for example `D3`). ERC checks this, see
section 9.

### Buses across sheets

A sheet pin, a Port (global label) or a hierarchical label can carry a bus: name
it with a bus label such as `D[0..7]` and place it on a bus. The signals join
one by one, never the bus as a whole:

- A bus sheet pin joins the hierarchical label or Port with the same bus name on
  its child sheet. `D[0..7]` and `D[0..3]` are the same bus at two widths; the
  signals both name (`D0` to `D3`) join, and ERC reports the width mismatch,
  naming the pin and the port.
- A Port bus joins every Port bus of the same name in the project, signal by
  signal. ERC reports a Port bus that appears at two widths in the project
  (`D[0..7]` and `D[0..3]`), since only the shared signals join.
- A local net label bus never leaves its sheet.

A signal joins through the net label on its bus-entry wire, so each side needs
its entry wires labelled. A group (`{SDA SCL}`) joins only a group with the same
content. Still to come: a bus joined to another bus by geometry across sheets,
and bus-aware rendering of the pin (see #1049).

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

ERC also checks buses (section 7): a wire from a bus entry with no net label,
a label that is not one of the signals the bus carries (for example `D33` on
`D[0..7]`), a bus with no valid bus label, a label on a bus that looks like a bus
name but is malformed (such as `D[0 .. 7]`, anywhere along the bus), and two names
on one bus that carry
different signals (`D[0..7]` and `D[8..15]`) are each reported as warnings. A
bus-name label placed on a wire instead of a bus is also a warning: it still
names that wire's net, which is almost always a mistake.

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

## Walkthrough: a saved schematic with a wired symbol

This is sections 3 to 10 as one run. Each step names the section with the
detail. Step 3 cannot be completed from the UI yet; see its note.

1. **Create the project.** Choose **File ▸ New Project**, name it in the
   **New Project** save dialog, and keep the `.snxprj` ending (section 3). A
   blank `.snxsch` opens in a tab.
2. **Find a part.** Open **View ▸ Components** and expand **Stock**
   (section 5). Type in **Filter components…** to find the part.
3. **Place it.** Intended path: **File ▸ Library ▸ Place Component…**, select
   the part, press **Place**. *Not working yet:* the **Place** button closes
   the picker without placing (section 5). Everything below then runs on a
   sheet without a part.
4. **Wire it.** Choose **Place ▸ Wire**, click to start, click to bend, and
   double-click to stop (section 6). End each wire on a pin end of the part.
5. **Name the net.** Choose **Place ▸ Net Label**, click the wire, press `Esc`,
   double-click the label, type the name, and press `Enter` (section 7).
6. **Annotate.** Press `Alt+A` (**Design ▸ Annotation ▸ Annotate Schematics
   Quietly**) to give every part a unique reference designator (section 8).
7. **Run ERC.** Press `F8`, read the **ERC** panel, and click a row to jump to
   the item (section 9). Fix what it reports, then run it again.
8. **Save.** Press `Ctrl+S` or choose **File ▸ Save** (section 10). Signex
   writes the sheet to its `.snxsch` file. Close the tab and reopen it with
   **File ▸ Open Recent** to check that the wires and labels came back.

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
