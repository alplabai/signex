//! The operands of the board routing tools (#2470): a track, a route, a
//! differential pair, a multi-net route or a via named in full, so the
//! command line routes net by net through the interactive router without a
//! pointer. Points count from the board origin with Y up, as the status bar
//! shows them; a point also takes a pad as `<designator>.<pad>`. See
//! `docs/CLI.md`, "Routing a board".

use super::operands::{Operand, OperandKind, optional, required};

/// One net's run from `start` to `end`, through the corners `through`
/// (`x,y;x,y`, in order), on the copper `layer` (the layer of the copper
/// under `start` when left out), `width` mm wide (the net class's, held
/// within the rules, when left out) and on `net` (the net under `start`
/// when left out). `corner` is how each leg bends: `90` (the default),
/// `45_start`, `45_end` or `free`. `dry_run` answers without changing the
/// board. Track and Quick Route take these.
pub const BOARD_TRACK: &[Operand] = &[
    required("document", OperandKind::Document),
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("through", OperandKind::Text),
    optional("layer", OperandKind::Text),
    optional("width", OperandKind::Text),
    optional("net", OperandKind::Text),
    optional("corner", OperandKind::Text),
    optional("dry_run", OperandKind::Bool),
];

/// Interactive Routing: as [`BOARD_TRACK`], each leg routed around other
/// nets' copper, or pushing their tracks aside with `mode=shove`
/// (`walkaround`, the default).
pub const BOARD_ROUTE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("through", OperandKind::Text),
    optional("layer", OperandKind::Text),
    optional("width", OperandKind::Text),
    optional("net", OperandKind::Text),
    optional("corner", OperandKind::Text),
    optional("mode", OperandKind::Text),
    optional("dry_run", OperandKind::Bool),
];

/// Differential Pair Routing: the pair of `net` (either of its nets), each
/// member started at its copper nearest `start`, the centre line routed to
/// `end` through `through`, at the pair's coupled width and gap.
pub const BOARD_PAIR_ROUTE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("net", OperandKind::Text),
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("through", OperandKind::Text),
    optional("layer", OperandKind::Text),
    optional("corner", OperandKind::Text),
    optional("dry_run", OperandKind::Bool),
];

/// Multi-Route: the `nets` named (two or more, comma separated) side by
/// side, as [`BOARD_PAIR_ROUTE`].
pub const BOARD_MULTI_ROUTE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("nets", OperandKind::Text),
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("through", OperandKind::Text),
    optional("layer", OperandKind::Text),
    optional("corner", OperandKind::Text),
    optional("dry_run", OperandKind::Bool),
];

/// A via `at` a point, on `net` (the net under it when left out), joining
/// the two copper `layers` (`TopCopper,BottomCopper`, a through via, when
/// left out), `via_diameter` and `via_drill` mm (the board's when left out).
pub const BOARD_VIA: &[Operand] = &[
    required("document", OperandKind::Document),
    required("at", OperandKind::Point),
    optional("net", OperandKind::Text),
    optional("layers", OperandKind::Text),
    optional("via_diameter", OperandKind::Text),
    optional("via_drill", OperandKind::Text),
    optional("dry_run", OperandKind::Bool),
];
