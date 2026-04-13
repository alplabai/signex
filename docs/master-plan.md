# Signex — Master Plan

## Architecture

**Stack:** wgpu + Iced 0.14 + pure Rust
**Editions:** Signex Community (free, GPL-3.0) + Signex Pro (subscription, proprietary)

### Why wgpu + Iced

- **Elm architecture** — clean unidirectional data flow (Message → update → view), no ECS boilerplate
- **Retained-mode UI** — native-feeling widgets, proper layout engine
- **Raw wgpu access** — `iced::widget::Canvas` for schematic, `widget::Shader` for PCB GPU rendering
- **~15MB binary** — lean dependency tree
- **30-90s clean build** — fast iteration
- **Built for desktop** — file dialogs, clipboard, multi-window support
- **iced_aw 0.13** — additional widgets (Tabs, MenuBar, NumberInput, etc.)

### Known Risks and Mitigations

**Risk 1: No docking/tab system out of the box.**
Iced's `PaneGrid` is a tiling window manager (like i3/tmux) — it splits space into panes but has NO tabs, NO floating panels, NO dock targets. Building an Altium-style docking system requires combining `PaneGrid` + `iced_aw::Tabs` + custom floating window logic. This is the single largest infrastructure gap.

**Mitigation:** Phase 0 dedicates significant effort to building a custom `DockArea` widget that wraps `PaneGrid` with tab headers. This is non-trivial but bounded — the Signex panel system has fixed dock zones (left/right/bottom), not arbitrary floating docking.

**Risk 2: No tree view widget.**
Essential for Projects panel (sheet hierarchy) and component browser (library categories). Neither Iced nor `iced_aw` provides one.

**Mitigation:** Build a custom `TreeView` widget using `Column` + `Row` + indentation + expand/collapse state. This is straightforward but needs to be done.

**Risk 3: Canvas performance ceiling at 100K+ elements.**
`iced::widget::Canvas` tessellates paths via `lyon` on the CPU. For PCB-scale rendering (100K+ tracks/pads), CPU tessellation is too slow.

**Mitigation:** Use `widget::Shader` (custom WGSL) for PCB rendering. `widget::Shader` gives direct `wgpu::Device`, `wgpu::Queue`, `wgpu::RenderPass` access. Write instanced rendering pipelines (track.wgsl, pad.wgsl) similar to KiCad's GPU renderer. Canvas is fine for schematic-scale (hundreds to low thousands of elements).

**Risk 4: No EDA precedent.**
Zero production EDA tools use Iced. The only attempt — **Circe** (github.com/Iraeis/circe, 41 stars) — was abandoned and rewritten as **Scirke** using Bevy + bevy_egui. The author found Iced's canvas insufficient for serious EDA work.

**Mitigation:** Signex uses `widget::Shader` for GPU rendering (Circe only used Canvas), builds custom infrastructure where Iced lacks it, and the Elm architecture is well-suited for UI panels even if the canvas needs custom work. The key bet is that Iced's UI chrome is better than egui's for a professional desktop app, and `widget::Shader` closes the rendering gap.

**Risk 5: Iced 0.14 API may change.**
Iced master is already on 0.15-dev with Rust 2024 edition and wgpu 28.

**Mitigation:** Pin to Iced 0.14 for initial development. Upgrade to 0.15 after it stabilizes. The Elm architecture core is stable across versions.

### The Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Iced Application (native window, wgpu backend)             │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ Menu Bar (Iced Row + Buttons, custom dropdown)         │ │
│  ├────────────────────────────────────────────────────────┤ │
│  │ Toolbar Strip (Iced Row + icon buttons)                │ │
│  ├──────────┬──────────────────────┬──────────────────────┤ │
│  │ Iced     │                      │ Iced                 │ │
│  │ Left     │   wgpu Canvas        │ Right                │ │
│  │ Panels   │   (custom rendering) │ Panels               │ │
│  │          │                      │                      │ │
│  │ Projects │   Schematic / PCB    │ Properties           │ │
│  │ Comps    │   rendered via       │ Inspector            │ │
│  │ Nav      │   iced::Canvas       │ Filter               │ │
│  │ Filter   │   with raw wgpu     │ Layers               │ │
│  │          │   draw calls         │                      │ │
│  ├──────────┴──────────────────────┴──────────────────────┤ │
│  │ Bottom Panels (Messages, Signal AI, DRC, Waveform)     │ │
│  ├────────────────────────────────────────────────────────┤ │
│  │ Status Bar (X/Y, Grid, Snap, Layer, Zoom, Units)       │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**Key insight:** Iced provides two custom rendering widgets:

1. **`iced::widget::Canvas`** — 2D drawing via `Frame` API (paths, strokes, fills, text). Uses `lyon` for CPU tessellation, with `canvas::Cache` to avoid re-tessellation. Ideal for schematic rendering (hundreds to low thousands of elements). Supports pan/zoom via manual transforms in `Program::update`.

2. **`iced::widget::Shader`** — raw wgpu access. You implement `shader::Program` + `shader::Primitive` traits and get direct access to `wgpu::Device`, `wgpu::Queue`, `wgpu::RenderPass`. Write custom WGSL shaders with instanced rendering. Required for PCB (100K+ elements) and 3D viewer.

This gives us:
- **Iced** handles all UI chrome (panels, toolbars, dialogs, menus)
- **Canvas widget** handles schematic rendering (2D paths via `Frame`)
- **Shader widget** handles PCB 2D rendering (custom instanced WGSL) and 3D PCB viewer (custom wgpu pipelines)

**Rendering strategy by view mode:**

| View | Widget | Rendering | Why |
|---|---|---|---|
| Schematic | `Canvas` | `Frame` API (paths/strokes/fills) + `Cache` | Hundreds of elements, lyon tessellation is fine |
| PCB 2D | `Shader` | Custom WGSL instanced pipelines | 100K+ tracks/pads, CPU tessellation too slow |
| PCB 3D | `Shader` | Custom wgpu vertex/fragment shaders | Full 3D with PBR materials |

---

## Editions: Community vs Pro

Signex ships as two editions from the same codebase. Feature gating is compile-time (`#[cfg(feature = "pro")]`) for clean separation, with runtime license validation for Pro.

### Signex Community (Free, Open Source)

**License:** GPL-3.0
**Price:** Free forever
**Includes everything in Phases 0–11, 13–14:**
- Full schematic editor (Altium-class parity)
- Full PCB editor (routing, DRC, copper pour, Gerber/ODB++ export)
- 3D PCB viewer with PBR materials and STEP models
- Simulation: SPICE (ngspice), RF/EM (OpenEMS), Thermal (Elmer FEM)
- WASM plugin system (Extism)
- KiCad file format compatibility
- 6 built-in themes
- All Altium keyboard shortcuts
- Offline-only — no account required

### Signex Pro (Subscription)

**License:** Proprietary (separate binary, not GPL)
**Price:** Subscription (monthly/annual, pricing TBD)
**Includes everything in Community, plus:**

#### Signal AI (Pro-only — Phase 12)
- Claude API integration via Alp Lab's managed API gateway (users never need their own API key)
- Streaming chat with full design context (component lists, net connectivity, ERC/DRC violations, sim results)
- Tool use: Claude can run simulations, add components, draw wires, fix ERC violations
- Design review mode: automated review of schematic/PCB with actionable suggestions
- Circuit templates: AI-guided placement of common circuits (buck, LDO, op-amp, filter, etc.)
- Visual context: schematic/PCB screenshots sent to Claude for spatial reasoning
- Session cost included in subscription — no per-token billing to users

#### Live Collaboration (Pro-only — Phase 15)
Real-time co-editing for schematic and PCB, inspired by Altium 365 but purpose-built:

**Schematic co-editing:**
- Multiple engineers editing the same schematic simultaneously
- Per-user cursor visibility (colored cursors with username labels)
- Real-time sync of all edits: wire placement, symbol placement, property changes, label edits
- Sheet-level locking: optional exclusive lock per sheet to prevent conflicts
- Conflict resolution: last-write-wins for properties, spatial partitioning for geometry
- Change feed: live activity stream showing who changed what

**PCB co-editing:**
- Simultaneous PCB editing with per-user cursors
- Region locking: lock rectangular board regions for exclusive editing (prevents routing conflicts)
- Layer locking: lock specific copper layers per user
- Net assignment: assign nets to specific engineers for routing ownership
- Live DRC: violations appear in real-time as any collaborator makes changes

**Infrastructure (Supabase):**
- **Supabase Realtime** (WebSocket channels) for live sync — cursors, edits, presence, activity
- **Supabase Postgres** for project metadata, edit log, comments, locks, reviews (with RLS)
- **Supabase Storage** (S3-compatible) for project files (.snxsch, .snxpcb, STEP models)
- **Supabase Auth** (GoTrue) for accounts, team invites, JWT tokens, Pro license validation
- **Supabase Edge Functions** (Deno) for CRDT merge, notifications, lock management
- CRDT-based document model (conflict-free merging via Edge Function arbiter)
- Project workspace: shared project with role-based access (owner, editor, viewer) via Postgres RLS
- Version history: full edit history with per-change attribution in `edit_log` table
- Comments and annotations: pin comments to specific schematic/PCB locations, real-time via Realtime
- Review workflow: request review, approve/reject changes, merge branches
- Offline support: local SQLite queue for ops, replay on reconnect
- No custom server to build, deploy, or maintain — Supabase handles all infrastructure

**Presence and awareness:**
- Online status: see who's viewing/editing the project
- Follow mode: click a collaborator's avatar to follow their viewport
- Voice/text chat: integrated lightweight communication (optional, via WebRTC)

### Feature Gate Architecture

```
signex/
├── crates/
│   ├── signex-app/Cargo.toml
│   │   [features]
│   │   default = []
│   │   pro = ["signex-signal", "signex-collab"]    # Pro edition
│   │
│   ├── signex-signal/          # Signal AI — Pro only
│   │   ├── Cargo.toml          # NOT in default workspace members
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs       # Claude API via Alp Lab gateway
│   │       ├── chat.rs         # Chat session management
│   │       ├── tools.rs        # Tool use definitions
│   │       ├── context.rs      # Design context injection
│   │       ├── review.rs       # Automated design review
│   │       └── templates.rs    # Circuit template library
│   │
│   └── signex-collab/          # Live collaboration — Pro only
│       ├── Cargo.toml          # NOT in default workspace members
│       └── src/
│           ├── lib.rs
│           ├── realtime.rs     # Supabase Realtime WebSocket client
│           ├── crdt.rs         # CRDT document model (client-side)
│           ├── cursor.rs       # Remote cursor tracking + rendering
│           ├── presence.rs     # Online status, follow mode (Supabase Presence)
│           ├── lock.rs         # Sheet/region/layer/net locking
│           ├── sync.rs         # Document sync via Realtime channels
│           ├── offline.rs      # Offline op queue (rusqlite)
│           ├── storage.rs      # Supabase Storage client (project files)
│           ├── auth.rs         # Supabase Auth + license validation
│           ├── history.rs      # Version history (PostgREST queries)
│           ├── comments.rs     # Location-pinned comments
│           └── review.rs       # Review workflow
│
│   # Server-side logic lives in supabase/functions/ (Deno Edge Functions)
│   # NOT a Rust crate — no custom server to deploy
```

**Build commands:**
```bash
# Community edition (default)
cargo build --workspace --release

# Pro edition
cargo build --workspace --release --features pro

# Supabase backend (deploy Edge Functions + migrations)
cd supabase && supabase db push && supabase functions deploy
```

**Runtime license check (Pro):**
```rust
#[cfg(feature = "pro")]
mod license {
    /// Validates subscription via Alp Lab license server.
    /// Returns user info + expiry. Caches locally for 7 days offline grace.
    pub async fn validate_license(key: &str) -> Result<LicenseInfo, LicenseError> { ... }
}
```

### Pricing Model

| | Community | Pro |
|---|---|---|
| Price | Free | $X/month or $Y/year (TBD) |
| License | GPL-3.0 | Proprietary |
| Schematic editor | Full | Full |
| PCB editor | Full | Full |
| 3D viewer | Full | Full |
| Simulation | Full | Full |
| Plugins (WASM) | Full | Full |
| Signal AI | -- | Included (no per-token cost) |
| Live collaboration | -- | Included (Supabase Realtime) |
| Cloud storage | -- | Included (Supabase Storage) |
| Version history | Local git only | Cloud + per-edit attribution |
| Comments & review | -- | Included (pinned to canvas) |
| Team management | -- | Included (owner/editor/viewer roles) |
| Offline sync | -- | Included (local SQLite queue) |
| Priority support | Community forum | Email + chat |

---

## Altium Designer UX Reference

Everything in Signex must match Altium Designer's UX patterns. This section is the canonical reference.

### Panel System

**Panel Behavior:**
- Panels dock to left, right, and bottom regions
- Panels within a dock are tabbed (click tab to switch)
- Panels can be collapsed to a vertical rail (icon + label)
- Panels can float as separate windows
- Panel visibility is context-aware: schematic panels hide in PCB mode and vice versa
- Properties panel (F11) is the most important — context-aware based on selection
- Double-clicking a panel tab undocks it to floating

**Panel Registry (Altium-equivalent):**

| Panel | Dock | Context | Altium Name |
|---|---|---|---|
| Projects | Left | Both | Projects |
| Components | Left | Both | Components |
| Navigator | Left | Both | Navigator |
| Libraries | Left | Both | Installed Libraries |
| SCH Library | Left | Schematic | SCH Library |
| PCB Library | Left | PCB | PCB Library |
| Net Classes | Left | Both | Net Classes |
| Properties | Right | Both | Properties |
| Filter | Right | Schematic | SCH Filter |
| List | Right | Schematic | SCH List |
| Inspector | Right | Schematic | Inspector |
| Snippets | Right | Schematic | Snippets |
| Variants | Right | Schematic | Variants |
| Net Inspector | Right | PCB | Net Inspector |
| Layer Stack | Right | PCB | Layer Stack Manager |
| Messages | Bottom | Both | Messages |
| Signal (AI) | Bottom | Both | — (Signex-specific) |
| Output Jobs | Bottom | Schematic | Output Job |
| DRC | Bottom | PCB | DRC |
| Cross Section | Bottom | PCB | Board Cross Section |
| Waveform | Bottom | Sim | — |
| S-Parameters | Bottom | Sim | — |
| Thermal | Bottom | Sim | — |

### Mouse Behavior (Altium Standard)

