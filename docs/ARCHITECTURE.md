# Signex — Architecture

> **Status:** Authoritative architectural reference.
> **Audience:** Engineers writing Signex code.
> **Companion to:** `MASTER_PLAN.md` (the *what* and *why*).

This document describes *how* Signex is built. Every architectural decision in
the codebase must trace back to a rule in this document. If something in the
code contradicts this document, the code is wrong — unless a proposal has been
merged that updates the document first.

---

## 1. The One Sentence That Drives Everything

Signex is a KiCad-native editor. The persisted document is a KiCad S-expression
file. The UI is a view onto that file, mediated by an engine. There is no
native format, no hidden second source of truth, no private representation that
outlives a session.

If you remember only one thing, remember this. Every rule below is a consequence.

---

## 2. The Four Layers

Signex is built as four distinct layers, each with a single responsibility.
They form a strict stack: each layer depends only on the layers below it, and
communication across non-adjacent layers is forbidden.

```
┌───────────────────────────────────────────────────────────┐
│  Layer 4 — Render Cache                                    │
│  Geometry caches, hit-test structures, dirty tracking.     │
│  Derived from Layer 2. Never persisted.                    │
├───────────────────────────────────────────────────────────┤
│  Layer 3 — UI State                                        │
│  Selection, hover, viewport, active tool, previews.        │
│  Transient. Never touches Layer 1 or Layer 2 directly.     │
├───────────────────────────────────────────────────────────┤
│  Layer 2 — Semantic Model                                  │
│  Typed domain objects: Symbol, Wire, Net, Sheet, Pin.      │
│  Identity-stable. The editing surface.                     │
├───────────────────────────────────────────────────────────┤
│  Layer 1 — Raw KiCad Document                              │
│  S-expression tree with spans, identity, unknown nodes.    │
│  The canonical persisted representation.                   │
└───────────────────────────────────────────────────────────┘
```

The engine (described in Section 4) is the component that moves data between
these layers. The engine is not itself a layer; it is the orchestrator that
enforces layer separation.

### 2.1. Layer 1 — Raw KiCad Document

**Purpose:** faithful representation of the on-disk KiCad file.

**Responsibilities:**

- Parse `.kicad_sch`, `.kicad_pcb`, `.kicad_sym`, `.kicad_pro`, and related
  files into a structured S-expression tree
- Preserve every node, including nodes the semantic model does not understand
- Preserve ordering of child nodes within parent lists
- Attach stable node handles (not positions) to every node
- Attach source spans (byte offsets in the original file) where possible
- Support incremental rewriting: changing one value should not require
  re-serializing the entire file
- Write the tree back to disk in a form KiCad accepts without complaint

**Non-responsibilities:**

