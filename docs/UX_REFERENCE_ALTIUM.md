# Signex — UX Reference (Altium Baseline)

> **Status:** Canonical UX specification. Authoritative for all UI implementation.
> **Audience:** Anyone building user-facing surfaces in Signex.
> **Rule:** If the code does something different from this document and there is
> no approved exception, the code is wrong.

This document describes how Signex *feels* to use. It is derived from Altium
Designer's interaction model because that is the target for professional EDA
UX parity. Signex-specific additions are clearly marked; everything else maps
to Altium behavior that professional users already have in muscle memory.

---

## 1. Guiding Principles

1. **Altium muscle memory is sacred.** An engineer who has used Altium for
   years should sit down in Signex and begin working without reading a manual.
   If we deviate from Altium behavior, we document why and we make it
   discoverable.

2. **Right-click is pan, not context menu.** This is the single most important
   interaction rule. It is opposite to most general-purpose applications and it
   is correct for EDA. Left-click selects. Right-click + drag pans. Right-click
   without drag opens the context menu.

3. **Keyboard-first.** Common operations have single-key shortcuts (`W` for
   wire, `P` for place, `L` for label, `Space` for rotate). Modifier keys
   are for variants of the base action. The keyboard shortcut should always be
   faster than the menu.

4. **Context-aware everything.** The Properties panel changes based on
   selection. The context menu changes based on what is under the cursor. The
   toolbar changes based on whether the user is in schematic or PCB mode.
   Panels show and hide based on the active editor context.

5. **No mode traps.** Escape always returns to the selection tool. The user
   should never be stuck in a mode they did not choose and cannot leave.

---

## 2. Panel System

### 2.1. Panel Behavior

- Panels dock to **left**, **right**, and **bottom** regions
- Panels within a dock region are **tabbed** — click a tab to switch
- A dock region can be **collapsed** to a vertical rail showing icons and
  rotated labels; clicking a rail item expands that dock
- Panels can be **floated** as separate windows (deferred to post-v1.0;
  v1.0 supports docked and collapsed only)
- Panel visibility is **context-aware**: schematic-only panels hide when the
  PCB editor is active, and vice versa
- **Properties panel** (`F11`) is the most important panel — it is
  context-aware based on the current selection and should be visible by default
- Double-clicking a panel tab undocks it to floating (post-v1.0)
- Panel sizes and collapsed state persist across sessions (saved to a local
  settings file)

### 2.2. Panel Registry

Each panel has a name, a default dock position, and a context filter.

| Panel          | Default Dock | Context    | Notes                                    |
|----------------|:------------:|:----------:|------------------------------------------|
| Projects       | Left         | Both       | Sheet hierarchy tree                     |
| Components     | Left         | Both       | Library browser (KiCad symbol libraries)  |
| Navigator      | Left         | Both       | Design object navigator                  |
| Libraries      | Left         | Both       | Installed library management             |
| SCH Library    | Left         | Schematic  | Schematic symbol editor library          |
| PCB Library    | Left         | PCB        | Footprint editor library                 |
| Net Classes    | Left         | Both       | Net class assignment and management      |
| Properties     | Right        | Both       | Context-aware property editor (`F11`)    |
| Filter         | Right        | Schematic  | Selection filter (per-type toggle)       |
| List           | Right        | Schematic  | Object list with sorting and filtering   |
| Inspector      | Right        | Schematic  | Multi-object property inspector          |
| Snippets       | Right        | Schematic  | Reusable schematic snippets              |
| Variants       | Right        | Schematic  | Design variant management                |
| Net Inspector  | Right        | PCB        | Per-net length, via count, status        |
| Layer Stack    | Right        | PCB        | Layer visibility, color, active layer    |
| Messages       | Bottom       | Both       | ERC violations, compiler output          |
| Signal (AI)    | Bottom       | Both       | Signal AI chat (Pro only)                |
| Output Jobs    | Bottom       | Schematic  | Export job queue and status              |
| DRC            | Bottom       | PCB        | Design rule check violations             |
| Cross Section  | Bottom       | PCB        | Board cross-section visualization        |
| Waveform       | Bottom       | Simulation | SPICE waveform viewer                    |
| S-Parameters   | Bottom       | Simulation | S-parameter and Smith chart viewer       |
| Thermal        | Bottom       | Simulation | Thermal analysis result viewer           |