| Action | Mouse | Notes |
|---|---|---|
| **Pan** | Right-click + drag | NOT left-click drag (critical difference from other apps) |
| **Pan** | Middle-click + drag | Alternative pan |
| **Zoom** | Scroll wheel | Zooms centered on cursor position |
| **Select** | Left-click | Single object selection |
| **Toggle select** | Shift + left-click | Add/remove from selection |
| **Box select (enclosing)** | Left-drag left→right (empty area) | Only fully enclosed objects selected |
| **Box select (crossing)** | Left-drag right→left (empty area) | Any overlapping objects selected |
| **Context menu** | Right-click (no drag) | Shows context-sensitive menu |
| **Properties** | Double-click | Opens properties for object |
| **In-place edit** | F2 or click-pause-click | Edit text directly on canvas |
| **Move with rubber-band** | Left-drag on selected | Wires stretch to maintain connections |
| **Stiff move** | Ctrl + left-drag on selected | Move without rubber-banding |
| **Cross-probe** | Ctrl + double-click | Jump between schematic ↔ PCB |
| **Net highlight** | Alt + click | Highlight entire net across sheets |
| **Wire endpoint drag** | Left-drag on wire end | Reposition wire endpoint |
| **Wire segment drag** | Left-drag on wire mid | Reposition wire segment |

### Single Layer Mode (PCB — Shift+S)

Cycles through 4 modes:
1. **Off** — all visible layers at full opacity
2. **Hide** — only active layer visible, all others completely hidden
3. **Grayscale** — active layer at full color, inactive desaturated to gray
4. **Monochrome** — active layer at full color, inactive at dim single color

**Layer pairs:** `+` or `=` key toggles active layer between F.Cu and B.Cu. Flipping a component (`F` key) moves it between paired layers (Top↔Bottom). Paired tech layers (Top Overlay↔Bottom Overlay, Top Solder↔Bottom Solder) follow automatically.

### Keyboard Shortcuts (Complete)

#### General Editing
| Key | Action | Context |
|---|---|---|
| Ctrl+C | Copy | Both |
| Ctrl+X | Cut | Both |
| Ctrl+V | Paste | Both |
| Shift+Ctrl+V | Smart Paste | Both |
| Ctrl+D | Duplicate | Both |
| Delete | Delete selection | Both |
| Backspace | Remove last wire point / delete | Both |
| Ctrl+Z | Undo | Both |
| Ctrl+Y / Ctrl+Shift+Z | Redo | Both |
| F2 | In-place text edit | Both |
| Ctrl+F | Find | Both |
| Ctrl+H | Find and Replace | Both |
| Ctrl+Q | Toggle mm/mil/inch | Both |
| Ctrl+A | Select all | Both |
| Ctrl+M | Measure distance | Both |

#### Placement & Drawing
| Key | Action | Context |
|---|---|---|
| W | Draw wire | Schematic |
| B | Draw bus | Schematic |
| T | Place text | Schematic |
| L | Place net label | Schematic |
| P | Place component (from search) | Schematic |
| Escape | Cancel current action / deselect | Both |
| Tab | Pause placement, open Properties | Both |
| Enter | Confirm placement | Both |

#### Transformation
| Key | Action | Context |
|---|---|---|
| Space | Rotate 90 CCW | Both |
| Shift+Space | Change wire routing mode | Schematic |
| R | Rotate selected | Both |
| X | Mirror X (horizontal flip) | Both |
| Y | Mirror Y (vertical flip) | Both |

#### View
| Key | Action | Context |
|---|---|---|
| Home | Fit all / center view | Both |
| G | Cycle grid size forward | Both |
| Shift+G | Cycle grid size backward | Both |
| Shift+Ctrl+G | Toggle grid visibility | Both |
| F5 | Toggle net color override | Both |
| F11 | Toggle Properties panel | Both |
| Ctrl+Shift+A | Open Signal AI panel | Both |
| Shift+F | Find Similar Objects | Both |

#### Nudge
| Key | Action | Context |
|---|---|---|
| Ctrl+Arrow | Nudge by 1 grid | Both |
| Shift+Ctrl+Arrow | Nudge by 10 grid | Both |

#### Selection Memory
| Key | Action | Context |
|---|---|---|
| Ctrl+1-8 | Store selection to slot | Both |
| Alt+1-8 | Recall selection from slot | Both |

### Layer Colors (Altium Default)

```
Copper Layers (32):
  Top Layer (F.Cu):         #FF0000  Red
  Mid Layer 1 (In1.Cu):     #FFFF00  Yellow
  Mid Layer 2 (In2.Cu):     #00FF00  Green
  Mid Layer 3 (In3.Cu):     #00FFFF  Cyan
  Mid Layer 4 (In4.Cu):     #FF00FF  Magenta
  Mid Layer 5 (In5.Cu):     #808000  Olive
  Mid Layer 6 (In6.Cu):     #008080  Teal
  Mid Layer 7 (In7.Cu):     #800080  Purple
  Mid Layer 8 (In8.Cu):     #FF8000  Orange
  Mid Layer 9 (In9.Cu):     #0080FF  Azure
  Mid Layer 10 (In10.Cu):   #80FF00  Chartreuse
  Mid Layer 11 (In11.Cu):   #FF0080  Rose
  Mid Layer 12 (In12.Cu):   #00FF80  Spring
  Mid Layer 13 (In13.Cu):   #8000FF  Violet
  Mid Layer 14 (In14.Cu):   #FF8080  Salmon
  Mid Layer 15 (In15.Cu):   #80FF80  Light Green
  Mid Layer 16 (In16.Cu):   #8080FF  Light Blue
  (Mid Layers 17-30: cycling colors from palette)
  Bottom Layer (B.Cu):      #0000FF  Blue

Technical Layers:
  Top Overlay (F.SilkS):    #FFFF00  Yellow
  Bottom Overlay (B.SilkS): #404080  Dark Blue-Gray
  Top Solder (F.Mask):      #800080  Purple (40% alpha)
  Bottom Solder (B.Mask):   #008080  Teal (40% alpha)
  Top Paste (F.Paste):      #808080  Gray (90% alpha)
  Bottom Paste (B.Paste):   #004040  Dark Teal (90% alpha)
  Top Assembly (F.Fab):     #AFAFAF  Light Gray
  Bottom Assembly (B.Fab):  #585D84  Slate
  Top Courtyard (F.CrtYd):  #FF26E2  Pink
  Bottom Courtyard (B.CrtYd): #26E9FF Cyan
  Keep-Out (Edge.Cuts):     #FF00FF  Magenta
  Board Outline (Margin):   #FFFF00  Yellow

Mechanical Layers:
  Mechanical 1 (Dwgs.User): #FF8000  Orange
  Mechanical 2 (Cmts.User): #5994DC  Steel Blue
  Mechanical 3 (Eco1.User): #B4DBD2  Mint
  Mechanical 4 (Eco2.User): #D8C852  Gold

Virtual/System Layers:
  Multi-Layer:              #C0C0C0  Silver
  Via Holes:                #E3B72E  Gold
  Via Holewalls:            #ECECEC  Near White
  Plated Holes:             #C2C200  Dark Yellow
  Non-Plated Holes:         #1AC4D2  Cyan
  Ratsnest:                 #00F8FF  Bright Cyan (35% alpha)
  DRC Error:                #00FF00  Green (80% alpha)
  DRC Warning:              #FFD042  Amber (80% alpha)
  Selection Overlay:        #FFFFFF  White
  Grid:                     #404040  Dark Gray
  Cursor:                   #FFFFFF  White
  PCB Background:           #000000  Black (all themes)
```

### Schematic Canvas Colors (6 Built-in Themes)

| Element | Catppuccin Mocha | VS Code Dark | Altium Dark | GitHub Dark | Solarized Light | Nord |
|---|---|---|---|---|---|---|
| Background | #1A1B2E | #1E1E1E | #1A1A1A | #0D1117 | #EEE8D5 | #2E3440 |
| Paper | #1E2035 | #252526 | #FFFFFF | #161B22 | #FDF6E3 | #3B4252 |
| Wire | #4FC3F7 | #4EC994 | #0000FF | #58A6FF | #268BD2 | #88C0D0 |
| Junction | #4FC3F7 | #4EC994 | #0000FF | #58A6FF | #268BD2 | #88C0D0 |
| Body | #9FA8DA | #DCDCAA | #000000 | #C9D1D9 | #657B83 | #81A1C1 |
| Body Fill | #1E2035 | #252526 | #FFFFC0 | #161B22 | #FDF6E3 | #3B4252 |
| Pin | #81C784 | #569CD6 | #880000 | #3FB950 | #859900 | #A3BE8C |
| Reference | #E8C66A | #FFE0A0 | #0000AA | #D29922 | #B58900 | #EBCB8B |
| Value | #9598B3 | #9D9D9D | #444444 | #8B949E | #657B83 | #D8DEE9 |
| Net Label | #81C784 | #4EC994 | #880000 | #3FB950 | #859900 | #A3BE8C |
| Global Label | #FF8A65 | #CE9178 | #CC6600 | #FFA657 | #CB4B16 | #D08770 |
| Hier Label | #BA68C8 | #C586C0 | #660066 | #BC8CFF | #6C71C4 | #B48EAD |
| No Connect | #E8667A | #F48771 | #CC0000 | #F85149 | #DC322F | #BF616A |
| Power | #EF5350 | #D16969 | #FF0000 | #FF7B72 | #D33682 | #D08770 |
| Selection | #00BCD4 | #007ACC | #00AAFF | #388BFD | #2AA198 | #88C0D0 |
| Bus | #4A86C8 | #307ABC | #000088 | #2F6AF5 | #1A7AA3 | #5E81AC |

### Status Bar Layout

Left-to-right:
1. Cursor position: `X:12.54 Y:8.20` (with unit conversion)
2. Grid size: `2.54mm` (click to toggle visibility, G to cycle)
3. Snap indicator: `Snap` / `Free` (click to toggle)
4. Electrical snap: `E-Snap` (always on)
5. Active layer (PCB only): `Top Layer`
6. Current mode: `Select` / `Draw Wire` / etc.
7. _(spacer)_
8. Zoom: `100%`
9. Units: `mm` / `mil` / `inch` (click to cycle, Ctrl+Q)
10. Panels button: dropdown to toggle panel visibility

---

## Gap Analysis & Parallel Workstreams

See **`docs/altium-gap-analysis.md`** for:
- Complete Altium feature gap analysis (67 critical gaps, 27 important gaps, 12 deferred)
- Constraint Manager design (schematic-side spreadsheet rule editor)
- 17 Signex-exclusive features that go beyond Altium (AI design review, thermal-aware routing, impedance-on-canvas, BOM cost optimization, live DRC, natural language constraints, etc.)
- Parallel workstream map with crate ownership and sprint schedule

### Summary: 10 Parallel Workstreams

The plan is restructured so **multiple engineers work simultaneously** on independent workstreams, each owning specific crates with minimal overlap:

| Workstream | Owner | Crates | Key Deliverables |
|---|---|---|---|
| **WS-K** Parser & Types | Eng A (Sprint 1) | `kicad-parser`, `kicad-writer`, `signex-types` | KiCad format support, domain types |
| **WS-A** Editor Core | Eng B→A | `signex-app` (shell/canvas/panels), `signex-render/schematic`, `formula-render` | Iced shell, canvas, schematic editor |
| **WS-B** Validation | Eng C | `signex-erc`, `signex-drc` | ERC 11 rules, DRC 15+ rules, ECO dialog |
| **WS-C** Output | Eng D | `kicad-writer`, output modules | PDF, BOM, Gerber, ODB++, panelization, drill table |
| **WS-D** Router | Eng B (Sprint 4) | `pcb-geom` (router) | Interactive routing, diff pair, length tune, glossing |
| **WS-E** PCB Core | Eng A (Sprint 4) | `signex-render/pcb`, `pcb-geom`, `signex-app/canvas/pcb` | PCB viewer, copper pour, layer stack, keepout |
| **WS-F** High-Speed | Eng B (Sprint 5) | `pcb-geom` (xsignal) | xSignals, topology constraints, return path |
| **WS-G** 3D Viewer | Eng E | `signex-render/pcb_3d`, `step-loader` | 3D PCB, PBR, STEP, 3D clearance |
| **WS-H** Library | Eng B (Sprint 2) | Library module in `signex-app` | Unified component model, lifecycle, suppliers |
| **WS-I** Import | Eng C (Sprint 2) | `kicad-parser` extensions | Altium .SchDoc/.PcbDoc, Eagle .sch/.brd import |
| **WS-J** Simulation | Eng A (Sprint 5) | `spice-gen`, `openems-bridge`, `elmer-bridge` | SPICE, OpenEMS, Elmer |

### Sprint Schedule — Core First (v1.0 in 6 sprints)

**Goal: ship v1.0 (usable editor) as fast as possible, then layer features incrementally.**

| Sprint | Weeks | Eng | Version | Deliverables |
|---|---|---|---|---|
| **1** | 1-4 | 2 | v0.1-v0.2 | WS-K (parser + types + coord system), WS-A (scaffold + canvas) |
| **2** | 5-8 | 3 | v0.3-v0.4 | WS-A (schematic viewer), WS-K (writer), WS-C (start output) |
| **3** | 9-14 | 4 | v0.5-v0.6 | WS-A (schematic editor), WS-B (ERC), WS-C (PDF + BOM + library editor) |
| **4** | 15-22 | 5 | v0.7-v0.8 | WS-E (PCB viewer), WS-D (router), WS-B (DRC 15 rules), WS-C (PCB output) |
| **5** | 23-28 | 5 | v0.9-v0.10 | WS-E (PCB editor), WS-D (routing complete), WS-C (Gerber/ODB++/STEP) |
| **6** | 29-34 | 5 | v0.11-v1.0 | Native format, Canvas Docs basic, installers, performance, QA, **v1.0 RELEASE** |

**Post v1.0 — feature releases (parallel, not sequential):**

| Sprint | Weeks | Version | Focus |
|---|---|---|---|
| **7** | 35-40 | v1.1 | 3D viewer + STEP |
| **8** | 41-48 | v1.2 | Advanced SCH (variants, multi-channel, tables, named unions) |
| **9** | 49-58 | v1.3 | Advanced PCB (layer stack, impedance, constraints, keepout, castellated) |
| **10** | 59-66 | v1.4-v1.5 | Simulation (ngspice + OpenEMS + Elmer) |
| **11** | 67-74 | v1.6 | High-speed (xSignals, DDR SI, eye diagram, PDN) |
| **12** | 75-82 | v1.7-v1.8 | Signal AI (Pro) + BOM Studio (Pro) + plugins |
| **13** | 83-90 | v2.0-v2.1 | **PRO RELEASE** — collaboration (Supabase) |
| **14+** | 91+ | v2.2+ | Advanced output, Git, Altium import, creepage, autorouter |

**Key rule: each workstream owns specific crates — no two workstreams edit the same crate simultaneously.** The shared `signex-types` crate requires PR review from all affected workstreams when modified.

---

---

## Repository: alplabai/signex

### Branch Strategy

```
main                    # Protected. Stable releases only. Tagged vX.Y.Z.
└── dev                 # Default branch. All feature branches merge here via PR.
    ├── feature/...     # New features: feature/v0.6-full-editor
    ├── fix/...         # Bug fixes: fix/parser-unicode
    └── hotfix/...      # Urgent fixes branched from main, merged to both main and dev
```

