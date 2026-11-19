//! The implicit inputs of a parity row: the closed vocabulary, the
//! [`CommandContext`] facts each stands for, and which facts a gate reads.

use std::collections::BTreeSet;

use super::super::CommandContext;

/// What a command reads besides its operands. The list is closed (#2504).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Input {
    ActiveDocument,
    Window,
    Selection,
    Cursor,
    ToolState,
    PrePlacementForm,
    InProgressEdit,
    RenderCache,
    Clipboard,
    DialogAnswer,
    /// Who calls, a person or an agent: a port operand of `execute` in
    /// ADR-0006, not an implicit input, so it never meets the kill
    /// criterion. Today the code reads it as GUI state (`agent.serving`).
    Origin,
}

impl Input {
    pub(super) const ALL: [Self; 11] = [
        Self::ActiveDocument,
        Self::Window,
        Self::Selection,
        Self::Cursor,
        Self::ToolState,
        Self::PrePlacementForm,
        Self::InProgressEdit,
        Self::RenderCache,
        Self::Clipboard,
        Self::DialogAnswer,
        Self::Origin,
    ];

    /// A port operand rather than an implicit input.
    pub(super) fn is_port_operand(self) -> bool {
        self == Self::Origin
    }

    pub(super) fn word(self) -> &'static str {
        match self {
            Self::ActiveDocument => "active document",
            Self::Window => "window",
            Self::Selection => "selection",
            Self::Cursor => "cursor",
            Self::ToolState => "tool state",
            Self::PrePlacementForm => "pre-placement form",
            Self::InProgressEdit => "in-progress edit",
            Self::RenderCache => "render cache",
            Self::Clipboard => "clipboard",
            Self::DialogAnswer => "dialog answer",
            Self::Origin => "origin",
        }
    }
}

/// Sets one [`CommandContext`] fact.
pub(super) type Set = fn(&mut CommandContext, bool);

/// Every [`CommandContext`] fact and the inputs it stands for.
/// `every_context_fact_is_listed` fails to compile when a field is added.
pub(super) const FACTS: &[(&str, &[Input], Set)] = {
    use Input::*;
    &[
        ("has_schematic", &[ActiveDocument], |c, v| {
            c.has_schematic = v
        }),
        ("has_pcb", &[ActiveDocument], |c, v| c.has_pcb = v),
        ("pcb_view_turned", &[Window], |c, v| c.pcb_view_turned = v),
        ("undocked_has_canvas", &[Window], |c, v| {
            c.undocked_has_canvas = v
        }),
        (
            "document_is_schematic",
            &[ActiveDocument, Window],
            |c, v| {
                c.document_is_schematic = v;
            },
        ),
        ("document_is_editor", &[ActiveDocument, Window], |c, v| {
            c.document_is_editor = v;
        }),
        (
            "document_is_dirty_board",
            &[ActiveDocument, Window],
            |c, v| {
                c.document_is_dirty_board = v;
            },
        ),
        ("has_sheets", &[ActiveDocument], |c, v| c.has_sheets = v),
        ("has_symbol_editor", &[ActiveDocument], |c, v| {
            c.has_symbol_editor = v
        }),
        ("has_footprint_editor", &[ActiveDocument], |c, v| {
            c.has_footprint_editor = v;
        }),
        ("has_selection", &[Selection], |c, v| c.has_selection = v),
        ("selection_all_locked", &[Selection], |c, v| {
            c.selection_all_locked = v
        }),
        ("has_drawing_selection", &[Selection], |c, v| {
            c.has_drawing_selection = v;
        }),
        ("drawing", &[InProgressEdit], |c, v| c.drawing = v),
        ("can_undo", &[ActiveDocument], |c, v| c.can_undo = v),
        ("can_redo", &[ActiveDocument], |c, v| c.can_redo = v),
        ("project_dirty", &[ActiveDocument], |c, v| {
            c.project_dirty = v
        }),
        ("document_has_file", &[ActiveDocument, Window], |c, v| {
            c.document_has_file = v;
        }),
        ("has_dirty_documents", &[ActiveDocument], |c, v| {
            c.has_dirty_documents = v
        }),
        ("has_document", &[ActiveDocument], |c, v| c.has_document = v),
        ("has_open_project", &[ActiveDocument], |c, v| {
            c.has_open_project = v
        }),
        ("has_parent_sheet", &[ActiveDocument], |c, v| {
            c.has_parent_sheet = v
        }),
        ("has_active_path", &[ActiveDocument], |c, v| {
            c.has_active_path = v
        }),
        ("has_library_editor", &[ActiveDocument], |c, v| {
            c.has_library_editor = v
        }),
        ("has_open_library", &[ActiveDocument], |c, v| {
            c.has_open_library = v
        }),
        ("has_project_sheet", &[ActiveDocument], |c, v| {
            c.has_project_sheet = v
        }),
        ("can_tile_windows", &[Window], |c, v| c.can_tile_windows = v),
        ("is_tiled", &[Window], |c, v| c.is_tiled = v),
        ("can_remove_part", &[ActiveDocument], |c, v| {
            c.can_remove_part = v
        }),
        ("turn_in_hand", &[ToolState, InProgressEdit], |c, v| {
            c.turn_in_hand = v
        }),
        ("pcb.shown", &[ActiveDocument], |c, v| c.pcb.shown = v),
        ("pcb.any", &[Selection], |c, v| c.pcb.any = v),
        ("pcb.placeable", &[Selection], |c, v| c.pcb.placeable = v),
        ("pcb.deletable", &[Selection], |c, v| c.pcb.deletable = v),
        ("pcb.copyable", &[Selection], |c, v| c.pcb.copyable = v),
        ("pcb.pasteable", &[Clipboard], |c, v| c.pcb.pasteable = v),
        ("pcb.part_in_hand", &[ToolState], |c, v| {
            c.pcb.part_in_hand = v
        }),
        ("pcb.routing", &[InProgressEdit], |c, v| c.pcb.routing = v),
        ("pcb.lockable", &[Selection], |c, v| c.pcb.lockable = v),
        ("pcb.unlockable", &[Selection], |c, v| c.pcb.unlockable = v),
        ("pcb.read_only", &[ActiveDocument], |c, v| {
            c.pcb.read_only = v
        }),
        // Only 0 or 2: no gate reads the count, which
        // `no_gate_reads_the_footprint_count` checks with 1 too.
        ("pcb.footprints", &[Selection], |c, v| {
            c.pcb.footprints = if v { 2 } else { 0 };
        }),
        ("pcb.pad_pair", &[Selection], |c, v| c.pcb.pad_pair = v),
        ("pcb.one_footprint", &[Selection], |c, v| {
            c.pcb.one_footprint = v
        }),
    ]
};