**Implementation note:** The panel system is custom-built on `iced::widget::pane_grid`
(for region splitting) combined with tab headers (for panel switching within a
region). Neither iced nor iced_aw provides a ready-made tabbed docking system.
See `ARCHITECTURE.md` for crate ownership.

### 2.3. Default Layout

On first launch (no saved layout):

- **Left dock:** Projects (active tab), Components
- **Right dock:** Properties (active tab)
- **Bottom dock:** Messages (active tab), Signal (Pro only)
- **Center:** Empty canvas with welcome screen or last-opened project

---

## 3. Mouse Behavior

This table is the single most important reference for canvas interaction.

| Action                    | Input                              | Notes                                             |
|---------------------------|------------------------------------|----------------------------------------------------|
| Pan                       | Right-click + drag                 | Primary pan. NOT left-click drag.                  |
| Pan (alt)                 | Middle-click + drag                | Alternative for mice without right-click comfort   |
| Zoom                      | Scroll wheel                       | Centered on cursor position                        |
| Select                    | Left-click                         | Single object under cursor                         |
| Toggle select             | Shift + left-click                 | Add or remove object from selection                |
| Box select (enclosing)    | Left-drag left→right on empty area | Only fully enclosed objects are selected            |
| Box select (crossing)     | Left-drag right→left on empty area | Any overlapping objects are selected               |
| Context menu              | Right-click (no drag)              | Context-sensitive; appears on mouse-up if no drag  |
| Open properties           | Double-click                       | Opens Properties panel for the object              |
| In-place edit             | F2 or click-pause-click            | Inline text editing on canvas                      |
| Move (rubber-band)        | Left-drag on selected object       | Connected wires stretch to maintain connections     |
| Move (stiff)              | Ctrl + left-drag on selected       | Move without rubber-banding                        |
| Cross-probe               | Ctrl + double-click                | Jump between schematic and PCB                     |
| Net highlight             | Alt + click                        | Highlight entire net across all sheets             |
| Wire endpoint drag        | Left-drag on wire endpoint         | Reposition wire endpoint                           |
| Wire segment drag         | Left-drag on wire midpoint         | Reposition wire segment                            |

### 3.1. Important Interaction Details

**Right-click pan vs. context menu disambiguation:**

- Right mouse down: begin tracking
- Right mouse move (>4px threshold): this is a pan gesture; consume all
  further right-mouse events until release
- Right mouse up without move: this is a context menu request; show context
  menu at cursor position

**Box select direction matters:**

- Left-to-right drag creates an *enclosing* selection box (green/solid) —
  only objects fully inside the box are selected
- Right-to-left drag creates a *crossing* selection box (blue/dashed) —
  objects that intersect the box boundary are also selected

This is Altium's (and AutoCAD's) convention. It must be visually distinct:
different border color and different border style.

**Zoom centering:**

Scroll-wheel zoom centers on the cursor's world-space position, not on the
viewport center. This means the point under the cursor stays under the cursor
during zoom. This is the single most important detail for zoom to feel natural.

---

## 4. Keyboard Shortcuts

### 4.1. General Editing

| Key                | Action                       | Context    |
|--------------------|------------------------------|------------|
| Ctrl+C             | Copy                         | Both       |
| Ctrl+X             | Cut                          | Both       |
| Ctrl+V             | Paste                        | Both       |
| Shift+Ctrl+V       | Smart paste                  | Both       |
| Ctrl+D             | Duplicate                    | Both       |
| Delete             | Delete selection             | Both       |
| Backspace          | Remove last wire point / delete | Both    |
| Ctrl+Z             | Undo                         | Both       |
| Ctrl+Y             | Redo                         | Both       |
| Ctrl+Shift+Z       | Redo (alternative)           | Both       |
| F2                 | In-place text edit           | Both       |
| Ctrl+F             | Find                         | Both       |
| Ctrl+H             | Find and replace             | Both       |
| Ctrl+Q             | Toggle units (mm/mil/inch)   | Both       |
| Ctrl+A             | Select all                   | Both       |
| Ctrl+M             | Measure distance             | Both       |
| Escape             | Cancel action / deselect all | Both       |

### 4.2. Placement and Drawing