**Rules:**
- `main` is protected — requires PR with 1 approval, no direct pushes, no force pushes.
- `dev` is the integration branch. Feature branches merge here via PR.
- Feature branches: `feature/<description>` format.
- Hotfixes: `hotfix/<description>` branched from `main`, merged to both `main` and `dev`.
- Every merge to `main` gets a version tag (e.g., `v0.6.0`).
- CI runs on every push to `dev` and `feature/*`.

---

## Workspace Structure

```
signex/
├── Cargo.toml                        # [workspace] manifest
├── CLAUDE.md                         # Agent instructions
├── LICENSE                           # GPL-3.0
├── README.md
├── crates/
│   ├── signex-app/                   # Main binary — Iced Application
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs               # Entry point + App struct
│   │       ├── app.rs                # Iced Application impl (Message, update, view)
│   │       ├── theme.rs              # 6 themes + token system
│   │       ├── shortcuts.rs          # Keyboard shortcut registry (Altium-compatible)
│   │       ├── menu_bar.rs           # Top menu bar widget
│   │       ├── toolbar.rs            # Schematic/PCB toolbar strips
│   │       ├── status_bar.rs         # Bottom status bar
│   │       ├── tab_bar.rs            # Document tab bar
│   │       ├── dock/                 # Panel docking system
│   │       │   ├── mod.rs
│   │       │   ├── dock_area.rs      # pane_grid-based dock container
│   │       │   ├── panel_rail.rs     # Collapsed panel rail (vertical icons)
│   │       │   └── panel_tab.rs      # Tab header within dock
│   │       ├── panels/               # All panel implementations
│   │       │   ├── mod.rs
│   │       │   ├── projects.rs
│   │       │   ├── components.rs     # Component browser (226 KiCad libs)
│   │       │   ├── navigator.rs
│   │       │   ├── properties.rs     # Context-aware property editor
│   │       │   ├── filter.rs         # SCH selection filter
│   │       │   ├── list.rs           # SCH object list
│   │       │   ├── messages.rs       # ERC violations
│   │       │   ├── signal.rs         # Signal AI chat panel
│   │       │   ├── output_jobs.rs
│   │       │   ├── inspector.rs
│   │       │   ├── drc.rs            # DRC violations
│   │       │   ├── layer_stack.rs    # PCB layer manager
│   │       │   ├── net_classes.rs
│   │       │   ├── net_inspector.rs
│   │       │   ├── variants.rs
│   │       │   ├── snippets.rs
│   │       │   ├── cross_section.rs  # PCB board cross section
│   │       │   ├── waveform.rs       # SPICE waveform viewer
│   │       │   ├── sparam.rs         # S-parameter + Smith chart
│   │       │   └── thermal.rs        # Thermal map viewer
│   │       ├── canvas/               # wgpu canvas widgets
│   │       │   ├── mod.rs
│   │       │   ├── schematic.rs      # Schematic canvas (iced::Canvas)
│   │       │   ├── pcb.rs            # PCB 2D canvas (iced::Canvas)
│   │       │   ├── pcb_3d.rs         # PCB 3D viewer (raw wgpu)
│   │       │   ├── camera.rs         # Pan/zoom camera (right-click=pan, scroll=zoom)
│   │       │   ├── grid.rs           # Grid renderer
│   │       │   ├── hit_test.rs       # Point/box/lasso hit testing
│   │       │   └── selection.rs      # Selection overlay rendering
│   │       ├── dialogs/              # Modal dialogs
│   │       │   ├── mod.rs
│   │       │   ├── annotation.rs
│   │       │   ├── bom_config.rs
│   │       │   ├── export_pdf.rs
│   │       │   ├── netlist_export.rs
│   │       │   ├── preferences.rs
│   │       │   ├── find_similar.rs
│   │       │   ├── find_replace.rs
│   │       │   ├── parameter_manager.rs
│   │       │   ├── erc_matrix.rs
│   │       │   ├── constraints.rs
│   │       │   ├── via_stitching.rs
│   │       │   └── bga_fanout.rs
│   │       └── context_menu.rs       # Right-click context menu
│   │
│   ├── signex-types/                 # Domain types — NO rendering deps
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schematic.rs          # Wire, Label, Symbol, Sheet, Junction, BusEntry
│   │       ├── pcb.rs                # Track, Pad, Via, Zone, Footprint
│   │       ├── net.rs                # Net, NetClass, Pin, DiffPair
│   │       ├── layer.rs              # LayerId (0-63), LayerKind, LayerStackup, colors
│   │       ├── violation.rs          # ErcViolation (11), DrcViolation (15)
│   │       ├── sim.rs                # SpiceModelRef, SimJob, WaveformData, SParamData
│   │       ├── markup.rs             # Rich text parser: V_{CC}, ~{CS}, subscript/superscript/overbar
│   │       ├── project.rs            # Project, Sheet, Tab
│   │       └── theme.rs              # ThemeTokens, CanvasColors, 6 theme definitions
│   │
│   ├── kicad-parser/                 # S-expression parser — .kicad_sch, .kicad_pcb, .kicad_sym
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sexpr.rs              # S-expression tokenizer + tree builder
│   │       ├── schematic.rs          # .kicad_sch → SchematicSheet
│   │       ├── pcb.rs                # .kicad_pcb → PcbBoard
│   │       ├── symbol_lib.rs         # .kicad_sym → SymbolLibrary
│   │       └── tests/
│   │
│   ├── kicad-writer/                 # S-expression serializer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schematic.rs
│   │       └── pcb.rs
│   │
│   ├── signex-render/                # wgpu rendering primitives (no Iced dep)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schematic/            # Schematic draw routines
│   │       │   ├── mod.rs
│   │       │   ├── wire.rs
│   │       │   ├── symbol.rs         # LibSymbol → paths, unit/bodyStyle filtering
│   │       │   ├── pin.rs
│   │       │   ├── label.rs          # Net, Global, Hierarchical, Power label shapes
│   │       │   ├── junction.rs
│   │       │   ├── sheet.rs
│   │       │   ├── text.rs
│   │       │   ├── drawing.rs        # Line, rect, circle, arc, polyline, polygon
│   │       │   └── rich_label.rs     # Markup rendering (subscript, superscript, overbar)
│   │       ├── pcb/                  # PCB 2D draw routines
│   │       │   ├── mod.rs
│   │       │   ├── track.rs
│   │       │   ├── pad.rs
│   │       │   ├── via.rs
│   │       │   ├── zone.rs
│   │       │   ├── footprint.rs
│   │       │   ├── silkscreen.rs
│   │       │   └── layer_compositing.rs
│   │       ├── pcb_3d/               # 3D PCB render (raw wgpu pipelines)
│   │       │   ├── mod.rs
│   │       │   ├── extrude.rs        # 2D copper → 3D mesh
│   │       │   ├── materials.rs      # PBR: copper, soldermask, FR4, silkscreen
│   │       │   ├── step_loader.rs    # STEP → mesh (truck-modeling)
│   │       │   └── thermal_overlay.rs
│   │       └── colors.rs             # Color utilities, theme color resolution
│   │
│   ├── signex-erc/                   # Electrical Rules Check engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── rules.rs              # 11 ERC rule implementations
│   │       └── pin_matrix.rs         # 12x12 pin connection matrix
│   │
│   ├── signex-drc/                   # Design Rules Check engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── rules.rs              # 15 DRC rule implementations
│   │
│   ├── pcb-geom/                     # PCB geometry operations
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── polygon.rs            # Clipper2 polygon boolean
│   │       ├── copper_pour.rs        # Zone filling, thermal relief
│   │       ├── ratsnest.rs           # MST + UnionFind
│   │       ├── router.rs             # Interactive router (walkaround, push/shove)
│   │       ├── diff_pair.rs          # Differential pair routing
│   │       ├── length_tune.rs        # Meander length tuning
│   │       └── hit_test.rs           # Geometric hit testing
│   │
│   ├── spice-gen/                    # SPICE netlist generation + result parsing
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── netlist.rs            # SchematicDoc → .cir
│   │       ├── raw_parser.rs         # .raw → WaveformData
│   │       └── ngspice_ffi.rs        # libngspice FFI (libloading)
│   │
│   ├── openems-bridge/               # OpenEMS FDTD bridge
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── csx_writer.rs
│   │       ├── geometry.rs
│   │       ├── materials.rs
│   │       ├── ports.rs
│   │       ├── mesh.rs
│   │       ├── runner.rs
│   │       ├── hdf5_reader.rs
│   │       └── sparam.rs
│   │
│   ├── elmer-bridge/                 # Elmer FEM bridge
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sif_writer.rs
│   │       ├── geometry.rs
│   │       ├── mesh.rs
│   │       ├── materials.rs
│   │       ├── solver_thermal.rs
│   │       ├── solver_dc_ir.rs
│   │       ├── solver_em.rs
│   │       ├── runner.rs
│   │       └── vtk_reader.rs
│   │
│   ├── formula-render/               # LaTeX/math formula rendering
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── inline.rs             # rex → SVG → RGBA
│   │       ├── block.rs              # typst → RGBA
│   │       └── cache.rs              # Formula image cache
│   │
│   ├── step-loader/                  # STEP import
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   │
│   ├── plugin-api/                   # WASM plugin system (Extism)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── host.rs               # Host function categories
│   │       └── permission.rs         # Permission gateway
│   │
│   ├── signex-signal/                # Signal AI — PRO ONLY
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs             # Claude API via Alp Lab gateway
│   │       ├── chat.rs               # Chat session management
│   │       ├── tools.rs              # Tool use definitions
│   │       ├── context.rs            # Design context injection
│   │       ├── review.rs             # Automated design review
│   │       └── templates.rs          # Circuit template library
│   │
│   └── signex-collab/                # Live collaboration client — PRO ONLY
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── realtime.rs           # Supabase Realtime WebSocket client
│           ├── crdt.rs               # CRDT document model (client-side)
│           ├── cursor.rs             # Remote cursor tracking + rendering
│           ├── presence.rs           # Online status, follow mode (Supabase Presence)
│           ├── lock.rs               # Sheet/region/layer/net lock requests
│           ├── sync.rs               # Document sync via Realtime channels
│           ├── offline.rs            # Offline op queue (rusqlite)
│           ├── history.rs            # Version history (PostgREST queries)
│           ├── comments.rs           # Location-pinned comments
│           ├── storage.rs            # Supabase Storage client (project files)
│           ├── auth.rs               # Supabase Auth + license validation
│           └── review.rs             # Review workflow
│
├── supabase/                         # Supabase backend — PRO ONLY
│   ├── config.toml                   # Supabase project config
│   ├── migrations/                   # Postgres schema migrations
│   │   └── 001_collab_tables.sql
│   ├── functions/                    # Edge Functions (Deno/TypeScript)
│   │   ├── crdt-merge/index.ts       # Merge CRDT ops, broadcast via Realtime
│   │   ├── notify-review/index.ts    # Notification dispatch
│   │   ├── lock-acquire/index.ts     # Lock acquire/release with timeout
│   │   └── cleanup-expired/index.ts  # Cron: release expired locks
│   └── seed.sql                      # Dev seed data
│
├── assets/
│   ├── fonts/                        # Iosevka, Roboto, etc.
│   ├── icons/                        # Toolbar/panel icons (SVG)
│   └── kicad-libs/                   # Bundled KiCad symbol libraries
│
├── tests/
│   ├── fixtures/                     # KiCad test files (.kicad_sch, .kicad_pcb)
│   └── integration/                  # Integration tests
│
└── docs/
    ├── altium-reference.md           # Altium UX reference
    └── architecture.md               # Architecture decisions
```

---

## Versioning & Priority Tiers

Semantic versioning: `MAJOR.MINOR.PATCH`
- `0.x` = Alpha/Beta — building toward a usable tool
- `1.0` = First public release — a complete schematic + PCB editor that replaces Altium for basic workflows
- `1.x` = Feature releases — simulation, high-speed, advanced output
- `2.0` = Pro launch — AI + collaboration

### Priority Philosophy

**Ship a usable editor first.** An engineer should be able to open a KiCad project, edit a schematic, route a PCB, run DRC, and export Gerber by v1.0. Everything else (simulation, AI, plugins, high-speed analysis, advanced output) is value-add that comes after the core loop works.

Features are categorized into 4 tiers:

| Tier | Definition | Ships in |
|---|---|---|
| **P0 — Core** | Cannot ship without this. Basic schematic + PCB editing loop. | v0.1–v1.0 |
| **P1 — Professional** | Needed for professional use. Engineers expect these but can work without them briefly. | v1.1–v1.3 |
| **P2 — Differentiator** | Signex advantages over Altium. Competitive moat. Drives adoption and Pro subscriptions. | v1.4–v2.0 |
| **P3 — Nice-to-have** | Good features that can wait. Add when core is stable. | v2.1+ |

---

### Release Plan

```
v0.1.0  Scaffold          ── window, panels, themes, status bar, dock system
v0.2.0  Parser            ── KiCad .kicad_sch/.kicad_pcb/.kicad_sym parse + write
v0.3.0  Canvas            ── wgpu canvas, pan/zoom/grid, coordinate system (nm, mm/mil/µm)
v0.4.0  Schematic Viewer  ── render KiCad schematics, multi-sheet nav, all element types
v0.5.0  Schematic Editor  ── select, move, wire, delete, rotate, undo/redo, save, properties
v0.6.0  Full Editor       ── copy/paste, labels, components, bus, in-place edit, context menu
v0.7.0  Validation        ── ERC 11 checks, pin matrix, annotation, basic design constraints
v0.8.0  Output            ── PDF, BOM (CSV/HTML/Excel), netlist, library editor
v0.9.0  PCB Viewer        ── render PCB, layer compositing, layer stack panel, cross-probe
v0.10.0 PCB Editor        ── routing (walkaround/push), DRC 15 rules, copper pour, via, export
v0.11.0 PCB Output        ── Gerber X2, Excellon, ODB++, pick-and-place, STEP 3D export
──────── v1.0.0 COMMUNITY RELEASE ─── usable schematic + PCB editor ────────────
v1.1.0  3D Viewer         ── 3D PCB, PBR materials, STEP model loading
v1.2.0  Advanced SCH      ── variants, multi-channel, harness, parameter manager, templates
v1.3.0  Advanced PCB      ── layer stack editor, impedance calc, diff pair, length matching
v1.4.0  Simulation        ── SPICE (ngspice), waveform viewer
v1.5.0  EM Simulation     ── OpenEMS (S-params, TDR), Elmer (thermal, IR drop)
v1.6.0  High-Speed        ── xSignals, DDR SI (eye diagram, channel sim), PDN analysis
v1.7.0  Signal AI         ── Claude API (Pro), design review, tool use, circuit templates
v1.8.0  Plugins           ── Extism WASM, host functions, permission gateway
──────── v2.0.0 PRO RELEASE ─── AI + collaboration ─────────────────────────────
v2.1.0  Collaboration     ── Supabase real-time co-editing, presence, locking (Pro)
v2.2.0  Advanced Output   ── panelization, Canvas Docs, drill table, DXF, Gerber X3
v2.3.0  Advanced Features ── Git integration, Altium import, creepage, MCPCB stackup
v2.4.0  Polish            ── antenna sim, auto-router, rigid-flex, supplier data
```