/// Contexts the probe samples. A fact that decides a gate only while `w`
/// other facts hold given values is pivotal in about `SAMPLES / 2^w` of
/// them, so a deep one can go unseen. `the_probe_finds_every_fact_a_gate_reads`
/// checks the probe against an exhaustive enumeration of the facts each
/// gate's source names, independently of what the probe found.
const SAMPLES: usize = 1024;

/// Random contexts, the same on every run (a fixed xorshift seed).
pub(super) fn samples() -> impl Iterator<Item = CommandContext> {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..SAMPLES).map(move |_| {
        let mut ctx = CommandContext::default();
        for (_, _, set) in FACTS {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            set(&mut ctx, state & 1 == 1);
        }
        ctx
    })
}

/// Whether `gate` holds in some sampled context.
pub(super) fn ever(gate: impl Fn(&CommandContext) -> bool) -> bool {
    samples().any(|ctx| gate(&ctx))
}

/// The inputs `gate` reads: those of every fact [`facts_read_by`] finds.
pub(super) fn read_by(gate: impl Fn(&CommandContext) -> bool) -> BTreeSet<Input> {
    facts_read_by(gate)
        .into_iter()
        .flat_map(|name| inputs_of(name).iter().copied())
        .collect()
}

/// The facts whose flip changes `gate`'s answer in some sampled context.
pub(super) fn facts_read_by(gate: impl Fn(&CommandContext) -> bool) -> BTreeSet<&'static str> {
    let mut read = BTreeSet::new();
    for ctx in samples() {
        for (name, _, set) in FACTS {
            if decides(&gate, ctx, *set) {
                read.insert(*name);
            }
        }
    }
    read
}

/// Whether the fact `set` sets decides `gate` in `ctx`.
pub(super) fn decides(
    gate: impl Fn(&CommandContext) -> bool,
    ctx: CommandContext,
    set: Set,
) -> bool {
    let (mut on, mut off) = (ctx, ctx);
    set(&mut on, true);
    set(&mut off, false);
    gate(&on) != gate(&off)
}

fn inputs_of(fact: &str) -> &'static [Input] {
    FACTS
        .iter()
        .find(|(name, _, _)| *name == fact)
        .map_or(&[], |(_, inputs, _)| *inputs)
}