| Key                | Action                              | Context    |
|--------------------|-------------------------------------|------------|
| W                  | Draw wire                           | Schematic  |
| B                  | Draw bus                            | Schematic  |
| T                  | Place text                          | Schematic  |
| L                  | Place net label                     | Schematic  |
| P                  | Place component (opens search)      | Schematic  |
| Tab                | Pause placement, open Properties    | Both       |
| Enter              | Confirm placement                   | Both       |

### 4.3. Transformation

| Key                | Action                              | Context    |
|--------------------|-------------------------------------|------------|
| Space              | Rotate 90° counter-clockwise        | Both       |
| Shift+Space        | Cycle wire routing mode             | Schematic  |
| R                  | Rotate selected                     | Both       |
| X                  | Mirror X (horizontal flip)          | Both       |
| Y                  | Mirror Y (vertical flip)            | Both       |

### 4.4. View

| Key                | Action                              | Context    |
|--------------------|-------------------------------------|------------|
| Home               | Fit all / center view               | Both       |
| G                  | Cycle grid size forward             | Both       |
| Shift+G            | Cycle grid size backward            | Both       |
| Shift+Ctrl+G       | Toggle grid visibility              | Both       |
| F5                 | Toggle net color override           | Both       |
| F11                | Toggle Properties panel             | Both       |
| Ctrl+Shift+A       | Open Signal AI panel (Pro)          | Both       |
| Shift+F            | Find Similar Objects                | Both       |

### 4.5. Nudge

| Key                | Action                              | Context    |
|--------------------|-------------------------------------|------------|
| Ctrl+Arrow         | Nudge selection by 1 grid unit      | Both       |
| Shift+Ctrl+Arrow   | Nudge selection by 10 grid units    | Both       |

### 4.6. Selection Memory

| Key                | Action                              | Context    |
|--------------------|-------------------------------------|------------|
| Ctrl+1 through Ctrl+8 | Store current selection to slot  | Both       |
| Alt+1 through Alt+8   | Recall selection from slot       | Both       |

### 4.7. PCB-Specific (v2.0+)

| Key                | Action                              | Context    |
|--------------------|-------------------------------------|------------|
| Shift+S            | Cycle single-layer mode             | PCB        |
| + or =             | Toggle active layer (F.Cu ↔ B.Cu)  | PCB        |
| F                  | Flip component (Top ↔ Bottom)      | PCB        |

### 4.8. Shortcut Customization

Keyboard shortcuts are configurable (planned for post-v2.0). Until then, the
Altium defaults above are hardcoded. When customization ships:

- A shortcut editor dialog lists all actions with current bindings
- Conflicts are detected and shown at assignment time
- A "Reset to Altium defaults" button is always available
- Custom shortcuts persist in the settings file

---

## 5. Single Layer Mode (PCB — Shift+S)

Single layer mode cycles through four display states:

| Mode       | Active Layer          | Inactive Layers             |
|------------|-----------------------|-----------------------------|
| Off        | Full color            | Full color                  |
| Hide       | Full color            | Completely hidden           |
| Grayscale  | Full color            | Desaturated to gray         |
| Monochrome | Full color            | Dim single color            |

Each press of `Shift+S` advances to the next mode in the cycle.

**Layer pair toggling:** The `+` or `=` key toggles the active layer between
`F.Cu` and `B.Cu`. Flipping a component with `F` moves it between paired
layers. Paired technical layers (Top Overlay ↔ Bottom Overlay, Top Solder ↔
Bottom Solder) follow automatically.

---

## 6. Status Bar

The status bar runs along the bottom edge of the window, below the bottom
dock panel. It is always visible.

### 6.1. Layout (left to right)

| Position | Content                  | Interaction                             |
|:--------:|--------------------------|------------------------------------------|
| 1        | Cursor: `X:12.54 Y:8.20` | Updates live as mouse moves              |
| 2        | Grid: `2.54mm`           | Click to toggle visibility; `G` to cycle |
| 3        | Snap: `Snap` or `Free`   | Click to toggle snap-to-grid             |
| 4        | E-Snap: `E-Snap`         | Always on (electrical snap)              |
| 5        | Layer: `Top Layer`       | PCB only; click to open layer picker     |
| 6        | Mode: `Select`           | Shows active tool name                   |
|          | *(spacer)*               |                                          |
| 7        | Zoom: `100%`             | Display only                             |
| 8        | Units: `mm`              | Click to cycle; `Ctrl+Q` to cycle        |
| 9        | Panels button            | Dropdown to toggle panel visibility      |

### 6.2. Grid Size Cycle