---

### Detailed Feature → Version Mapping

#### v0.1–v0.3: Foundation (P0)

| Version | Feature | Gap ID |
|---|---|---|
| v0.1 | Iced shell, dock system (PaneGrid + Tabs), tree view widget | — |
| v0.1 | Menu bar, toolbar, status bar, document tab bar | — |
| v0.1 | 6 themes (Catppuccin, VS Code, GitHub, Altium, Solarized, Nord) | — |
| v0.2 | S-expression tokenizer + schematic/PCB/symbol parser | — |
| v0.2 | S-expression writer (save) | — |
| v0.2 | Rich text markup parser (subscript, superscript, overbar) | — |
| v0.2 | Domain types: Wire, Symbol, Label, Junction, Track, Pad, Via, Zone | — |
| v0.2 | Coordinate system: i64 nanometers, mm/mil/inch/µm units | G59l, G60y |
| v0.3 | wgpu Canvas widget with 3-layer cache (background/content/overlay) | — |
| v0.3 | Camera: right-click pan, scroll zoom, Home = fit all | — |
| v0.3 | Grid: dot grid, G to cycle, Shift+Ctrl+G toggle, snap | — |
| v0.3 | Cursor coordinates → status bar | — |

#### v0.4–v0.6: Schematic Editing (P0)

| Version | Feature | Gap ID |
|---|---|---|
| v0.4 | File open dialog (rfd), parse .kicad_pro | — |
| v0.4 | Render all schematic elements (13 z-layers, KiCad order) | — |
| v0.4 | Symbol rendering: unit/bodyStyle filter, 2x2 matrix transform | — |
| v0.4 | Rich label rendering (all text through spawn_rich_label) | — |
| v0.4 | Pin rendering: line + marker + name/number labels | — |
| v0.4 | Title block, sheet border, paper sizes | — |
| v0.4 | Multi-sheet navigation (Projects panel tree) | — |
| v0.4 | Theme-aware colors for all elements | — |
| v0.5 | Left-click select, Shift+click toggle, box select (crossing/enclosing) | — |
| v0.5 | Move with rubber-banding, Ctrl = stiff move | — |
| v0.5 | Wire drawing (W key), 3 routing modes (Shift+Space cycle) | — |
| v0.5 | Auto-junction at T-intersections | — |
| v0.5 | Delete, Rotate (Space/R), Mirror X/Y | — |
| v0.5 | Undo/Redo (50 levels, descriptive history) | G60o |
| v0.5 | Save (Ctrl+S), dirty flag on tab | — |
| v0.5 | Properties panel (F11), context-aware | — |
| v0.6 | Copy/Cut/Paste (Ctrl+C/X/V), Duplicate (Ctrl+D) | — |
| v0.6 | Label placement (L key): net, global, hierarchical, power | — |
| v0.6 | Component placement (P key): library browser, auto-designator | — |
| v0.6 | Bus drawing (B key), bus entries | — |
| v0.6 | In-place text edit (F2) | — |
| v0.6 | Context menu (right-click no-drag) | — |
| v0.6 | Find/Replace (Ctrl+F/H) | — |
| v0.6 | Selection memory (Ctrl+1-8, Alt+1-8) | — |
| v0.6 | Selection filter (per-type toggle) | — |
| v0.6 | Measure tool (Ctrl+M) | G1 |
| v0.6 | Keyboard shortcut system (Altium-compatible defaults) | — |

#### v0.7–v0.8: Validation & Output (P0)

| Version | Feature | Gap ID |
|---|---|---|
| v0.7 | ERC engine: 11 violation types | — |
| v0.7 | Pin connection matrix (12x12) | — |
| v0.7 | Messages panel with click-to-zoom | — |
| v0.7 | ERC severity configuration | — |
| v0.7 | Annotation: 4 modes, preview, lock/unlock | — |
| v0.7 | Net color override (F5) | — |
| v0.7 | AutoFocus (dim unrelated objects) | — |
| v0.8 | PDF export (single/multi-sheet, DPI, color mode) | — |
| v0.8 | BOM export (CSV, TSV, HTML, Excel) | — |
| v0.8 | Netlist export (KiCad S-expr, XML) | — |
| v0.8 | Symbol library editor (create/edit symbols, pins, graphics) | — |
| v0.8 | Footprint library editor (create/edit footprints, pads) | — |
| v0.8 | Sheet templates (ISO A4, ANSI A) | — |
| v0.8 | Text string substitution (=Title, =Date, =Rev) | — |
| v0.8 | Watermarking (DRAFT/CONFIDENTIAL overlay) | G59v |
| v0.8 | Project lock / read-only mode | G60t |

#### v0.9–v0.11: PCB Core (P0)

| Version | Feature | Gap ID |
|---|---|---|
| v0.9 | PCB rendering via widget::Shader (instanced WGSL) | — |
| v0.9 | 32 copper layers + tech layers, Altium default colors | — |
| v0.9 | Layer stack panel (visibility, color, active layer) | — |
| v0.9 | Layer compositing order (KiCad/Altium) | — |
| v0.9 | Single-layer mode (off/hide/grayscale/monochrome) | — |
| v0.9 | Board flip (Ctrl+F), net color override (F5) | — |
| v0.9 | Cross-probe (SCH ↔ PCB bidirectional) | — |
| v0.9 | Ratsnest (MST + UnionFind) | — |
| v0.9 | Grid extends beyond board outline | G60n |
| v0.10 | Interactive routing: walkaround, push/shove, ignore | — |
| v0.10 | Corner styles: 45, 90, arc45, arc90, any | — |
| v0.10 | Via placement during routing (through/blind/buried) | — |
| v0.10 | DRC: 15 base rules (clearance, width, via, drill, etc.) | — |
| v0.10 | Copper pour (zone fill, thermal relief, island removal) | — |
| v0.10 | Component placement (move, rotate, flip) | — |
| v0.10 | Board outline editing | — |
| v0.10 | Diff pair routing (gap control) | — |
| v0.10 | Length tuning (accordion, sawtooth, trombone) | — |
| v0.10 | Teardrops (as design rule, per-pad property) | G59z |
| v0.10 | Multi-track routing | — |
| v0.10 | BGA fanout, via stitching | — |
| v0.10 | Back annotation / ECO | — |
| v0.11 | Gerber RS-274X + X2 export | — |
| v0.11 | Excellon drill export | — |
| v0.11 | ODB++ export | — |
| v0.11 | Pick-and-place CSV | — |
| v0.11 | STEP 3D export (board body) | — |
| v0.11 | IPC-2581 export | — |
| v0.11 | Assembly SVG | — |

#### v1.0.0 — Pre-release Additions

| Feature | Notes |
|---|---|
| Signex native format (.snxsch, .snxpcb, .snxprj, .snxsym, .snxpkg) | Binary + JSON hybrid, faster than S-expr. KiCad import/export always supported. |
| Signex Canvas Docs (basic: board assembly view + drill drawing on a drawing sheet) | Foundation for full Canvas Docs in v2.2 |
| Native file associations + "Open with Signex" on Windows/macOS/Linux | — |
| Installer (Windows .msi, macOS .dmg, Linux .AppImage) | — |
| Performance validation: 100+ component SCH at 60fps, 10K track PCB at 60fps | — |

#### ═══ v1.0.0 — COMMUNITY RELEASE ═══

A complete, usable schematic + PCB editor. An engineer can:
1. Open a KiCad project or create a new Signex project (.snxprj)
2. Edit the schematic (place components, draw wires, add labels)
3. Annotate and run ERC
4. Cross-probe to PCB
5. Route traces (walkaround, push/shove, diff pair, length tune)
6. Run DRC
7. Export Gerber, drill, BOM, PDF
8. Save as .snxsch/.snxpcb (native) or .kicad_sch/.kicad_pcb (KiCad compatible)

---

#### v1.1: 3D Viewer (P1)

| Feature | Gap ID |
|---|---|
| 3D PCB viewer (widget::Shader, custom wgpu pipelines) | — |
| PCB extrusion (layer stackup → 3D mesh) | — |
| PBR materials (copper, soldermask, FR4, silkscreen) | — |
| STEP model loading (truck-modeling, async) | — |
| Orbit camera (right-click orbit, middle pan, scroll zoom) | — |
| Board cross-section view | — |
| Convert 3D body group to STEP export | G60r |

#### v1.2: Advanced Schematic (P1)

| Feature | Gap ID |
|---|---|
| Design variants (fitted/not-fitted/alternate) | — |
| Variant drawing styles (7 options: cross/ghost/stripe/dot/tint/hide/badge) | G59m visual |
| True variants in multi-channel (per-channel state) | G59m |
| Multi-channel design (Repeat keyword, channel naming) | G59g |
| Signal harnesses (connectors, entries, nested) | — |
| Parameter Manager (spreadsheet editing) | — |
| Net classes + diff pair classes | — |
| Drawing tools (bezier, dimension) | G3, G4 |
| Named unions (hierarchical groups with tags) | G60m |
| Variant-specific BOM export | G8 |
| Variant visual on canvas (per-project toggle) | G9, G60q |
| Variant comparison mode (side-by-side overlay) | — |
| Smart paste (Ctrl+R rubber stamp, paste array) | G2 |
| Change component (swap keeping connections) | G58 |
| Schematic tables (pin assignment, register map, connector pinout) | G59j |
| Table of Contents (auto-generated with block diagram) | G59k |

#### v1.3: Advanced PCB (P1)

| Feature | Gap ID |
|---|---|
| Layer stack editor (full: εr, tanδ, copper weight, material library) | G18 |
| Impedance profile per layer pair + built-in calculator | G19 |
| Stackup templates (2/4/6/8 layer, HDI, MCPCB) | G20, G59s |
| Impedance-controlled routing (Z₀ display, width-from-impedance) | G12 |
| Interactive length gauge during routing | G13 |
| Route completion (loop removal) | G10 |
| Glossing (post-route optimization) | G11 |
| Retrace + trace dragging | G54, G55 |
| Split planes (negative plane layers) | G14 |
| Via-in-pad | G15 |
| Paste expansion / mask expansion rules | G16, G17 |
| Per-layer keepout (via/routing/component keepout types) | G24, G25 |
| Board cutouts (arbitrary internal) | G26 |
| Plated board cutouts + castellated holes | G59p, G59q |
| Square / tapered track ends | G59t |
| Polygon pour rounded edges around pads | G60x |
| Routing grid (per-layer, separate from placement grid) | G59u |
| Plane connect style (thermal/direct for internal planes) | G47, G48, G49 |
| Board Insight HUD (clearance, net name under cursor) | G52 |
| Query-based PCB filter + mask mode | G53 |
| Constraint Manager (schematic-side spreadsheet rule editor) | G59i |
| Query-based rule scoping (InNet, InNetClass, OnLayer, boolean) | G41 |
| Design rule width: min/preferred/max | G42 |
| Design rule variables ($DDR_CLR = 5mil) | G60s |
| DRC rule profiles (save/load: JLCPCB, PCBWay, IPC presets) | G59n |
| Additional DRC: silk-silk, acute angle, component clearance, height | G50, G51, G27, G28 |
| Mechanical layer clearance rules | G59w |
| Formal ECO dialog (change list with review) | G34 |
| Routing via style / priority / layer constraint per net class | G43, G44, G45, G46 |
| Area-selective DRC | G60v |
| Net Inspector panel (total length, via count per net) | G80 |

#### v1.4: Simulation — SPICE (P2)

| Feature | Gap ID |
|---|---|
| ngspice FFI (libloading) | — |
| Netlist generation (SchematicDoc → .cir) | — |
| DC, AC, Transient, Noise, Fourier analysis | — |
| .raw parser → WaveformData | — |
| Waveform panel (multi-trace, dual cursor, PNG/CSV export) | — |
| IBIS model support | — |
| Parameter sweep + temperature sweep | G72 |
| Monte Carlo analysis | — |
| Encrypted SPICE model support (.snxmod) | G59y |
| S-parameter import (.s2p/.s4p as SPICE elements) | G60d |

#### v1.5: EM + Thermal Simulation (P2)

| Feature | Gap ID |
|---|---|
| OpenEMS: PCB geometry → CSX XML, subprocess, HDF5 reader | — |
| S-parameter extraction (S11/S21), Smith chart, TDR | — |
| Crosstalk analysis (NEXT/FEXT) | — |
| GPU-accelerated FDTD (feature/cuda-engine) | — |
| Elmer FEM: PCB → GMSH mesh → .sif, subprocess, VTK reader | — |
| Thermal analysis (steady-state, component heat sources) | — |
| DC IR drop (voltage distribution, current density) | — |
| Joule heating (coupled thermal-electrical) | — |
| 3D thermal overlay on PCB model | — |
| Sim job queue (UUID, status, progress, cancel) | — |
| Via impedance + delay calculation (analytical) | G60j, G60k |
| Via stub resonance estimation | G60l |

#### v1.6: High-Speed Design (P2)

| Feature | Gap ID |
|---|---|
| xSignals (pad-to-pad through-component path analysis) | G21 |
| Length matching group management UI + bar chart dashboard | G22 |
| Topology constraints (star, chain, fly-by, T) | G23 |
| Eye diagram generation | G60a |
| DDR timing analysis (setup/hold vs spec) | G60b |
| Channel simulation (Tx IBIS → S-param cascade → Rx IBIS) | G60c |
| PDN impedance analysis Z(f) | G60e |
| Return path analyzer | G64 |
| Power plane analyzer | G65 |
| Via counting in matched length | G81 |
| Diff pair via placement (simultaneous) | G82 |
| Pad-to-pad length (from pad edge) | G83 |
| Power plane resonance detection | G60i |
| Transient thermal analysis | G60g |
| Component junction temp estimation (Rjc + FEM) | G60h |

#### v1.7: Signal AI + BOM Studio — Pro (P2)

| Feature | Gap ID |
|---|---|
| **BOM Studio** (live BOM management panel — Pro) | G59b |
| BOM Studio: real-time pricing + availability (Octopart/Mouser/Digi-Key API) | G70 |
| BOM Studio: lifecycle status (active/NRND/EOL) with color indicators | G36 |
| BOM Studio: part choice ranking (green/orange/red by availability + price) | G70 |
| BOM Studio: variant-filtered BOM (select variant → BOM updates) | G8 |
| BOM Studio: supply chain risk alerts (single-source, low stock, long lead time) | S2 |
| Claude API via Alp Lab managed gateway | — |
| Signal panel (streaming chat, markdown, tool use) | — |
| Design context injection (components, nets, ERC/DRC, sim results) | — |
| Visual context (schematic/PCB screenshot to Claude vision) | — |
| Design review mode (automated scored analysis) | — |
| ERC/DRC fix suggestions + auto-apply | — |
| Circuit templates (6+ pre-built) | — |
| AI design review on commit | S1 |
| BOM cost optimization suggestions | S8 |
| Design intent auto-documentation | S9 |
| Natural language constraint entry | S16 |
| Stackup wizard (AI-assisted) | S17 |
| AI-guided routing suggestions | S5 |

