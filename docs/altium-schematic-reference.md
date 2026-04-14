# Altium Designer Schematic Editor — Feature Reference

## Keyboard Shortcuts

### General Editing
| Shortcut | Action |
|----------|--------|
| Ctrl+C/X/V | Copy/Cut/Paste |
| Shift+Ctrl+V | Smart Paste (transform objects, create arrays) |
| Ctrl+R | Rubber stamp (repeated paste) |
| Delete | Delete selection |
| Ctrl+Z/Y | Undo/Redo |
| F2 | Edit selected text in-place |
| Ctrl+F/H | Find / Find and Replace |
| Ctrl+M | Measure distance |
| Ctrl+Q | Toggle mm/mil units |

### Selection
| Shortcut | Action |
|----------|--------|
| Click | Select/deselect |
| Shift+Click | Toggle in/out of selection |
| Ctrl+A | Select all |
| Alt+Click | Select and highlight entire net across all sheets |
| Shift+F | Find Similar Objects dialog |
| Shift+Ctrl+X | Toggle Cross Select Mode (sync with PCB) |

### Movement & Transformation
| Shortcut | Action |
|----------|--------|
| Arrow Keys | Move cursor by 1 grid unit |
| Shift+Arrow | Move cursor by 10 grid units |
| Ctrl+Arrow | Nudge selection by 1 grid unit |
| Shift+Ctrl+Arrow | Nudge selection by 10 grid units |
| Spacebar | Rotate 90 CCW (during placement/drag) |
| Shift+Spacebar | Rotate 90 CW |
| X | Mirror X-axis |
| Y | Mirror Y-axis |

### View & Zoom
| Shortcut | Action |
|----------|--------|
| Ctrl+PgDn | Zoom to fit all |
| PgUp/PgDn | Zoom in/out |
| Mouse Wheel | Scroll vertically |
| Shift+Wheel | Scroll horizontally |
| Home | Center cursor |
| Right-Click+Drag | Pan view |

### Grid & Display
| Shortcut | Action |
|----------|--------|
| G | Cycle forward through snap grid presets |
| Shift+G | Cycle backward through snap grid presets |
| Shift+Ctrl+G | Toggle visible grid |
| Ctrl+Shift (hold) | Temporarily disable grid snap |
| Shift+E | Toggle electrical grid snap |
| F5 | Toggle Net Color Override |

### Panels
| Shortcut | Action |
|----------|--------|
| F11 | Toggle Properties panel |
| F12 | Toggle Filter panel |

### During Placement
| Shortcut | Action |
|----------|--------|
| Tab | Pause placement, open Properties panel |
| Enter | Confirm placement (same as click) |
| Esc / Right-click | Exit placement mode |
| Backspace | Remove last placed vertex |
| Shift+Spacebar | Change wire routing mode |

## Component Placement Workflow

### Search & Discovery
- Components Panel via P,P shortcut
- Category browsing via hierarchical tree
- Text search with wildcards (*)
- Parametric filtering with faceted filters
- Side-by-side component comparison
- Part Choices with supplier data (stock, price, lifecycle)

### Placement Methods
1. Place button in details pane
2. Right-click > Place (component floats on cursor)
3. Click-hold-drag (single instance)
4. Double-click (multi-instance mode, right-click exits)

### During Placement
- Tab: Open Properties to edit before placing
- Spacebar: Rotate 90 CCW
- X/Y: Mirror
- Auto-increment on designator suffixes
- Default designators use ? suffix (R?, C?)

## Wire Routing

### Placement Modes (cycle with Shift+Spacebar)
| Mode | Behavior |
|------|----------|
| 90 Degree | Orthogonal, Start/End sub-modes |
| 45 Degree | 45-degree angles |
| Any Angle | Unrestricted |
| Auto Wire | Point-to-point with obstacle avoidance |

### Smart Behaviors
- **Auto-Junction**: Compiler places junctions at T-intersections
- **Electrical Grid**: Cursor snaps to nearby connection points
- **Connection Markers**: Red cross at valid hotspots, blue at snap points
- **Rubber-banding**: Wires stretch when dragging connected components
- **Break at Autojunctions**: Optional auto-splitting at new junctions

### Wire Editing
- Drag endpoint: Reposition wire ends
- Drag vertex: Move intermediate point
- Drag segment: Reposition segment
- Delete vertex while moving

## Net Labels & Power Ports

### Net Labels
- Hotspot at lower-left must touch wire
- Auto-increment numeric endings
- In-place editing: click-pause-click
- Overline/negation: backslash syntax
- Case-insensitive matching

### Power Ports
- Global scope by default (connect across entire design)
- Styles: Arrow, Bar, Circle, Earth, Ground, Wave, etc.
- Style is VISUAL only; net name determines connectivity

### Bus System
- Naming: Data[0..7] creates Data0-Data7
- Bus Entry for signal extraction
- Width matching enforcement

## Multi-Sheet Design

### Flat Design
- All sheets same level
- Same-named nets connect across sheets

### Hierarchical Design
- Parent sheet has Sheet Symbols for children
- Signal flow: Child Port > Sheet Entry > Parent wiring
- Net scope modes: Automatic, Global, Flat, Hierarchical, Strict

### Navigation
- Ctrl+Double-Click: Jump between entries and ports
- Alt+Double-Click: Connectivity tree preview
- Ctrl+Alt hover: Selectable tree for quick sheet nav

## ERC (Electrical Rules Check)

### Violation Categories
- Bus violations (range, syntax, width mismatches)
- Component violations (duplicate designators, unused parts, missing models)
- Document structure (missing sheets, circular deps)
- Connection matrix (pin-to-pin electrical validity)

### Error Handling
- 4 severity levels: Fatal Error, Error, Warning, No Report
- No ERC directives to suppress individual violations
- Double-click violation zooms to source with dimming

## Annotation

### Methods
1. Schematic Level (logical order)
2. PCB Level (physical board position)
3. Board Level (maps schematic to PCB)

### Options
- Processing order: Up/Down/Across combinations
- Multi-part matching per sheet or whole project
- Lock/unlock individual designators
- Auto-increment configurable

## Properties Panel
- F11 toggle
- Context-aware based on selection
- Batch editing for multiple selected objects
- When nothing selected: shows document properties (grid, page, template)

## Smart Behaviors Summary

| Behavior | Description |
|----------|-------------|
| Auto-Junction | Junctions at T-intersections and pin connections |
| Electrical Grid | Cursor snaps to nearby pins/connections |
| Auto-Increment | Designators/labels increment during repeated placement |
| Auto Wire | Obstacle-avoidance routing |
| Cross-Probing | Sync selection between schematic and PCB |
| In-Place Editing | Click-pause-click to edit text directly |
| Re-entrant Commands | Start new command mid-operation |
| Smart Paste | Transform types, create arrays |
| Rubber-banding | Wires follow dragged components |
| Net Color Override | F5 to color-code nets |
| AutoFocus | Dim unrelated wiring on selection |

## All Schematic Object Types

### Electrical
Wire, Bus, Bus Entry, Net Label, Power Port, Port, Off-Sheet Connector, Junction, No Connect, Signal Harness, Harness Connector/Entry

### Hierarchical
Sheet Symbol, Sheet Entry

### Component
Part/Component, Pin

### Directives
No ERC, Differential Pair, Parameter Set, Blanket, Compile Mask

### Drawing
Text String, Text Frame, Note, Line, Arc, Bezier, Ellipse, Polygon, Polyline, Rectangle, Round Rectangle, Image