The `G` key cycles grid size forward through this sequence:

`0.635mm → 1.27mm → 2.54mm → 5.08mm → 10.16mm → (wrap)`

`Shift+G` cycles backward. These values match KiCad's standard grid sequence
and are based on the 100 mil (2.54mm) base grid that PCB design universally
uses.

### 6.3. Coordinate Display

Coordinates are displayed in the current unit system (mm, mil, or inch). The
display updates in real time as the mouse moves. When snap is enabled, the
displayed coordinates are the snapped position, not the raw cursor position.

---

## 7. Layer Colors

### 7.1. PCB Layer Colors (Altium Defaults)

These are the default layer colors used by Signex's PCB editor. They match
Altium Designer's defaults for familiarity. Users may customize per-project
(post-v2.0).

#### Copper Layers (32 maximum)

| Layer                | KiCad Name   | Default Color | Hex       |
|----------------------|-------------|---------------|-----------|
| Top Layer            | F.Cu        | Red           | `#FF0000` |
| Mid Layer 1          | In1.Cu      | Yellow        | `#FFFF00` |
| Mid Layer 2          | In2.Cu      | Green         | `#00FF00` |
| Mid Layer 3          | In3.Cu      | Cyan          | `#00FFFF` |
| Mid Layer 4          | In4.Cu      | Magenta       | `#FF00FF` |
| Mid Layer 5          | In5.Cu      | Olive         | `#808000` |
| Mid Layer 6          | In6.Cu      | Teal          | `#008080` |
| Mid Layer 7          | In7.Cu      | Purple        | `#800080` |
| Mid Layer 8          | In8.Cu      | Orange        | `#FF8000` |
| Mid Layer 9          | In9.Cu      | Azure         | `#0080FF` |
| Mid Layer 10         | In10.Cu     | Chartreuse    | `#80FF00` |
| Mid Layer 11         | In11.Cu     | Rose          | `#FF0080` |
| Mid Layer 12         | In12.Cu     | Spring        | `#00FF80` |
| Mid Layer 13         | In13.Cu     | Violet        | `#8000FF` |
| Mid Layer 14         | In14.Cu     | Salmon        | `#FF8080` |
| Mid Layer 15         | In15.Cu     | Light Green   | `#80FF80` |
| Mid Layer 16         | In16.Cu     | Light Blue    | `#8080FF` |
| Mid Layers 17–30     |             | Cycling from palette | — |
| Bottom Layer         | B.Cu        | Blue          | `#0000FF` |

#### Technical Layers

| Layer                | KiCad Name   | Default Color     | Hex       | Alpha |
|----------------------|-------------|-------------------|-----------|:-----:|
| Top Overlay          | F.SilkS     | Yellow            | `#FFFF00` | 100%  |
| Bottom Overlay       | B.SilkS     | Dark Blue-Gray    | `#404080` | 100%  |
| Top Solder           | F.Mask       | Purple            | `#800080` | 40%   |
| Bottom Solder        | B.Mask       | Teal              | `#008080` | 40%   |
| Top Paste            | F.Paste      | Gray              | `#808080` | 90%   |
| Bottom Paste         | B.Paste      | Dark Teal         | `#004040` | 90%   |
| Top Assembly         | F.Fab        | Light Gray        | `#AFAFAF` | 100%  |
| Bottom Assembly      | B.Fab        | Slate             | `#585D84` | 100%  |
| Top Courtyard        | F.CrtYd      | Pink              | `#FF26E2` | 100%  |
| Bottom Courtyard     | B.CrtYd      | Cyan              | `#26E9FF` | 100%  |
| Keep-Out             | Edge.Cuts    | Magenta           | `#FF00FF` | 100%  |
| Board Outline        | Margin       | Yellow            | `#FFFF00` | 100%  |

#### Mechanical Layers

| Layer                | KiCad Name   | Default Color     | Hex       |
|----------------------|-------------|-------------------|-----------|
| Mechanical 1         | Dwgs.User   | Orange            | `#FF8000` |
| Mechanical 2         | Cmts.User   | Steel Blue        | `#5994DC` |
| Mechanical 3         | Eco1.User   | Mint              | `#B4DBD2` |
| Mechanical 4         | Eco2.User   | Gold              | `#D8C852` |

#### Virtual / System Layers