#### v1.8: Plugin System (P1)

| Feature | Gap ID |
|---|---|
| Extism WASM runtime | — |
| 5 host function categories (Document, Mutation, UI, Query, Sim) | — |
| Permission gateway (plugin manifest + user approval) | — |
| Undo stack integration for plugin mutations | — |
| Hot-loading (no restart) | — |

#### ═══ v2.0.0 — PRO RELEASE ═══

Full Signex with AI + plugins. Pro subscription unlocks Signal AI + Collaboration.

---

#### v2.1: Live Collaboration — Pro (P2)

| Feature | Gap ID |
|---|---|
| Supabase Realtime (WebSocket channels for sync) | — |
| CRDT document model (conflict-free concurrent editing) | — |
| Per-user cursors on canvas (colored + name label) | — |
| Presence panel (online status, follow mode) | — |
| Sheet/region/layer/net locking | — |
| Supabase Auth (accounts, teams, roles) | — |
| Supabase Storage (project files) | — |
| Version history + edit attribution | — |
| Comments pinned to canvas locations | — |
| Review workflow (request/approve/reject) | — |
| Offline support (local SQLite op queue) | — |

#### v2.2: Advanced Output (P1)

| Feature | Gap ID |
|---|---|
| Panelization (step-and-repeat, rails, tabs, mouse bites) | G30 |
| V-cut scoring / tab routing | G68 |
| Auto-fiducials + tooling holes + test coupons | G69 |
| Canvas Docs (section views, drill drawing, component placement diagram) | G67, G59d, G59e |
| Drill table (auto-generated on fab drawing) | G31 |
| Board stackup report (PDF with layer diagram) | G32 |
| DXF import/export | G33 |
| Gerber X3 | G60p |
| Smart PDF with bookmarks + PDF layers | G59a |
| Output Job file (.OutJob equivalent) with containers | G57 |
| External program in OutJob | G60u |
| One-click manufacturing package (validated ZIP for JLCPCB/PCBWay) | S14 |
| Test point report | G74 |

#### v2.3: Import + Git + Safety (P1)

| Feature | Gap ID |
|---|---|
| Altium import (.SchDoc, .PcbDoc, .SchLib, .PcbLib, .PrjPcb) | G59o |
| Eagle import (.sch, .brd) | G40 |
| Built-in Git (branch, commit, merge, visual diff, blame) | G59x |
| Git-derived parameters (=GitBranch, =GitTag, =GitCommit in title blocks) | G60w |
| Visual schematic diff (canvas overlay) | S3 |
| Visual PCB diff (canvas overlay) | S4 |
| Creepage/clearance measurement + DRC (IEC 60950 standards) | G59r |
| Keyboard shortcut customization | G37 |
| Workspace layout save/restore | G38 |
| Preferences import/export | G76 |

#### v2.4+: Future (P3 — nice-to-have)

| Feature | Gap ID |
|---|---|
| Auto-router (topological) | G59 |
| ActiveRoute (semi-automatic routing assist) | G60 |
| Copper balancing / thieving | G62, G63 |
| Backdrilling | G61 |
| Rooms (from multi-channel, per-room rules) | G66 |
| PCB-level annotation (by board position) | G7 |
| Unified component model (sym+fp+3D+sim linked) | G35 |
| Lifecycle management (active/EOL/obsolete) | G36 |
| Part choices (multi-supplier, BOM Studio) | G70 |
| Where-used analysis | G71 |
| Component risk dashboard (Pro) | S2 |
| Manufacturing rules (acid trap, slivers) | G73 |
| Net antennae detection | G75 |
| Max via count rule | G84 |
| Fanout control rule | G85 |
| Antenna simulation (pattern, gain) | G60f |
| Register map import (SVD/IP-XACT) | S20 |
| Rigid-flex board | G86 |
| Embedded components (cavity) | G88 |
| OrCAD/PADS/Mentor import | G90 |
| 3D clearance checking (body-to-body) | G29 |
| Device sheets (reusable schematic blocks) | G79 |
| Thermal-aware routing (AI) | S7 |
| Placement heatmap (connectivity density) | S12 |
| Cross-probe to simulation (click net → waveform) | S10 |
| Smart snap points (impedance-optimal widths) | S11 |
| Collaborative freehand annotations (Pro) | S13 |
| Interactive impedance calculator on canvas | S6 |
| Data-bound schematic tables (auto-update from component) | S18 |
| Board shape from STEP (extract from enclosure) | G95 |
| Enclosure fit checking (import STEP, check clearance) | G96 |

---

## Phase 0: Application Scaffold — v0.1.0

**Branch:** `feature/v0.1-scaffold`
**Goal:** Empty Iced window with Altium-style shell layout — menu bar, toolbar, dockable panels, status bar, tabbed document area, 6 themes.

### Implementation

#### 0.1 — Workspace setup
- Create `Cargo.toml` workspace manifest
- Create `crates/signex-app/` and `crates/signex-types/`
- Set up workspace dependencies and CI

#### 0.2 — Iced application shell
- `main.rs`: window setup (title "Signex", size 1400x900, dark theme)
- `app.rs`: Iced `Application` trait — `Message` enum, `update()`, `view()`
- Basic layout: `Column` [ menu_bar, toolbar, `Row` [ left_panel, center, right_panel ], bottom_panel, status_bar ]

#### 0.3 — Theme system
- Define all 6 themes in `crates/signex-types/src/theme.rs`
- `ThemeTokens` struct with all color tokens (bg, text, accent, canvas colors)
- Iced `Theme` implementation that maps tokens to widget styles
- Theme switching via menu

#### 0.4 — Panel docking system (CRITICAL — custom infrastructure)

Iced has NO built-in tabbed docking. Build a custom `DockArea` widget by combining:
- `iced::widget::pane_grid` — splits the window into left/center/right/bottom regions
- `iced_aw::Tabs` — tabbed panel groups within each dock region
- Custom `CollapsedRail` widget — vertical icon strip when dock is collapsed

Implementation:
- `dock/dock_area.rs` — top-level layout: `PaneGrid` with 4 fixed panes (left, center, right, bottom)
- `dock/panel_tab.rs` — wraps `iced_aw::Tabs` with tab bar styling matching Altium (10px uppercase, semibold)
- `dock/panel_rail.rs` — collapsed state: vertical `Column` of rotated text labels + icons
- Panel registry with context awareness (schematic vs PCB panels filtered)
- Persist dock sizes and collapsed state to settings file
- No floating panels in v0.1.0 (defer to later phase)

#### 0.4b — Tree view widget (custom)

Iced has NO built-in tree view. Build a `TreeView` widget for Projects panel and component browser:
- `Column` + `Row` with indentation per depth level
- Expand/collapse state per node (triangle icon)
- Click to select, double-click to open/navigate
- Keyboard navigation (up/down/left=collapse/right=expand)
- Used by: Projects panel (sheet hierarchy), Components panel (library categories), Navigator panel

#### 0.5 — Menu bar
- Custom menu bar widget (Iced `Row` + dropdown buttons)
- File, Edit, View, Place, Design, Tools, Window, Help menus
- Keyboard shortcut labels shown in menu items
- Separator support

#### 0.6 — Toolbar strip
- Icon buttons for common actions (select, wire, bus, label, component, etc.)
- Active tool highlight
- Tooltip on hover

#### 0.7 — Document tab bar
- Tabs for open documents (schematic sheets, PCB)
- Close button per tab
- Active tab highlight
- Right-click context menu (close, close others, close all)

#### 0.8 — Status bar
- Left: cursor position (X/Y with unit conversion)
- Grid size indicator (clickable to toggle visibility)
- Snap indicator
- Electrical snap indicator
- Active layer (PCB mode only)
- Current mode
- Right: zoom percentage, units (clickable to cycle mm/mil/inch), panels button

### Dependencies (v0.1.0)

```toml
[workspace.dependencies]
iced            = { version = "0.14", features = ["wgpu", "canvas", "advanced", "multi-window"] }
iced_aw         = { version = "0.13", default-features = false, features = ["tabs", "card", "modal", "split"] }
serde           = { version = "1", features = ["derive"] }
serde_json      = "1"
uuid            = { version = "1", features = ["v4", "serde"] }
thiserror       = "2"
anyhow          = "1"
```

### Test Plan (v0.1.0)
- [ ] `cargo build --workspace` compiles without errors
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] Application launches, window appears with correct title
- [ ] Menu bar renders with File/Edit/View/Place/Design/Tools menus
- [ ] Left panel shows "Projects" and "Components" tabs
- [ ] Right panel shows "Properties" tab
- [ ] Bottom panel shows "Messages" and "Signal" tabs
- [ ] Clicking collapsed rail expands the dock
- [ ] Status bar shows cursor position, grid, snap, zoom, units
- [ ] All 6 themes switch correctly: Catppuccin Mocha, VS Code Dark, GitHub Dark, Altium Dark, Solarized Light, Nord
- [ ] Units cycle mm → mil → inch via Ctrl+Q
- [ ] Window resizes correctly, panels maintain proportions
- [ ] Panel tabs switch on click

---

## Phase 1: KiCad Parser — v0.2.0

**Branch:** `feature/v0.2-parser`
**Goal:** Build the S-expression parser in `crates/kicad-parser/`.

### Implementation

#### 1.1 — S-expression tokenizer
- Build `sexpr.rs` — tokenizer + tree builder
- Unit tests for nested S-expressions, quoted strings, escape sequences

#### 1.2 — Schematic parser
- Build `parser.rs` — `.kicad_sch` → `SchematicSheet`
- All types in `signex-types`: Symbol, Wire, Label, Junction, Bus, BusEntry, NoConnect, TextNote, Drawing, ChildSheet

#### 1.3 — PCB parser
- Build `pcb_parser.rs` — `.kicad_pcb` → `PcbBoard`
- All PCB types in `signex-types`: Track, Pad, Via, Zone, Footprint, BoardOutline

#### 1.4 — Symbol library parser
- `.kicad_sym` → `SymbolLibrary`
- Graphics: Polyline, Rectangle, Circle, Arc, Text, TextBox
- Pins: name, number, position, orientation, electrical type
- Unit + bodyStyle filtering metadata

#### 1.5 — Writer (save)
- Build `writer.rs` — `SchematicSheet` → S-expression string
- Round-trip test: parse → write → parse → assert equal

#### 1.6 — Rich text markup parser
- `parse_markup("V_{CC}")` → `[Normal("V"), Sub("CC")]`
- Support `_{}` (subscript), `^{}` (superscript), `~{}` (overbar)
- Unit tests for all segment types + combinations

### Dependencies (added)

```toml
nom = "8"    # Parser combinator for S-expressions
```

### Test Plan (v0.2.0)
- [ ] `cargo test -p kicad-parser` — all parser tests pass
- [ ] `cargo test -p signex-types` — markup parser tests pass
- [ ] Parse `tests/fixtures/simple.kicad_sch` — symbols, wires, labels extracted
- [ ] Parse `tests/fixtures/multi-sheet.kicad_sch` — child sheets found
- [ ] Parse `tests/fixtures/board.kicad_pcb` — tracks, pads, vias extracted
- [ ] Parse `tests/fixtures/library.kicad_sym` — symbols + pins extracted
- [ ] Round-trip: parse → write → parse produces identical data
- [ ] Markup: `V_{CC}` → Normal("V") + Sub("CC")
- [ ] Markup: `~{CS}` → Overbar("CS")
- [ ] Markup: `V^{+}` → Normal("V") + Sup("+")
- [ ] Markup: `~{WR}_{0}` → Overbar("WR") + Sub("0")
- [ ] KiCad 8, 9, 10 format files all parse without error

---

## Phase 2: wgpu Canvas — v0.3.0

**Branch:** `feature/v0.3-canvas`
**Goal:** Working `iced::widget::Canvas` with Altium-style pan/zoom/grid in the center area.

### Implementation

#### 2.1 — Canvas widget
- Implement `iced::widget::canvas::Program` for schematic canvas
- `Frame`-based drawing (paths, strokes, fills, text)
- Canvas fills the center area between docked panels
- Three-layer cache system (from Circe pattern):
  - `background_cache` — grid, sheet border, title block (cleared only on theme/grid change)
  - `content_cache` — all schematic elements (cleared on document change, edit, undo/redo)
  - `overlay_cache` — selection highlights, cursor, wire-in-progress (cleared every frame)
- Note: PCB rendering will use `widget::Shader` (Phase 8), not Canvas

#### 2.2 — Camera system (Altium-style)
- Right-click + drag = pan (NOT left-click)
- Middle-click + drag = pan (alternative)
- Scroll wheel = zoom centered on cursor position
- Zoom range: 0.01x to 100x
- Home key = fit all / center view
- Smooth zoom with cursor-centered scaling

#### 2.3 — Grid renderer
- Dot grid (default) with major grid lines
- Grid visibility toggle (Shift+Ctrl+G)
- Grid size cycling: 0.635, 1.27, 2.54, 5.08, 10.16mm (G / Shift+G)
- Grid fades out when zoomed out past threshold
- Colors from active theme

#### 2.4 — Coordinate system
- World coordinates in mm (KiCad internal units)
- Screen-to-world and world-to-screen transforms
- Cursor position tracking → status bar update
- Snap-to-grid logic

#### 2.5 — Basic input routing
- Iced handles clicks on panels, menus, toolbars
- Canvas handles clicks inside the center area
- Keyboard shortcuts routed to canvas when focused
- Right-click on canvas = pan (not context menu, unless no drag)

### Test Plan (v0.3.0)
- [ ] Canvas renders in center area, fills available space
- [ ] Right-click + drag pans the view (Altium behavior)
- [ ] Middle-click + drag also pans
- [ ] Left-click does NOT pan (reserved for selection)
- [ ] Scroll wheel zooms centered on cursor
- [ ] Grid dots/lines appear at correct world positions
- [ ] Grid size cycles with G key (0.635 → 1.27 → 2.54 → 5.08 → 10.16 → 0.635)
- [ ] Shift+Ctrl+G toggles grid visibility
- [ ] Home key fits view to paper/board bounds
- [ ] Status bar cursor position updates as mouse moves
- [ ] Status bar shows correct units (mm/mil/inch)
- [ ] Cursor snaps to grid when snap is enabled
- [ ] 60fps maintained during pan/zoom

---

## Phase 3: Schematic Viewer — v0.4.0

**Branch:** `feature/v0.4-schematic-viewer`
**Goal:** Open a KiCad `.kicad_sch` file and render it correctly on the wgpu canvas.

### Implementation

#### 3.1 — File open dialog
- Menu: File → Open Project (native file dialog via rfd)
- Parse `.kicad_pro` → extract schematic root + sheet list
- Open first sheet, populate Projects panel tree

#### 3.2 — Schematic render pipeline
Implement render logic referencing KiCad `SCH_PAINTER` for correctness.