- Interpreting what any node *means* (that is Layer 2's job)
- Connectivity, validation, or any design-level reasoning
- Rendering

**Crate:** `kicad-document` (see Section 7)

**Key data type:**

```rust
pub struct Document {
    pub root: NodeHandle,
    arena: NodeArena,           // All nodes stored here, referenced by handle
    spans: SpanMap,             // NodeHandle -> Option<Span>
    // ...
}

pub struct Node {
    pub kind: NodeKind,         // Atom | List
    pub handle: NodeHandle,     // Stable across edits
    pub children: Vec<NodeHandle>,
    pub value: Option<Atom>,
    // ...
}
```

The tree is arena-allocated. `NodeHandle` is a stable identifier; node position
within a parent's children can change, but handles never move. This is how we
avoid the classic S-expression editing failure mode where an insertion
invalidates every reference to every subsequent node.

### 2.2. Layer 2 — Semantic Model

**Purpose:** the editing surface. This is the layer the engine operates on and
the layer the UI reads from.

**Responsibilities:**

- Expose typed domain objects: `Symbol`, `Wire`, `Junction`, `Label`, `Sheet`,
  `Pin`, `Net`, `BusEntry`, and so on
- Assign every object a stable `ObjectId` (UUID) that outlives edits, saves,
  and reloads
- Provide a connectivity graph: which pins connect to which nets, which wires
  share junctions
- Maintain a bidirectional map between `ObjectId` and `NodeHandle` (semantic
  object to raw document node)
- Be rebuildable from Layer 1 without loss of the information Layer 2 actually
  needs

**Non-responsibilities:**

- Owning the persistence format (Layer 1 does)
- Knowing anything about rendering (Layer 4 does)
- Knowing anything about the user's current selection (Layer 3 does)
- Containing any unknown KiCad constructs — those stay in Layer 1

**Crate:** `signex-model`

**Key principle:** the semantic model is a *view* over the raw document. If a
KiCad file contains ten symbols, the semantic model contains ten `Symbol`
objects, each holding a `NodeHandle` back to Layer 1. If the semantic model
does not understand a node (for example, a vendor-specific extension), that
node simply is not represented in Layer 2 — but it is preserved in Layer 1 and
will be written back out on save.

### 2.3. Layer 3 — UI State

**Purpose:** everything the user is currently doing that is not part of the
design itself.

**Contents:**

- Selection (by `ObjectId`)
- Hover target
- Active tool (`SelectionTool`, `WireTool`, `SymbolPlacementTool`, etc.)
- Viewport (zoom, pan, active sheet)
- Tool preview geometry (the wire being drawn right now, the symbol being
  placed)
- Drag state, snap candidates, marquee rectangle
- Panel visibility, panel sizes, dock state
- Pending command state (the half-completed action waiting for more input)

**Non-contents:**

- Anything that should survive closing the editor
- Anything that should appear in another user's session during collaboration
- Any reference to Layer 1 node handles (UI always talks to Layer 2 by
  `ObjectId`)

**Crate:** `signex-app` (the iced application shell)

**Key principle:** UI state is disposable. Closing the editor throws it all
away. None of it is ever written to a KiCad file. This separation is what lets
Signex reload a project cleanly, open the same project in KiCad without
session-state pollution, and support collaboration without leaking one user's
selection into another's document.

### 2.4. Layer 4 — Render Cache

**Purpose:** make the canvas fast.

**Contents:**

- Per-object render primitives (the precomputed geometry for drawing each
  symbol, wire, label)
- Text layout caches (shaped glyphs for labels, ready to blit)
- Bounding boxes
- Spatial indices for hit-testing
- Layer-visibility filters
- Dirty-region and dirty-object tracking

**Crate:** `signex-render`

**Key principle:** the render cache is derived. It is rebuilt from the semantic
model whenever the semantic model changes. It is invalidated by the engine, not
by the UI directly. Rendering never reads from Layer 1 (the raw document);
doing so would couple display performance to persistence format details.

---

## 3. The Engine

The engine is the single most important component in the codebase.

### 3.1. Why It Exists

Without an engine, editing logic metastasizes into the UI. Every tool, every
menu action, every keyboard shortcut ends up holding its own understanding of
how to mutate the document. Undo/redo breaks because mutations happen in too
many places. Compatibility breaks because each mutation site reinvents how to
talk to Layer 1. Rendering breaks because nothing knows what was invalidated.

The engine exists so that these responsibilities have exactly one home.

### 3.2. What It Does

The engine is a headless crate that:

1. Accepts `Command` values from the UI (or from the AI, or from a plugin, or
   from a test harness)
2. Validates the command against the current semantic model
3. Produces a `SemanticPatch` describing the intended change
4. Produces a corresponding `DocumentPatch` describing the raw-document change
5. Applies both patches atomically
6. Updates `ObjectId` ↔ `NodeHandle` mappings as needed
7. Emits invalidation events to the render cache
8. Records the patch pair on the undo stack
9. Emits change notifications for any interested listeners (the UI, the
   collaboration subsystem, validators)

**Crate:** `signex-engine`

### 3.3. What It Does Not Do

- It does not render
- It does not know about selection, hover, or viewports
- It does not know about iced or any UI framework
- It does not parse or serialize files (it calls into `kicad-document` for that)

### 3.4. The Hard Rule

**The UI never mutates Layer 1 or Layer 2 directly. All mutations go through
the engine.**

This rule has no exceptions. If a piece of code needs to change the document
and it is not inside `signex-engine`, that code is wrong. If a new feature
feels like it cannot be expressed as a command, the feature is not yet
well-defined and should be decomposed until it can be.

This rule is enforced by crate visibility: the types in `signex-model` that
represent the semantic model are read-only to everyone except `signex-engine`.
Mutation methods are pub(crate) or live behind engine-owned interfaces.

### 3.5. Editing Flow

```
User interaction in UI
        │
        ▼
Message dispatched to application
        │
        ▼
Active tool translates Message -> Command
        │
        ▼
Engine::execute(Command)
        │
        ├─► Validation (is this command legal?)
        │
        ├─► SemanticPatch generation
        │
        ├─► DocumentPatch generation
        │
        ├─► Apply to Layer 2 (semantic model)
        │
        ├─► Apply to Layer 1 (raw document)
        │
        ├─► Update ObjectId ↔ NodeHandle maps
        │
        ├─► Record on undo stack
        │
        ├─► Emit render invalidation
        │
        └─► Emit change events
        │
        ▼
UI reads updated semantic model, re-renders from refreshed cache
```

Every link in this chain is testable without any UI present. The engine's
test suite can drive the entire flow with synthetic commands and assert that
the semantic model, the raw document, and the undo stack all end up in the
correct state. This is how we can develop and verify editing logic before any
canvas pixel is drawn.

### 3.6. Commands

Commands are the vocabulary the UI uses to talk to the engine. They are
user-intent-level, not tree-manipulation-level.

**Good commands:**

- `MoveObjects { ids, delta }`
- `DrawWire { from, to, routing_style }`
- `PlaceSymbol { lib_id, position, orientation }`
- `UpdateProperty { id, property, value }`
- `DeleteObjects { ids }`
- `RenameLabel { id, new_text }`
- `ConnectPin { pin_id, net_id }`

**Bad commands (these would be rejected in review):**

- `InsertSExprNode { parent, child }`
- `SetFifthAtomOf { handle, value }`
- `ReplaceSubtree { at_offset, with }`

The bad examples describe *tree surgery*. The good examples describe *user
intent*. The engine translates intent into tree surgery. The UI never does.

### 3.7. Engine Public API Shape

```rust
// signex-engine/src/lib.rs

pub struct Engine {
    model: Model,               // Layer 2
    document: Document,         // Layer 1 (via kicad-document)
    history: UndoStack,
    id_map: IdentityMap,        // ObjectId <-> NodeHandle
    listeners: ListenerRegistry,
}

impl Engine {
    pub fn new(document: Document) -> Result<Self, EngineError>;
    pub fn open(path: &Path) -> Result<Self, EngineError>;
    pub fn save(&mut self) -> Result<(), EngineError>;
    pub fn save_as(&mut self, path: &Path) -> Result<(), EngineError>;

    // The only way to change anything:
    pub fn execute(&mut self, cmd: Command) -> Result<CommandResult, EngineError>;

    pub fn undo(&mut self) -> Result<Option<PatchPair>, EngineError>;
    pub fn redo(&mut self) -> Result<Option<PatchPair>, EngineError>;

    // Read-only access for the UI and the render layer:
    pub fn model(&self) -> &Model;
    pub fn document(&self) -> &Document;

    // Listener registration for render invalidation, collab sync, etc.:
    pub fn subscribe(&mut self, listener: Box<dyn EngineListener>) -> ListenerId;
}
```

The UI holds an `Engine` (or a handle to one). It reads the model freely. It
changes the model only by calling `execute`.

---

## 4. The Two-Level Patch System

Every edit produces two patches: one expressing user intent (`SemanticPatch`)
and one expressing the raw-file change (`DocumentPatch`). They are applied and
undone together as a unit.

### 4.1. Why Two Levels

A single level is not enough:

- If we only had `DocumentPatch`, undo/redo would be meaningless to users —
  "undo atom replacement at offset 2,481" is not a useful history entry, and
  coalescing related mutations would be impossible.
- If we only had `SemanticPatch`, we would regenerate the entire document on
  every save, destroying round-trip stability and unknown-node preservation.

Two levels give us both: the user sees meaningful history, and the file sees
minimal, stable changes.

### 4.2. SemanticPatch

A `SemanticPatch` describes a change in terms of semantic objects.

```rust
pub enum SemanticPatch {
    AddObject { id: ObjectId, object: SemanticObject },
    RemoveObject { id: ObjectId },
    UpdateObject { id: ObjectId, before: SemanticObject, after: SemanticObject },
    MoveObject { id: ObjectId, before: Position, after: Position },
    // Composite patch for multi-object atomic edits:
    Composite { patches: Vec<SemanticPatch>, label: String },
}
```

Semantic patches are what the undo stack holds. They are what change
notifications carry. They are what collaboration synchronizes. They are what
the UI inspects to update its own state.

### 4.3. DocumentPatch

A `DocumentPatch` describes a change in terms of the raw S-expression tree.

```rust
pub enum DocumentPatch {
    InsertNode { parent: NodeHandle, position: usize, node: NodeData },
    RemoveNode { handle: NodeHandle },
    ReplaceAtom { handle: NodeHandle, new_atom: Atom },
    ReorderChildren { parent: NodeHandle, new_order: Vec<NodeHandle> },
    Composite { patches: Vec<DocumentPatch> },
}
```

Document patches are what actually mutate Layer 1. They are computed by the
engine from the semantic patch plus the current `IdentityMap`. The writer uses
them to produce minimal-diff output.

### 4.4. Patch Pairing

Every `execute(Command)` produces exactly one `PatchPair`:

```rust
pub struct PatchPair {
    pub semantic: SemanticPatch,
    pub document: DocumentPatch,
    pub label: String,              // Human-readable, for undo UI
    pub timestamp: Instant,
}
```

Both patches are reversible. Undoing a `PatchPair` means applying the inverse
semantic patch and the inverse document patch, atomically. The engine
guarantees they never drift out of sync.

### 4.5. The Invariant

After every `execute`, `undo`, or `redo` call, the following must hold:

> The semantic model derived fresh from the current Layer 1 document is equal
> to the current Layer 2 state, modulo the unknown-node content that Layer 2
> does not represent.

This invariant is checked in debug builds after every operation and in CI on
a large fixture corpus. If it ever fails, something is wrong with patch
generation and no further work can proceed until it is fixed.

---

## 5. Identity

Stable identity across three dimensions is essential. Positional identity —
"the fifth child of this list" — is never trusted.

### 5.1. Semantic Identity: `ObjectId`

Every semantic object has a `ObjectId`, a UUIDv4. It is assigned once, on
creation (whether by loading from disk or by a user command) and never changes.
It survives:

- Edits to the object (moving, renaming, updating properties)
- Edits to other objects
- Save and reload
- Undo and redo

`ObjectId` is what the UI uses to refer to objects. Selection is a set of
`ObjectId`s. Cross-probing uses `ObjectId`s. Collaboration protocol messages
use `ObjectId`s. Diagnostics point to `ObjectId`s.

### 5.2. Document Identity: `NodeHandle`

Every node in the raw S-expression tree has a `NodeHandle`, an opaque
arena-allocated identifier. It is stable across insertions and deletions of
sibling nodes. It is lost only when the node itself is removed.

`NodeHandle` is what the writer uses for minimal-rewrite targeting. It is what
the `IdentityMap` maps to from `ObjectId`. It is also what source-mapped
diagnostics use to point back to file offsets.

### 5.3. The IdentityMap

The `IdentityMap` is the bridge between Layers 1 and 2:

```rust
pub struct IdentityMap {
    semantic_to_document: HashMap<ObjectId, NodeHandle>,
    document_to_semantic: HashMap<NodeHandle, ObjectId>,
}
```

The engine maintains this map. Every edit that adds, removes, or re-parents
objects updates the map atomically with the patches. The map is rebuilt from
scratch on load (using KiCad's UUIDs where present, or deterministically
generated ones where not).

### 5.4. KiCad UUIDs

KiCad files already contain UUIDs for many objects. When we load a KiCad file,
we use those UUIDs as `ObjectId` values directly. This preserves object
identity across save/reload cycles and across KiCad ↔ Signex round-trips.

For objects where KiCad does not provide a UUID (some older constructs, some
annotations), we generate one deterministically from file content on first
load, so that reopening the same unchanged file produces the same `ObjectId`
values.

---

## 6. Sync Rules

This section is the contract between the layers. Every rule here is enforceable
by review, by crate visibility, and by test.

### 6.1. Who May Read What

| Reader        | May read Layer 1 | May read Layer 2 | May read Layer 3 | May read Layer 4 |
|---------------|:---:|:---:|:---:|:---:|
| UI            | no  | yes | yes | yes |
| Engine        | yes | yes | no  | no  |
| Render cache  | no  | yes | no  | yes |
| Validators    | no  | yes | no  | no  |
| Collab client | no  | yes | no  | no  |

The UI does not read Layer 1 directly. This keeps file-format details out of
the UI. The engine does not read Layer 3 (UI state), because that would make
editing outcomes depend on which panel was open.

### 6.2. Who May Write What

| Writer        | May write Layer 1 | May write Layer 2 | May write Layer 3 | May write Layer 4 |
|---------------|:---:|:---:|:---:|:---:|
| UI            | no  | no  | yes | no  |
| Engine        | yes | yes | no  | yes (invalidate) |
| Render cache  | no  | no  | no  | yes |

Only the engine mutates the design. The UI mutates its own state only. The
render cache mutates itself only.

### 6.3. Ordering

When the engine applies a `PatchPair`:

1. Validate the command against the current Layer 2 model
2. Generate the semantic patch
3. Generate the document patch (using the current `IdentityMap`)
4. Apply the semantic patch to Layer 2
5. Apply the document patch to Layer 1
6. Update the `IdentityMap`
7. Push the `PatchPair` to the undo stack
8. Emit invalidation events to Layer 4
9. Emit change notifications to listeners

This order is not negotiable. The semantic patch applies first because the
document patch depends on the updated `IdentityMap` for some patch types.
Listeners fire last because they may call back into the engine.

### 6.4. Save

`engine.save()` does not regenerate the file. It asks the `kicad-document`
crate to write the current Layer 1 tree back out, preserving source spans
and node order where possible. Because every edit has already been applied
to Layer 1 incrementally, the file on disk after a save is the result of
minimal rewrite, not regeneration.

### 6.5. Load

`engine.open(path)` performs:

1. Parse the file into a Layer 1 `Document`
2. Derive the Layer 2 `Model` by walking the document and constructing
   semantic objects for every recognized construct
3. Build the initial `IdentityMap`
4. Initialize Layer 3 and Layer 4 as empty

Unknown nodes in Layer 1 are simply not represented in Layer 2. They remain in
the document. On save, they are written back out unchanged. This is how
compatibility with future KiCad versions and vendor extensions is maintained.

---

## 7. Crate Boundaries

The crates listed here are the architectural crates. The full workspace
includes others (UI, rendering, validation) but those are implementation
details; these are the load-bearing boundaries.

### `kicad-document`

The raw document layer. Parses and writes KiCad files. Owns `Document`, `Node`,
`NodeHandle`, `Span`, and the writer that produces minimal-diff output.

Dependencies: none from Signex (only external parsing crates).

Consumers: `signex-engine`.

### `signex-model`

The semantic model layer. Defines `Symbol`, `Wire`, `Net`, `Sheet`, `Pin`,
`Label`, and so on. Defines `ObjectId`. Provides read-only accessors. Mutation
is `pub(crate)` — only `signex-engine` can construct or modify semantic
objects.

Dependencies: `kicad-document` (to hold `NodeHandle` references).

Consumers: `signex-engine`, UI, render, validators.

### `signex-engine`

The engine. Owns `Engine`, `Command`, `SemanticPatch`, `DocumentPatch`,
`PatchPair`, `UndoStack`, `IdentityMap`, `EngineListener`.

Dependencies: `kicad-document`, `signex-model`.

Consumers: `signex-app` (the UI), `signex-erc`, `signex-drc`, `signex-collab`.

### `signex-render`

The render cache layer. Derives drawable primitives from the semantic model.
Handles hit-testing, bounding boxes, dirty tracking.

Dependencies: `signex-model`.

Consumers: `signex-app`.

### `signex-app`

The UI shell. Holds `Engine`. Defines tools, panels, the iced application.
Translates user input into `Command` values and dispatches them.

Dependencies: everything above.

Consumers: none (it is the binary).

The dependency graph is acyclic and strictly downward. If a crate ever needs
to depend on something above it, the design is wrong.

---

## 8. Compatibility Policy

This section is enforced as rigorously as any code rule.

### 8.1. Supported KiCad Versions

Signex's first-class target is **KiCad 9.x**.

- **KiCad 8.x files:** supported on read. Signex will open them and produce
  KiCad 9.x output on save (KiCad 8 will still accept this in most cases, but
  the user is warned).
- **KiCad 7.x and earlier:** not supported. Users are advised to open the
  project in KiCad and save it once to upgrade the format.
- **KiCad 10.x (when released):** supported on best-effort basis via
  unknown-node preservation until an explicit update.

### 8.2. Round-Trip Guarantee

For any supported KiCad file, the following holds:

> Parsing the file, applying zero edits, and writing it back produces output
> that KiCad opens without warnings, and that differs from the input only in
> insignificant ways (whitespace, comment formatting, if anything at all).

This is a **hard guarantee**. A change that breaks this is a regression, not a
tradeoff. It is tested on every merge against a corpus of real KiCad projects.

### 8.3. Unknown Construct Handling

If Signex's semantic model does not recognize a construct in a KiCad file:

- The construct is preserved in Layer 1 unchanged
- The construct is not represented in Layer 2
- On save, the construct is written back out in its original form
- The UI does not display the construct (it is invisible to the user)
- Validation does not reason about the construct

This behavior applies to vendor extensions, future KiCad features,
KiCad-version-specific constructs we have not yet modeled, and any other
"we don't understand this" case. The default action is preservation, not
rejection.

### 8.4. What We Never Do

- We never strip metadata the user did not ask to remove
- We never reformat the whole file on save
- We never reorder nodes that were not affected by an edit
- We never add proprietary Signex tags to the document
- We never assume a construct is unused just because we don't render it

### 8.5. What We Warn About

- Parsing a file from an older KiCad version (informational, once per session)
- Parsing a file with constructs we preserve but cannot edit (user is told
  these will round-trip but not be editable in Signex)
- Saving a file that would change the KiCad format version (user confirms)

### 8.6. What We Refuse

- Files from KiCad versions earlier than 8 (we do not silently upgrade; we
  direct the user to open them in KiCad first)
- Files that are not valid S-expressions (we report parse errors with source
  location)
- Save paths that would overwrite a file we didn't open (accidental-overwrite
  guard)