| Layer                | Default Color     | Hex       | Alpha |
|----------------------|-------------------|-----------|:-----:|
| Multi-Layer          | Silver            | `#C0C0C0` | 100%  |
| Via Holes            | Gold              | `#E3B72E` | 100%  |
| Via Holewalls        | Near White        | `#ECECEC` | 100%  |
| Plated Holes         | Dark Yellow       | `#C2C200` | 100%  |
| Non-Plated Holes     | Cyan              | `#1AC4D2` | 100%  |
| Ratsnest             | Bright Cyan       | `#00F8FF` | 35%   |
| DRC Error            | Green             | `#00FF00` | 80%   |
| DRC Warning          | Amber             | `#FFD042` | 80%   |
| Selection Overlay    | White             | `#FFFFFF` | —     |
| Grid                 | Dark Gray         | `#404040` | —     |
| Cursor               | White             | `#FFFFFF` | —     |
| PCB Background       | Black             | `#000000` | —     |

---

## 8. Schematic Canvas Themes

Signex ships with six built-in color themes for the schematic canvas. All
themes are available in both Community and Pro editions.

### 8.1. Theme Color Table

| Element        | Catppuccin Mocha | VS Code Dark | Altium Dark | GitHub Dark | Solarized Light | Nord    |
|----------------|:----------------:|:------------:|:-----------:|:-----------:|:---------------:|:-------:|
| Background     | `#1A1B2E`        | `#1E1E1E`    | `#1A1A1A`   | `#0D1117`   | `#EEE8D5`       | `#2E3440` |
| Paper          | `#1E2035`        | `#252526`    | `#FFFFFF`   | `#161B22`   | `#FDF6E3`       | `#3B4252` |
| Wire           | `#4FC3F7`        | `#4EC994`    | `#0000FF`   | `#58A6FF`   | `#268BD2`       | `#88C0D0` |
| Junction       | `#4FC3F7`        | `#4EC994`    | `#0000FF`   | `#58A6FF`   | `#268BD2`       | `#88C0D0` |
| Body           | `#9FA8DA`        | `#DCDCAA`    | `#000000`   | `#C9D1D9`   | `#657B83`       | `#81A1C1` |
| Body Fill      | `#1E2035`        | `#252526`    | `#FFFFC0`   | `#161B22`   | `#FDF6E3`       | `#3B4252` |
| Pin            | `#81C784`        | `#569CD6`    | `#880000`   | `#3FB950`   | `#859900`       | `#A3BE8C` |
| Reference      | `#E8C66A`        | `#FFE0A0`    | `#0000AA`   | `#D29922`   | `#B58900`       | `#EBCB8B` |
| Value          | `#9598B3`        | `#9D9D9D`    | `#444444`   | `#8B949E`   | `#657B83`       | `#D8DEE9` |
| Net Label      | `#81C784`        | `#4EC994`    | `#880000`   | `#3FB950`   | `#859900`       | `#A3BE8C` |
| Global Label   | `#FF8A65`        | `#CE9178`    | `#CC6600`   | `#FFA657`   | `#CB4B16`       | `#D08770` |
| Hier Label     | `#BA68C8`        | `#C586C0`    | `#660066`   | `#BC8CFF`   | `#6C71C4`       | `#B48EAD` |
| No Connect     | `#E8667A`        | `#F48771`    | `#CC0000`   | `#F85149`   | `#DC322F`       | `#BF616A` |
| Power          | `#EF5350`        | `#D16969`    | `#FF0000`   | `#FF7B72`   | `#D33682`       | `#D08770` |
| Selection      | `#00BCD4`        | `#007ACC`    | `#00AAFF`   | `#388BFD`   | `#2AA198`       | `#88C0D0` |
| Bus            | `#4A86C8`        | `#307ABC`    | `#000088`   | `#2F6AF5`   | `#1A7AA3`       | `#5E81AC` |

### 8.2. Theme Design Rules

- Every rendered element reads its color from the active theme's token set
- Theme switching re-renders the entire canvas immediately (invalidate all
  render caches)
- The default theme is **Catppuccin Mocha** (dark, high contrast, easy on eyes)
- The user selects a theme from View → Theme or from Preferences
- Theme choice persists across sessions
- Custom themes are a post-v2.0 feature; v1.0 ships only the six built-in
  themes

### 8.3. Theme Token Structure

All six themes are defined as a `CanvasColors` struct with named tokens. The
render layer always resolves colors through these tokens, never through
hardcoded hex values.