**Render order (Z-layers, matching KiCad):**
1. Sheet border + title block (z=0)
2. Drawing objects: lines, rectangles, circles, arcs, polylines (z=1)
3. Wires (z=2)
4. Buses (z=3)
5. Bus entries (z=4)
6. Junctions — filled circles (z=5)
7. No-connect markers — X shape (z=6)
8. Net labels — 4 shape types (z=7)
9. Global/hierarchical labels — geometric shapes per type (z=8)
10. Power ports — symbol shapes (z=9)
11. Symbol bodies — polylines, rectangles, circles, arcs, fills (z=10)
12. Pins — line + marker + name/number labels (z=11)
13. Text annotations (z=12)

#### 3.3 — Symbol rendering
- LibSymbol graphics → Path primitives
- Unit + bodyStyle filtering (only render correct unit/style)
- Symbol transform: 2x2 matrix + translation (mirror, rotate)
- Fill types: none, outline, background

#### 3.4 — Rich label rendering
- ALL text goes through `render_rich_label()`
- Subscript (`V_{CC}`) — smaller text shifted down
- Superscript (`V^{+}`) — smaller text shifted up
- Overbar (`~{CS}`) — text with line above
- Combinations (`~{WR}_{0}`)

#### 3.5 — Pin rendering
- Pin line from symbol to endpoint
- Pin markers per electrical type (input arrow, output arrow, etc.)
- Pin name label (outside) + pin number label (inside)
- Show/hide pin names/numbers per symbol setting

#### 3.6 — Title block
- Sheet border (A4, A3, A2, A1 sizes)
- Title, revision, date, company fields
- Positioned per paper size

#### 3.7 — Multi-sheet navigation
- Projects panel shows sheet hierarchy tree
- Click sheet name to load and render that sheet
- Ctrl+double-click on sheet symbol to navigate into child sheet
- Back navigation (parent sheet)

#### 3.8 — Theme-aware colors
- All render colors from active theme's canvas tokens
- Theme switch re-renders immediately

### Dependencies (added)

```toml
rfd = "0.17"    # Native file dialogs
```

### Test Plan (v0.4.0)
- [ ] File → Open Project loads a KiCad project
- [ ] Projects panel shows sheet hierarchy
- [ ] Schematic renders on canvas: wires, symbols, labels, junctions visible
- [ ] Symbol graphics match KiCad rendering (polylines, rectangles, circles, arcs)
- [ ] Symbol mirrors/rotations render correctly (all 8 orientations)
- [ ] Multi-unit symbols show correct unit only
- [ ] Pin names and numbers render at correct positions
- [ ] Rich labels: `V_{CC}` shows subscript, `~{CS}` shows overbar
- [ ] Net labels render with correct shapes
- [ ] Global labels render with correct geometric shapes (input/output/bidir)
- [ ] Power ports render with correct symbols
- [ ] Junctions render as filled circles
- [ ] No-connects render as X markers
- [ ] Title block renders with correct fields
- [ ] Multi-sheet navigation works (click Projects tree)
- [ ] Ctrl+double-click on sheet symbol navigates to child
- [ ] All 6 themes render correctly
- [ ] Pan/zoom works over rendered schematic
- [ ] Large schematic (100+ components) renders at 60fps

---

## Phase 4: Editor Foundation — v0.5.0

**Branch:** `feature/v0.5-schematic-editor`
**Goal:** Selection, move, wire drawing, delete, rotate, mirror, undo/redo, save.

### Implementation

#### 4.1 — Selection system
- Left-click to select (single object)
- Shift+click to toggle selection
- Box select: left-drag on empty area (crossing vs enclosing by drag direction)
- Selection highlight: modified stroke/fill colors (no entity re-creation)
- Selection filter: per-type toggle (wires, symbols, labels, etc.)
- Ctrl+A select all

#### 4.2 — Move with rubber-banding
- Drag selected objects
- Connected wires stretch (rubber-band)
- Ctrl+drag = stiff move (no rubber-band)
- Ctrl+Arrow = nudge by 1 grid, Shift+Ctrl+Arrow = nudge by 10 grid
- Snap to grid during move

#### 4.3 — Wire drawing
- W key activates wire mode
- Click to place wire vertices
- Three routing modes (cycle with Shift+Space): Manhattan, diagonal, free
- Auto-junction when crossing existing wire
- Right-click or Escape to finish wire
- Backspace to remove last placed point

#### 4.4 — Delete
- Delete key removes selected objects
- Connected wire cleanup (remove dangling segments)

#### 4.5 — Rotate and mirror
- Space / R = rotate 90 CCW
- X = mirror X-axis (horizontal flip)
- Y = mirror Y-axis (vertical flip)
- Works during placement and for selected objects

#### 4.6 — Undo/redo
- Command-based undo (not full-state snapshot)
- Ctrl+Z = undo, Ctrl+Y / Ctrl+Shift+Z = redo
- 50-level deep undo stack

#### 4.7 — Save
- Ctrl+S saves current schematic
- Uses kicad-writer to serialize back to `.kicad_sch`
- Dirty flag tracking (asterisk on tab title)

#### 4.8 — Properties panel
- Context-aware based on selection
- Symbol: reference, value, footprint, unit, fields
- Wire: (no editable properties, just visual)
- Label: text, shape, rotation
- No selection: document properties (grid, page, template)
- F11 toggles Properties panel

### Test Plan (v0.5.0)
- [ ] Left-click selects single object, highlight appears
- [ ] Shift+click toggles object in/out of selection
- [ ] Box select works (left-drag on empty area)
- [ ] Crossing selection includes partially enclosed objects
- [ ] Ctrl+A selects all objects
- [ ] Drag moves selected objects, snaps to grid
- [ ] Connected wires rubber-band during move
- [ ] Ctrl+drag disables rubber-band (stiff move)
- [ ] Ctrl+Arrow nudges by 1 grid
- [ ] Shift+Ctrl+Arrow nudges by 10 grid
- [ ] W key activates wire drawing mode
- [ ] Click places wire vertices, right-click/Esc finishes
- [ ] Shift+Space cycles wire routing mode (Manhattan/diagonal/free)
- [ ] Auto-junction appears when wire crosses existing wire
- [ ] Delete removes selected objects
- [ ] Space rotates 90 CCW (placement and selected)
- [ ] X mirrors horizontally, Y mirrors vertically
- [ ] Ctrl+Z undoes, Ctrl+Y redoes (50 levels)
- [ ] Ctrl+S saves, file is valid KiCad format
- [ ] Properties panel shows context-aware fields
- [ ] F11 toggles Properties panel visibility
- [ ] Double-click opens properties for clicked object

---

## Phase 5: Core Editing — v0.6.0

**Branch:** `feature/v0.6-full-editor`
**Goal:** Full schematic editing with copy/paste, labels, power ports, bus, in-place text edit.

### Implementation

#### 5.1 — Copy/Cut/Paste
- Ctrl+C/X/V = copy/cut/paste
- Shift+Ctrl+V = smart paste (transform, arrays)
- Ctrl+D = duplicate
- Clipboard uses internal format (serde)
- Paste at cursor position

#### 5.2 — Label placement
- L key = place net label
- Net label, global label, hierarchical label, power port placement
- Tab key pauses placement, opens Properties to edit before placing
- Auto-increment on label suffixes

#### 5.3 — Bus drawing and entries
- B key = draw bus
- Bus entries (diagonal connectors)
- Bus naming: `Data[0..7]`

#### 5.4 — In-place text editing
- F2 or click-pause-click on text
- Inline text editing overlay
- Enter confirms, Escape cancels

#### 5.5 — Component placement
- P key opens component search
- Component browser (226 KiCad libraries)
- Search with wildcards
- Click to place, auto-designator increment
- Tab to edit properties during placement
- Space to rotate during placement

#### 5.6 — Context menu
- Right-click (no drag) shows context-sensitive menu
- Menu items depend on what's under cursor and selection
- Cut, Copy, Paste, Delete, Properties, Select All
- Rotate, Mirror X, Mirror Y
- Zoom to Fit, Zoom to Selection

#### 5.7 — Find/Replace
- Ctrl+F opens Find dialog
- Ctrl+H opens Find and Replace
- Regex support
- Navigate between matches

#### 5.8 — Selection memory
- Ctrl+1-8 stores current selection to slot
- Alt+1-8 recalls selection from slot

### Test Plan (v0.6.0)
- [ ] Ctrl+C copies, Ctrl+V pastes at cursor
- [ ] Ctrl+X cuts (copies then deletes)
- [ ] Ctrl+D duplicates selected objects
- [ ] Smart paste (Shift+Ctrl+V) transforms types
- [ ] L places net labels, auto-increments suffixes
- [ ] B draws buses, bus entries connect correctly
- [ ] P opens component search, placing works
- [ ] Tab pauses placement, Properties panel opens
- [ ] F2 activates in-place text editing
- [ ] Enter confirms edit, Escape cancels
- [ ] Right-click shows context-sensitive menu
- [ ] Ctrl+F opens Find, Ctrl+H opens Find/Replace
- [ ] Ctrl+1 stores selection, Alt+1 recalls it
- [ ] All shortcuts match Altium behavior

---

## Phase 6: Validation — v0.7.0

**Branch:** `feature/v0.7-validation`
**Goal:** ERC with 11 checks, pin connection matrix, annotation, net color override, AutoFocus.

### Implementation

#### 6.1 — ERC engine
Implement ERC rules:
1. Duplicate designator
2. Unconnected pin
3. Floating wire
4. No driver on net
5. Single pin net
6. Output-to-output conflict
7. Multiple net names
8. Unannotated component
9. Pin conflict (12x12 matrix)
10. Power pin not driven
11. Net without label

#### 6.2 — Messages panel
- ERC violations list with severity icons (error/warning)
- Click violation → zoom-to and highlight source on canvas
- Double-click → open properties for violated object
- Configurable severity per violation type

#### 6.3 — Pin connection matrix dialog
- 12x12 matrix (output, input, bidirectional, passive, etc.)
- Configurable per cell: error, warning, no report
- Default matrix matches Altium/KiCad defaults

#### 6.4 — Annotation
- 4 annotation modes (schematic level, PCB level, board level, single sheet)
- Processing order: up/down/across combinations
- Auto-increment designators
- Lock/unlock individual designators
- Preview before applying

#### 6.5 — Net color override
- F5 toggles net color overlay
- Each net gets a distinct color from palette
- Wire + label + junction all colored per net

#### 6.6 — AutoFocus
- When hovering/selecting an object, dim unrelated elements
- Shows electrical connectivity context
- Configurable dim level

### Test Plan (v0.7.0)
- [ ] Design → Run ERC finds violations in test schematic
- [ ] All 11 ERC types detected correctly
- [ ] Messages panel lists violations with correct severity
- [ ] Click violation in Messages → canvas zooms to source
- [ ] Pin connection matrix dialog opens, editable
- [ ] Design → Annotate assigns designators in correct order
- [ ] Annotation preview shows proposed changes before applying
- [ ] F5 toggles net color overlay
- [ ] Each net gets distinct color
- [ ] AutoFocus dims unrelated objects when hovering

---

## Phase 7: Advanced Schematic — v0.8.0

**Branch:** `feature/v0.8-output`
**Goal:** Library editor, PDF/print, output jobs, templates, BOM, drawing tools, selection filter, 40+ Altium parity features.

### Implementation

#### 7.1 — Symbol library editor
- Create/edit KiCad symbols
- Draw graphics: polyline, rectangle, circle, arc
- Place pins with type/name/number
- Save to `.kicad_sym`

#### 7.2 — Footprint library editor
- Create/edit footprints
- Place pads, graphics
- Save to `.kicad_mod`

#### 7.3 — PDF/Print export
- Single and multi-sheet PDF
- Configurable DPI, color mode
- Print via system dialog

#### 7.4 — Output jobs
- BOM export: CSV, TSV, HTML, Excel
- Netlist export: KiCad S-expression, generic XML
- Gerber export preparation
- Job queue with status

#### 7.5 — Templates and title block
- Sheet templates (ISO A4, ANSI A, etc.)
- Custom title block editing
- Text string substitution (`=Title`, `=Date`, `=Rev`)

#### 7.6 — Drawing tools
- Line, rectangle, circle, arc, ellipse, polyline, polygon
- Round rectangle, text frame, note
- Line styles: solid, dash, dot, dash-dot
- Arrow endpoints
- Fill options

#### 7.7 — Selection filter
- Per-type visibility/selectability toggle
- Wires, symbols, labels, junctions, text, drawings, etc.

#### 7.8 — Advanced features
- Net classes (add/remove/assign)
- Differential pairs (`_P`/`_N` naming convention)
- Signal harnesses
- Design constraints
- Design variants (fitted/not-fitted/alternate)
- Parameter Manager (spreadsheet-like editing)
- Multi-channel design (Repeat keyword)
- Groups/Union
- Find Similar Objects (Shift+F)
- Align/distribute (left/right/top/bottom/center H/V)
- Bring to front / send to back

### Test Plan (v0.8.0)
- [ ] Symbol library editor: create symbol, draw graphics, place pins, save
- [ ] Footprint editor: create footprint, place pads, save
- [ ] File → Export PDF produces valid PDF with schematic
- [ ] BOM export generates CSV with all components
- [ ] Netlist export matches KiCad format
- [ ] Templates apply correctly (ISO A4, ANSI A)
- [ ] All drawing tools work (line, rect, circle, arc, polyline)
- [ ] Line styles render correctly (dash, dot, etc.)
- [ ] Selection filter toggles work per type
- [ ] Net classes assignable to nets
- [ ] Differential pairs detected from `_P`/`_N` naming
- [ ] Parameter Manager opens with spreadsheet view
- [ ] Shift+F opens Find Similar Objects
- [ ] Align/distribute works for multiple selected objects
- [ ] Variants toggle fitted/not-fitted per component

---

## Phase 8: PCB Viewer — v0.9.0

**Branch:** `feature/v0.9-pcb-viewer`
**Goal:** Render KiCad `.kicad_pcb` files on the wgpu canvas with layer compositing.

### Implementation

#### 8.1 — PCB canvas setup (uses `widget::Shader`, NOT `Canvas`)
- PCB rendering uses `iced::widget::Shader` with custom WGSL pipelines — NOT `Canvas`
- `widget::Shader` gives raw wgpu access: `wgpu::Device`, `wgpu::Queue`, `wgpu::RenderPass`
- Custom instanced rendering pipelines:
  - `track.wgsl` — instanced track segments (start, end, width, color per instance)
  - `pad.wgsl` — instanced pads (center, size, shape, color per instance)
  - `via.wgsl` — instanced vias (center, drill, annular ring per instance)
  - `zone.wgsl` — zone fill polygons (pre-tessellated meshes)
- Layer compositing order (KiCad): B.Cu → ... → F.Cu (32 copper), paste, mask, silk, courtyard, fab, edge cuts
- Layer colors from Altium defaults (see reference above)
- This is critical for 100K+ element performance — Canvas (lyon CPU tessellation) would be too slow

#### 8.2 — PCB element rendering
- Tracks (lines with width, per-layer color)
- Pads (rect, circle, oval, roundrect, custom shapes)
- Vias (through, blind, buried — circle + drill hole)
- Footprints (graphics + pads + reference/value text)
- Zones (filled copper polygons)
- Board outline (Edge.Cuts)
- Silkscreen text/graphics