---

## 9. Open Architectural Questions

These are questions that the current architecture document does not yet
answer definitively. They will be resolved during Phase 0 or early Phase 1 and
this document will be updated accordingly.

### 9.1. Byte-Identical vs. Structural Minimal Rewrite

The goal is minimal-diff output. The open question is whether we aim for
*byte-identical* output to KiCad's own writer for unchanged regions (requires
preserving lossless trivia — whitespace, comments — through the parser), or
*structural* minimal rewrite (deterministic, stable serialization; diffs
minimal but not necessarily zero). The first is more valuable but significantly
more expensive to implement.

**Decision target:** end of Phase 0, based on how close a straightforward
implementation gets to KiCad's output on the fixture corpus.

### 9.2. Library Management

Symbol and footprint libraries are loaded via `sym-lib-table` and `fp-lib-table`.
Library cache, missing-library handling, and library version drift are major
subsystems. Their architecture is deferred to early Phase 1 but must be designed
before any UI work that depends on libraries begins.

### 9.3. DRC/ERC as Third Validation Layer

Structural validation (is the file well-formed?) and semantic validation (are
the references consistent?) are part of Layers 1 and 2. DRC and ERC — are the
electrical design rules satisfied? — are a third, heavier validation layer
that runs separately, possibly asynchronously. Their architectural placement
(inside `signex-engine` or as sibling crates) is deferred to the validation
phase.

### 9.4. Collaboration and the Engine

Live collaboration (Pro, v3.0+) synchronizes semantic patches across clients.
This implies that the engine must be able to accept a `SemanticPatch` from a
remote source and apply it safely. The API shape for this — whether remote
patches are a distinct `Command` variant, whether they bypass validation, how
conflicts are reconciled — is deferred to the Pro design phase but the current
engine API is designed with this use case in mind.

---

## 10. Rules for Changing This Document

This document changes. Architecture evolves. But it does not evolve in the
codebase first.

- Changes to this document require an architectural proposal and review
- Code that contradicts this document is reverted, even if it works
- New layers, new crates, or new sync rules are documented here before they
  exist in code
- If you find yourself reaching for an exception to a rule here, write the
  proposal first

The point of this document is to give the codebase something to be consistent
with. If we let the codebase drift and the document trail behind, we lose the
consistency, which is the whole value.