```rust
pub struct CanvasColors {
    pub background: Color,
    pub paper: Color,
    pub wire: Color,
    pub junction: Color,
    pub body: Color,
    pub body_fill: Color,
    pub pin: Color,
    pub reference: Color,
    pub value: Color,
    pub net_label: Color,
    pub global_label: Color,
    pub hier_label: Color,
    pub no_connect: Color,
    pub power: Color,
    pub selection: Color,
    pub bus: Color,
}
```

---

## 9. Context Menu

Right-click without drag on the canvas opens a context-sensitive menu. The
menu contents depend on what is under the cursor and the current selection.

### 9.1. Context Menu Structure

**Nothing selected, nothing under cursor:**

- Paste (`Ctrl+V`)
- Select All (`Ctrl+A`)
- Zoom to Fit (`Home`)
- Grid → (cycle sizes)

**Single object under cursor (not selected):**

- Select
- Properties (`Double-click`)
- Cut (`Ctrl+X`)
- Copy (`Ctrl+C`)
- Delete (`Delete`)
- Rotate 90° (`Space`)
- Mirror X (`X`)
- Mirror Y (`Y`)

**Selection exists:**

- Cut (`Ctrl+X`)
- Copy (`Ctrl+C`)
- Paste (`Ctrl+V`)
- Duplicate (`Ctrl+D`)
- Delete (`Delete`)
- Rotate 90° (`Space`)
- Mirror X (`X`)
- Mirror Y (`Y`)
- Align → (left, right, top, bottom, center H, center V, distribute H, distribute V)
- Properties (`F11`)

**Object-type-specific entries appear contextually:** for example, "Edit Symbol"
on a symbol instance, "Change Net Name" on a label, "Navigate Into" on a sheet
symbol.

---

## 10. Wire Routing Modes (Schematic)

When drawing a wire (`W` key), the user cycles between three routing modes
using `Shift+Space`:

| Mode       | Behavior                                         |
|------------|--------------------------------------------------|
| Manhattan  | 90° segments only, horizontal and vertical       |
| Diagonal   | 45° segments allowed                             |
| Free       | Arbitrary angle segments                         |

The active routing mode is indicated in the status bar. The default on first
activation is Manhattan.

**Auto-junction:** when a wire endpoint lands on an existing wire segment at a
T-intersection, a junction is automatically placed. Junctions are rendered as
small filled circles at the intersection point.

---

## 11. Selection System Details

### 11.1. Selection Filter

The Selection Filter panel (right dock, Schematic context) provides per-type
toggle switches:

- Wires
- Symbols
- Labels (net, global, hierarchical, power)
- Junctions
- Bus entries
- Text annotations
- Drawing objects (lines, rectangles, circles, arcs)
- Sheet symbols

When a type is disabled in the filter, objects of that type cannot be selected
by any means (click, box select, Ctrl+A). They are effectively invisible to
selection, though still rendered.

### 11.2. Selection Memory

The user can store up to eight named selection sets:

- `Ctrl+1` through `Ctrl+8` stores the current selection
- `Alt+1` through `Alt+8` recalls the stored selection (replacing current)

Selection memory is session-only and not persisted.

### 11.3. Find Similar Objects

`Shift+F` opens a dialog to find objects matching the currently selected
object's properties (type, value, footprint, etc.). Results are selected on
the canvas and optionally filtered to a specific scope (current sheet, all
sheets).

---

## 12. Properties Panel Behavior

The Properties panel is the most-used panel. Its content changes dynamically
based on the current selection.

| Selection State            | Properties Content                             |
|----------------------------|------------------------------------------------|
| Nothing selected           | Document properties (grid, page, template)     |
| Single symbol              | Reference, value, footprint, fields, unit      |
| Single wire                | Net name (read-only, derived)                  |
| Single label               | Label text, shape, rotation, font              |
| Single junction            | Position (read-only)                           |
| Single text annotation     | Text content, font, alignment, rotation        |
| Single sheet symbol        | Sheet name, file path, size                    |
| Multiple objects (same type)   | Common editable properties, batch-edit mode|
| Multiple objects (mixed types) | Intersection of common properties          |

**Editing behavior:**

- Typing in a property field applies immediately on focus-out or Enter
- Changes go through the engine as `UpdateProperty` commands (undoable)
- The panel updates reactively when the semantic model changes (another
  tool moves the selected object, undo restores a previous state, etc.)