#### 8.3 — Layer visibility controls
- Layer stack panel (right dock)
- Toggle layer visibility
- Toggle layer selectability
- Layer color editor
- Single-layer mode (dim all others)

#### 8.4 — View modes
- Standard: all visible layers composited
- Single layer: only active layer at full opacity, others dimmed
- Board flip: Ctrl+F flips view (bottom-up perspective)

#### 8.5 — Cross-probing
- Select component in schematic → highlights on PCB
- Select component in PCB → highlights on schematic
- Bidirectional zoom-to on cross-probe

#### 8.6 — Ratsnest
- MST + UnionFind connectivity
- Thin lines showing unrouted connections
- Toggle visibility

### Test Plan (v0.9.0)
- [ ] File → Open loads `.kicad_pcb` and renders on canvas
- [ ] Tracks render with correct width and layer color
- [ ] Pads render with correct shapes (rect, circle, roundrect)
- [ ] Vias render with drill hole
- [ ] Footprints render with graphics and pads
- [ ] Zones render as filled polygons
- [ ] Board outline renders
- [ ] Layer stack panel shows all layers with colors
- [ ] Toggle layer visibility hides/shows layer content
- [ ] Single-layer mode dims all except active layer
- [ ] Ctrl+F flips board view
- [ ] Cross-probe: select in SCH → highlights in PCB
- [ ] Ratsnest lines show unrouted connections
- [ ] Altium layer colors match defaults
- [ ] Large PCB (1000+ tracks) renders at 60fps

---

## Phase 9: PCB Editor — v0.10.0

**Branch:** `feature/v0.10-pcb-editor`
**Goal:** Interactive routing, DRC, copper pour, component placement on PCB.

### Implementation

#### 9.1 — Interactive routing
- Walkaround routing mode
- Push/shove routing mode
- 45/90 degree corners, arc corners
- Via placement during routing
- Track width from net class rules

#### 9.2 — Differential pair routing
- Coupled routing with gap control
- Length matching
- Meander patterns for length tuning

#### 9.3 — DRC engine
Implement DRC rules:
15 rule types (clearance, min width, min via, annular ring, etc.)

#### 9.4 — Copper pour
- Zone drawing
- Polygon clipping (Clipper2)
- Thermal relief pads
- Zone fill/unfill

#### 9.5 — Component placement
- Move footprints
- Rotate (any angle)
- Flip to other side
- Push/shove placement

#### 9.6 — Advanced PCB features
- Multi-track (bus) routing
- Via stitching (grid/fence patterns)
- BGA fanout (dog-bone escape)
- Teardrops (pad/via transitions)
- Net color override

#### 9.7 — Export
- Gerber (RS-274X + X2)
- Excellon drill
- ODB++
- Pick-and-place
- IPC-2581

### Test Plan (v0.10.0)
- [ ] Interactive routing places tracks with correct width
- [ ] Walkaround avoids existing copper
- [ ] Push/shove moves existing tracks to make room
- [ ] Via placement during routing works
- [ ] Differential pair routes with correct gap
- [ ] Length tuning meanders at correct target length
- [ ] DRC detects all 15 violation types
- [ ] Zone fill produces correct copper pour
- [ ] Thermal relief appears on pads in zones
- [ ] Component move/rotate/flip works
- [ ] Gerber export produces valid files (view in gerbview)
- [ ] Excellon drill file matches via positions

---

## Phase 10: 3D PCB Viewer — v0.11.0

**Branch:** `feature/v1.1-3d-viewer`
**Goal:** 3D PCB viewer with PBR materials, STEP models, thermal overlay.

### Implementation

#### 10.1 — 3D rendering engine (uses `widget::Shader`)
- `iced::widget::Shader` with custom 3D wgpu pipelines (vertex/fragment shaders)
- Implements `shader::Program` trait with custom `Primitive` for 3D scene
- Full wgpu access: create render pipelines, bind groups, buffers, depth textures
- Orbit camera (right-click orbit, middle-click pan, scroll zoom) — manual in `Program::update`
- Iced's `custom_shader` example (renders 500 3D cubes) is the reference pattern

#### 10.2 — PCB extrusion
- 2D copper geometry → 3D mesh
- Layer stackup: FR4 1.6mm, copper 35/70um, soldermask 25um, silkscreen 10um
- Per-layer z-offset

#### 10.3 — PBR materials
- Copper: metallic=1.0, roughness=0.3, #B87333
- Soldermask: green/red/blue/black, alpha=0.95
- FR4: roughness=0.9, #8B7355
- Silkscreen: white/yellow text on mask

#### 10.4 — STEP model loading
- truck-modeling for STEP import
- Async loading (never block UI)
- Position on footprint anchor point

#### 10.5 — Thermal overlay
- Vertex color mapping from Elmer FEM results
- Temperature color ramp: blue (20C) → green (60C) → red (100C) → white (150C+)
- Toggle overlay on/off

### Test Plan (v0.11.0)
- [ ] 3D view shows extruded PCB with correct layer heights
- [ ] Copper appears metallic, soldermask appears green and translucent
- [ ] FR4 substrate visible between copper layers
- [ ] Orbit camera: right-click orbits, middle-click pans, scroll zooms
- [ ] STEP models load and appear at correct footprint positions
- [ ] Board cross-section view works
- [ ] Thermal overlay colors vertices by temperature (when data available)

---

## Phase 11: Simulation — v0.12.0

**Branch:** `feature/v1.4-simulation`
**Goal:** SPICE (ngspice), RF/EM (OpenEMS), thermal (Elmer FEM) simulation.

### Implementation

#### 11.1 — ngspice bridge (SPICE)
- Schematic → .cir netlist generation
- ngspice FFI via libloading (libngspice.dll/.so/.dylib)
- DC, AC, transient analysis
- .raw parser → WaveformData
- Waveform panel: multi-trace, dual cursor, delta readout, PNG/CSV export

#### 11.2 — OpenEMS bridge (RF/EM FDTD)
- PCB geometry → CSX XML (tracks, vias, zones → conductors)
- Material database (copper, FR4, Rogers, soldermask, prepreg)
- Auto-mesh with conductor refinement (lambda/100 near edges)
- Build auto-detection
- Subprocess execution with progress parsing
- HDF5 → S-parameter matrix
- S-param panel: dB + phase plot, Smith chart

#### 11.3 — Elmer FEM bridge (thermal + DC IR drop)
- PCB geometry → GMSH mesh → Elmer .sif
- Thermal solver: component heat sources, convection boundaries
- DC IR drop: current injection/sink, voltage distribution
- EM: eddy currents, inductor modeling
- VTK reader → field data
- Thermal panel, IR drop overlay, current density vectors

#### 11.4 — Sim job queue
- UUID per job
- Status tracking: queued/meshing/solving/done/error
- Cancel button
- Duration tracking
- Results stored for AI context

### Dependencies (added)

```toml
hdf5         = "0.8"           # OpenEMS HDF5 output
vtkio        = "0.6"           # Elmer VTK output
num-complex  = "0.4"           # Complex S-parameters
libloading   = "0.8"           # ngspice FFI
```

### Test Plan (v0.12.0)
- [ ] Schematic → .cir netlist generates valid SPICE netlist
- [ ] ngspice DC analysis runs and produces waveform data
- [ ] Waveform panel shows voltage/current traces
- [ ] Dual cursor measures delta between points
- [ ] OpenEMS: microstrip 50ohm → S11 < -20dB at design frequency
- [ ] S-param panel shows S11/S21 in dB + Smith chart
- [ ] Elmer: 2-layer thermal sim produces temperature map
- [ ] Thermal overlay renders on 3D PCB model
- [ ] IR drop analysis shows voltage distribution
- [ ] Job queue tracks all running simulations
- [ ] Graceful error if ngspice/OpenEMS/Elmer not installed
- [ ] Sim results available as Signal AI context

---

## Phase 12: Signal AI — v0.13.0 (Pro Only)

**Branch:** `feature/v1.7-signal-ai`
**Edition:** Pro (feature-gated behind `#[cfg(feature = "pro")]`)
**Crate:** `crates/signex-signal/`
**Goal:** Claude-powered design copilot via Alp Lab's managed API gateway. Users don't need their own API key — Signal AI usage is included in the Pro subscription.

### Implementation

#### 12.1 — Alp Lab API gateway client
- reqwest HTTP client → `https://api.alplabai.com/signal/v1/chat`
- Authentication: Pro license key (validated against Alp Lab license server)
- No user-facing API key — all Claude usage routed through Alp Lab's gateway
- Gateway handles: model selection, rate limiting, usage tracking, billing
- Streaming SSE for real-time response display
- Fallback: graceful degradation if gateway unreachable (show offline message, not crash)

#### 12.2 — Signal panel (Pro UI)
- Chat interface in bottom dock (panel hidden in Community edition)
- Markdown rendering in messages (Iced `markdown` widget)
- Streaming response display with typing indicator
- Session history (persisted locally, optionally synced to cloud in collab mode)
- Usage meter: shows monthly included usage vs consumed
- Model indicator: shows which Claude model the gateway selected

#### 12.3 — Tool use
Claude can take actions on the user's design:
- **Simulation:** run_spice_sim, run_openems, run_thermal — triggers sim job queue
- **Query:** show_waveform, show_sparam, show_thermal — opens result panels
- **Edit:** add_component, draw_wire, set_value, delete_element — modifies schematic (undoable)
- **Analysis:** check_erc, check_drc, check_si — runs validation and reports results
- **Review:** design_review — systematic check of entire schematic/PCB with scored report

#### 12.4 — Context injection
Automatically includes relevant design context with each message:
- Component list (reference, value, footprint, fields)
- Net connectivity graph (which pins connect to which nets)
- ERC/DRC violations (current state)
- Simulation results (waveform data, S-params, thermal maps)
- Visual context: schematic/PCB screenshot rendered to PNG and sent as image
- Active selection context: what the user has selected right now

#### 12.5 — Circuit templates
- 6+ pre-built templates: buck converter, LDO, op-amp inverting/non-inverting, LC filter, diff pair termination
- "Place a buck converter for 12V→3.3V at 2A" → Signal generates and places the circuit
- Template insertion at cursor position with auto-wiring

#### 12.6 — Feature gate in Community edition
- Signal panel tab hidden in Community build
- Menu item "Signal AI" shows upgrade prompt: "Signal AI is available in Signex Pro"
- No AI code compiled into Community binary (`#[cfg(feature = "pro")]`)

### Dependencies (Pro only)

```toml
# crates/signex-signal/Cargo.toml
[dependencies]
reqwest         = { workspace = true }
tokio           = { workspace = true }
serde           = { workspace = true }
serde_json      = { workspace = true }
signex-types    = { path = "../signex-types" }
```

### Test Plan (v0.13.0)
- [ ] Community build: Signal panel tab not visible, menu shows upgrade prompt
- [ ] Pro build: Signal panel opens in bottom dock
- [ ] License key validates against Alp Lab gateway
- [ ] Chat message sends and receives streaming response via gateway
- [ ] Markdown renders in chat messages
- [ ] Tool use: "run a DC analysis" triggers SPICE simulation
- [ ] Tool use: "add a 10k resistor" places component on schematic
- [ ] Design review: Claude identifies issues and scores the design
- [ ] ERC fix: Claude suggests and optionally auto-applies fixes
- [ ] Usage meter shows correct monthly consumption
- [ ] Offline: graceful message when gateway unreachable
- [ ] Visual context: screenshot is included in message and Claude references it

---

## Phase 13: Plugin System — v0.14.0

**Branch:** `feature/v1.8-plugins`
**Goal:** Extism WASM plugin API for third-party extensions.

### Implementation

#### 13.1 — WASM host
- Extism runtime integration
- Plugin loading from `.wasm` files

#### 13.2 — Host function categories
- **Document:** get_schematic, get_pcb, get_netlist
- **Mutation:** add/delete/move/route
- **UI:** toast, panel, menu, toolbar
- **Query:** run_erc, run_drc, query_entities
- **Sim:** run_spice, get_waveform, run_thermal, get_thermal_map

#### 13.3 — Permission gateway
- Plugin manifest declares required permissions
- User approves/denies per plugin
- Undo stack integration for mutations

### Dependencies (added)

```toml
extism = "1.21"    # WASM plugin framework
```

### Test Plan (v0.14.0)
- [ ] WASM plugin loads without crash
- [ ] Plugin calls get_schematic and receives data
- [ ] Plugin calls add mutation and schematic updates
- [ ] UI toast from plugin appears
- [ ] Permission dialog appears for restricted operations
- [ ] Plugin errors don't crash the application

---

## Phase 14: Polish — v1.0.0 (Community Release)

**Branch:** `feature/v2.0-pro`
**Goal:** Production quality Community edition — performance, stability, UX polish.

### Implementation

- Performance profiling and optimization
- Memory usage optimization
- Large schematic stress test (1000+ components)
- Large PCB stress test (100K+ tracks)
- Accessibility review
- Native file associations (.snxsch, .snxpcb, .snxprj)
- Auto-update mechanism
- Installer for Windows/macOS/Linux
- Documentation
- Community edition branding and about dialog

### Test Plan (v1.0.0)
- [ ] `cargo build --workspace --release` succeeds (Community)
- [ ] `cargo build --workspace --release --features pro` succeeds (Pro)
- [ ] `cargo test --workspace` all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` zero warnings
- [ ] Open .kicad_pro → schematic renders correctly
- [ ] PCB edit → DRC 15 rules pass
- [ ] SPICE sim → waveform visible
- [ ] OpenEMS → S-param plot visible
- [ ] Elmer → 3D thermal overlay visible
- [ ] Community build: Signal AI panel hidden, upgrade prompt works
- [ ] Pro build: Signal AI → tool use works
- [ ] WASM plugin load → toast visible
- [ ] 100+ component schematic at 60fps
- [ ] 100K track PCB at 60fps
- [ ] All 6 themes working
- [ ] All Altium keyboard shortcuts working
- [ ] Binary size < 30MB (Community), < 35MB (Pro)
- [ ] Startup time < 2 seconds

---

## Phase 15: Live Collaboration — v1.1.0 (Pro Only)

**Branch:** `feature/v2.1-collaboration`
**Edition:** Pro (feature-gated behind `#[cfg(feature = "pro")]`)
**Crate:** `crates/signex-collab/` (client) + `crates/signex-collab-server/` (Edge Functions)
**Backend:** Supabase (Postgres + Realtime + Storage + Auth + Edge Functions)
**Goal:** Real-time co-editing for schematic and PCB, with presence awareness, locking, version history, and review workflows. Comparable to Altium 365 but built for real-time co-design rather than sequential handoff.

### Implementation

#### 15.1 — Supabase infrastructure

Supabase provides the entire backend stack — no custom server to build or deploy:

| Supabase Service | Use Case |
|---|---|
| **Realtime** (WebSocket channels) | Live edit sync, cursor broadcast, presence, activity feed |
| **Postgres** (database) | Project metadata, version history, comments, user profiles, review states |
| **Storage** (S3-compatible) | Project files (.snxsch, .snxpcb, .kicad_sch, .kicad_pcb, STEP models) |
| **Auth** (GoTrue) | User accounts, JWT tokens, team management, Pro license validation |
| **Edge Functions** (Deno) | CRDT merge logic, conflict resolution, DRC webhook, notification dispatch |
| **Row Level Security** | Per-project access control (owner/editor/viewer) |

**Client transport:**
- `signex-collab` connects to Supabase Realtime via WebSocket
- Channels: `project:{id}:edits` (ops), `project:{id}:cursors` (presence), `project:{id}:chat` (comments)
- Auto-reconnect with exponential backoff (built into Supabase client)
- Presence API tracks online users per project (built into Supabase Realtime)

#### 15.2 — CRDT document model
- Conflict-free replicated data types for concurrent edits
- Each schematic/PCB element identified by (UUID, author, lamport_clock)
- Operations: Insert, Delete, Update (property change), Move (position change)
- Supabase Edge Function acts as central merge point — receives ops, applies in causal order, broadcasts via Realtime channel
- Offline support: queue local ops in SQLite, replay on reconnect
- Postgres stores the canonical merged state + full operation log

#### 15.3 — Schematic co-editing
- Real-time sync of all edit operations (wire, symbol, label, junction, etc.)
- Per-user colored cursors rendered on canvas (username label + cursor icon)
- Sheet-level locking (optional): engineer takes exclusive lock on a sheet
- Property conflict resolution: last-write-wins with attribution
- Geometry conflict: spatial partitioning — edits to the same wire/symbol by two users show conflict marker

#### 15.4 — PCB co-editing
- Real-time sync of routing, placement, zone edits
- Region locking: select a rectangular area → lock for exclusive editing
- Layer locking: claim a copper layer for exclusive routing
- Net assignment: assign specific nets to engineers ("you route the DDR bus, I'll do power")
- Live DRC: violations update in real-time as any collaborator changes the board
- Conflict: two people routing the same net → second person sees "net claimed by [name]"

#### 15.5 — Presence and awareness
- Online status panel: shows who's in the project (avatar, name, active sheet/layer)
- Follow mode: click a collaborator's avatar → your viewport follows theirs
- Activity feed: "Alice placed U3 on Sheet2", "Bob routed VCC on F.Cu"
- Notification on important events: DRC violation introduced, ERC error, component deleted

#### 15.6 — Project workspace (Supabase)
- Create/join project workspaces via Supabase Auth (email/password, GitHub OAuth, Google OAuth)
- Role-based access via Postgres RLS: Owner, Editor, Viewer
- Project files stored in Supabase Storage (S3-compatible) with local cache
- Open project from workspace browser or shared URL
- Team management: invite by email, manage roles per project

#### 15.7 — Version history and review
- Full edit history stored in Postgres `edit_log` table with per-change attribution
- Browse history timeline, diff between any two versions
- Review workflow:
  - Request review: mark changes as "ready for review" (Postgres state machine)
  - Reviewer sees diff overlay on canvas (green=added, red=removed, yellow=modified)
  - Approve / request changes / reject
  - Merge reviewed changes into main branch
- Branching: create design branches (like git) for experimental changes
- Supabase Edge Function triggers: notify reviewers on review request, notify author on approval

#### 15.8 — Comments and annotations
- Pin comments to specific locations on schematic/PCB canvas
- Stored in Postgres `comments` table with (project_id, sheet_id, x, y, thread_id)
- Comment threads with replies
- @mention collaborators (Supabase Auth user lookup)
- Resolve/unresolve comments
- Comments visible as markers on canvas (click to expand)
- Real-time: new comments broadcast via Supabase Realtime subscription

#### 15.9 — Feature gate in Community edition
- Collaboration panel/menu hidden in Community build
- "Share" button shows upgrade prompt
- No collab code compiled into Community binary

### Supabase Schema

```sql
-- Projects
create table projects (
  id uuid primary key default gen_random_uuid(),
  name text not null,
  owner_id uuid references auth.users(id),
  created_at timestamptz default now(),
  updated_at timestamptz default now()
);

-- Project members (RBAC)
create table project_members (
  project_id uuid references projects(id) on delete cascade,
  user_id uuid references auth.users(id) on delete cascade,
  role text check (role in ('owner', 'editor', 'viewer')),
  invited_at timestamptz default now(),
  primary key (project_id, user_id)
);

-- Edit operation log (CRDT ops)
create table edit_log (
  id bigint generated always as identity primary key,
  project_id uuid references projects(id) on delete cascade,
  user_id uuid references auth.users(id),
  sheet_id text not null,           -- which sheet/PCB file
  op_type text not null,            -- 'insert', 'delete', 'update', 'move'
  element_id uuid not null,         -- target element UUID
  payload jsonb not null,           -- operation data
  lamport_clock bigint not null,
  created_at timestamptz default now()
);

-- Comments pinned to canvas locations
create table comments (
  id uuid primary key default gen_random_uuid(),
  project_id uuid references projects(id) on delete cascade,
  user_id uuid references auth.users(id),
  sheet_id text not null,
  x float8 not null, y float8 not null,
  thread_id uuid,                   -- null = root, non-null = reply
  body text not null,
  resolved boolean default false,
  created_at timestamptz default now()
);

-- Locks (sheet, region, layer, net)
create table locks (
  id uuid primary key default gen_random_uuid(),
  project_id uuid references projects(id) on delete cascade,
  user_id uuid references auth.users(id),
  lock_type text check (lock_type in ('sheet', 'region', 'layer', 'net')),
  target jsonb not null,            -- e.g. {"sheet": "root.kicad_sch"} or {"layer": "F.Cu"}
  acquired_at timestamptz default now(),
  expires_at timestamptz             -- auto-release after timeout
);

-- Review requests
create table reviews (
  id uuid primary key default gen_random_uuid(),
  project_id uuid references projects(id) on delete cascade,
  author_id uuid references auth.users(id),
  reviewer_id uuid references auth.users(id),
  status text check (status in ('pending', 'approved', 'changes_requested', 'rejected')),
  description text,
  from_version bigint,              -- edit_log range
  to_version bigint,
  created_at timestamptz default now(),
  resolved_at timestamptz
);

-- RLS policies
alter table projects enable row level security;
alter table project_members enable row level security;
alter table edit_log enable row level security;
alter table comments enable row level security;
alter table locks enable row level security;
alter table reviews enable row level security;

-- Members can see their own projects
create policy "members can view project" on projects
  for select using (
    id in (select project_id from project_members where user_id = auth.uid())
  );

-- Editors can insert edit ops
create policy "editors can insert edits" on edit_log
  for insert with check (
    project_id in (
      select project_id from project_members
      where user_id = auth.uid() and role in ('owner', 'editor')
    )
  );
```

### Dependencies (Pro only)

```toml
# crates/signex-collab/Cargo.toml
[dependencies]
tokio-tungstenite  = "0.26"          # Supabase Realtime WebSocket
tokio              = { workspace = true }
serde              = { workspace = true }
serde_json         = { workspace = true }
signex-types       = { path = "../signex-types" }
uuid               = { workspace = true }
chrono             = { workspace = true }
reqwest            = { workspace = true }  # Supabase REST API (PostgREST)
rusqlite           = { version = "0.32", features = ["bundled"] }  # Local offline op queue
```

```
# crates/signex-collab-server/ → supabase/functions/
# Edge Functions are TypeScript/Deno, NOT Rust crates
supabase/
├── config.toml                    # Supabase project config
├── migrations/                    # SQL migrations (schema above)
│   └── 001_collab_tables.sql
├── functions/
│   ├── crdt-merge/index.ts        # Merge incoming CRDT ops, broadcast via Realtime
│   ├── notify-review/index.ts     # Send notification on review request
│   ├── lock-acquire/index.ts      # Acquire/release locks with timeout
│   └── cleanup-expired/index.ts   # Cron: release expired locks, prune old ops
└── seed.sql                       # Development seed data
```

### Test Plan (v1.1.0)
- [ ] Community build: Share button shows upgrade prompt, no collab UI visible
- [ ] Pro build: Create workspace via Supabase Auth (email signup)
- [ ] Invite collaborator by email, role shows correctly
- [ ] Two clients connect to same project via Supabase Realtime
- [ ] Cursor of other user visible on canvas with name label
- [ ] User A places component → appears on User B's canvas within 500ms
- [ ] User A draws wire → appears on User B's canvas in real-time
- [ ] Sheet lock: User A locks Sheet1 → User B sees lock indicator, cannot edit Sheet1
- [ ] Region lock (PCB): User A locks region → User B cannot place/route in that region
- [ ] Net assignment: User A claims DDR nets → User B sees ownership indicator
- [ ] Offline: edits queue in local SQLite, replay correctly on reconnect
- [ ] Conflict: two users edit same property → last-write-wins via Edge Function merge
- [ ] Version history: browse edit_log timeline, diff shows correct changes
- [ ] Review: request review → reviewer sees diff overlay → approve merges changes
- [ ] Comments: pin comment to location, reply, resolve — real-time via Realtime
- [ ] Follow mode: clicking collaborator avatar follows their viewport
- [ ] Activity feed shows correct real-time events from Realtime channel
- [ ] RLS: viewer role cannot insert edits (Postgres rejects)
- [ ] Supabase Storage: project files upload/download correctly
- [ ] Lock timeout: expired locks auto-release via cron Edge Function

---

## Workspace Dependencies (Full)

```toml
[workspace]
resolver = "2"
members = [
    # Community edition (GPL-3.0)
    "crates/signex-app",
    "crates/signex-types",
    "crates/signex-render",
    "crates/signex-erc",
    "crates/signex-drc",
    "crates/kicad-parser",
    "crates/kicad-writer",
    "crates/pcb-geom",
    "crates/spice-gen",
    "crates/openems-bridge",
    "crates/elmer-bridge",
    "crates/formula-render",
    "crates/step-loader",
    "crates/plugin-api",
    # Pro edition (proprietary, feature-gated)
    "crates/signex-signal",
    "crates/signex-collab",
    # Server logic is in supabase/functions/ (Deno), not a Rust crate
]

[workspace.dependencies]
# GUI
iced            = { version = "0.14", features = ["wgpu", "canvas", "advanced"] }
iced_aw         = { version = "0.13", default-features = false, features = ["tabs", "card", "modal", "split"] }

# Rendering (wgpu version must match Iced's internal wgpu — check iced_wgpu/Cargo.toml)
wgpu            = "24"           # Iced 0.14 uses wgpu ~24; verify exact pin before starting

# Geometry & Math
clipper2        = "0.5"
nalgebra        = "0.34"
truck-modeling  = "0.6"

# Parsing
nom             = "8"
serde           = { version = "1", features = ["derive"] }
serde_json      = "1"

# Simulation I/O
hdf5            = "0.8"
vtkio           = "0.6"
num-complex     = "0.4"
libloading      = "0.8"

# Formula rendering
rex             = "0.4"
resvg           = "0.45"
tiny-skia       = "0.11"
typst           = "0.12"
typst-render    = "0.12"

# Networking (AI + Collab)
reqwest         = { version = "0.12", features = ["json", "rustls-tls", "stream"], default-features = false }
tokio           = { version = "1", features = ["full"] }
tokio-tungstenite = "0.26"       # Supabase Realtime WebSocket (Pro collab)
rusqlite        = { version = "0.32", features = ["bundled"] }  # Offline op queue (Pro collab)

# Plugin system
extism          = "1.21"

# Utilities
uuid            = { version = "1", features = ["v4", "serde"] }
chrono          = { version = "0.4", features = ["serde"] }
thiserror       = "2"
anyhow          = "1"
rfd             = "0.17"
arboard         = "3.6"

# Development
tracing         = "0.1"
tracing-subscriber = "0.3"
```

### External Tools (alplabai repos)

| Tool | Version | Repo | Branch | Language | Purpose |
|---|---|---|---|---|---|
| ngspice | pre-47 | `alplabai/ngspice` | main | C | SPICE sim (FFI via libngspice). 16+ analysis types, KLU solver, GPU OpenCL, arena allocators, batch/Monte Carlo API, electro-thermal co-sim, IBIS, device aging, electromigration |
| OpenEMS (project) | 0.0.36 | `alplabai/openEMS-Project` | main | Shell | Superproject (submodules). Branches: main, dev, feature/touchstone-export |
| OpenEMS (core) | 0.0.36 | `alplabai/openEMS` | main | C++ | EC-FDTD solver. exit()→exceptions, volatile→atomic, O(log N) SnapToMeshLine, memory leak fixes. Branches: main, dev, **feature/cuda-engine** |
| CSXCAD | 0.6.3 | `alplabai/CSXCAD` | main | C++ | Geometry library. Fixes: iterator UB after erase, UpdateIDs logic bug. Upstream: thliebig/CSXCAD |
| fparser | — | `alplabai/fparser` | master | C++ | Function parser for openEMS expressions. Fork of thliebig/fparser |
| AppCSXCAD | — | `alplabai/AppCSXCAD` | master | C++ | Minimal GUI for CSXCAD (Qt). Fork of thliebig/AppCSXCAD |
| QCSXCAD | — | `alplabai/QCSXCAD` | master | C++ | Qt GUI library for CSXCAD. Fork of thliebig/QCSXCAD |
| Elmer FEM | 26.1 | `alplabai/elmerfem` | devel | Fortran/C++ | Fork of ElmerCSC/elmerfem. Fixes: VectorHelmholtz segfault, 4 HIGH security bugs in Load.c/cholmod.c. Branch: signex-pcb |
| GMSH | 4.15.2 | upstream | — | C++ | Mesh generation (Elmer input). No fork needed |

**Notes:**
- All repos actively maintained (last push: 2026-04-08 to 2026-04-10)
- Zero open issues/PRs across all repos
- `alplabai/openEMS` has a `feature/cuda-engine` branch — GPU-accelerated FDTD solver
- ngspice has 58 custom commits on top of upstream pre-master-47
- CSXCAD has tags v0.6.0 through v0.6.3
- Elmer has 9,690 commits (full upstream history), signex-pcb branch has targeted fixes
- Build order: fparser → CSXCAD → openEMS (core) → openEMS-Project

---

## Quality Constraints

- `unwrap()`/`expect()` forbidden in production code — use `?` or `match`
- `clippy::all` + `clippy::pedantic` zero warnings
- `unsafe` forbidden (WASM FFI exception, must be commented)
- Every public function has doc comment
- Commit format: `feat(phase-N): description` / `fix(phase-N): description`
- All tests pass before merging to `dev`
- All CI checks pass before merging `dev` to `main`

## CI Pipeline

```yaml
# .github/workflows/ci.yml
on:
  push:
    branches: [dev, "feature/*"]
  pull_request:
    branches: [dev, main]

jobs:
  check:
    - cargo fmt --check
    - cargo clippy --workspace -- -D warnings
    - cargo test --workspace
    - cargo build --workspace --release
```