---

## 13. Tool System Interaction Model

The editor uses an explicit tool model. At any time, exactly one tool is
active. The active tool determines how mouse and keyboard input is interpreted
on the canvas.

### 13.1. Tool List

| Tool                  | Activation           | Cursor Icon  |
|-----------------------|----------------------|--------------|
| Selection             | Escape / click empty | Arrow        |
| Wire Drawing          | W                    | Crosshair    |
| Bus Drawing           | B                    | Crosshair    |
| Label Placement       | L                    | Crosshair    |
| Symbol Placement      | P                    | Crosshair    |
| Text Placement        | T                    | Text cursor  |
| Measure               | Ctrl+M               | Ruler        |
| Pan (temporary)       | Right-click hold     | Grab hand    |

### 13.2. Tool Lifecycle

Every tool follows the same interaction lifecycle:

1. **Activate** — user presses shortcut or clicks toolbar
2. **Preview** — tool shows preview geometry as mouse moves (e.g., wire
   rubber-band, symbol ghost at cursor)
3. **Commit points** — user clicks to commit intermediate state (e.g., wire
   vertex)
4. **Finish** — user completes the action (e.g., click endpoint, press Enter)
   or cancels (Escape, right-click)
5. **Produce commands** — tool emits one or more `Command` values to the engine
6. **Return** — tool either remains active for next placement (symbol, label)
   or returns to Selection tool (depends on tool type)

**Tab during placement:** pressing Tab while a placement tool is active pauses
the placement and opens the Properties panel for the object being placed. The
user edits properties, then presses Enter or Tab to resume placement with the
updated properties.

### 13.3. Tool Contracts

Every tool must handle:

- `pointer_down(position, button, modifiers)`
- `pointer_move(position, modifiers)`
- `pointer_up(position, button, modifiers)`
- `key_pressed(key, modifiers)`
- `cancel()` — Escape or right-click without drag
- `commit()` — Enter or final click

Every tool may produce:

- Zero or more `Command` values (sent to the engine)
- UI state transitions (selection change, mode change)
- Preview geometry (rendered on the overlay layer, cleared each frame)
- Snap requests (query the grid and nearby connection points)

Tools do not mutate the document, the semantic model, or the render cache.
They produce `Command` values. This is the hard rule from `ARCHITECTURE.md`
applied at the tool level.

---

## 14. Signex-Specific UX Additions

These features are not in Altium. They are clearly marked in the UI so users
know they are Signex-specific.

### 14.1. AutoFocus

When hovering over or selecting an object, unrelated objects are dimmed on the
canvas. This provides immediate visual context about what is electrically
connected to the selection. The dim level is configurable (default: 40% opacity
for unrelated objects).

### 14.2. Net Color Override (F5)

Pressing F5 enables a mode where each net is assigned a distinct color from a
palette. All wires, labels, and junctions belonging to the same net are drawn
in the same color. Pressing F5 again disables the override and returns to
theme colors.

### 14.3. Signal AI Panel (Pro)

A chat panel in the bottom dock that provides AI-assisted design feedback. See
`PRODUCT_AND_EDITIONS.md` for capabilities. The panel follows standard bottom-
dock panel behavior (tabbed, collapsible). In Community builds, this panel does
not exist.

---

## 15. Accessibility Notes

- All interactive elements are keyboard-reachable (no mouse-only operations)
- Color is never the only way to convey information (shape, label, or pattern
  accompanies every color-coded indicator)
- Themes include one light option (Solarized Light) for users who prefer or
  require light backgrounds
- The high-contrast theme variant is planned for post-v2.0
- Text sizes in panels use the system font size setting where possible

---

## 16. Rules for Changing This Document

This document changes as the UX evolves, but changes follow rules:

- **Altium-baseline behaviors** (Section 3, 4, 5, 6) change only if Altium
  itself changes or if user research demonstrates the Altium behavior is
  actively harmful. "We think X would be better" is not sufficient.
- **Signex-specific additions** (Section 14) can be added freely as long as
  they do not conflict with baseline behaviors.
- **Keyboard shortcuts** change only through the future customization system.
  Default bindings are locked to Altium's defaults.
- **Theme colors** can be refined for accessibility or aesthetics but the
  theme token structure (the set of named tokens) changes only through an
  architectural proposal.
