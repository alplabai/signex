//! The `.snxpcb` format tokens and the one rule that picks the token a save
//! writes. Policy: `docs/FORMAT_VERSIONS.md`.

use crate::format_version::{FormatVersion, MissingToken, NewerMinor};
use crate::pcb::PcbBoard;

/// Current `.snxpcb` format version. 1.1: pads are keyed to their footprint
/// by uuid (1.0 keyed by reference designator) and the stackup carries
/// `layer_types`. 1.2: the stackup carries `layer_ids`. 1.3: pads may
/// carry a `rotation` column (their own turn), `trapezoid` and `custom` pad
/// shapes are gone, and an unknown shape token or a roundrect ratio outside
/// `0..=0.5` is a load error. 1.4: the `trapezoid` and `custom` pad shapes
/// return with their parameters in the optional `trapezoid_delta` and
/// `custom_outline` pad columns, and a parameter that cannot make copper is
/// a load error. The policy is [`SNXPCB_VERSION`].
pub const SNXPCB_FORMAT_V1: &str = "snxpcb/1.4";

/// Written instead of [`SNXPCB_FORMAT_V1`] when the board carries layer
/// colours (`[board_options.layer_colors]`, 2026-11-18): a build that knows
/// only 1.4 would drop them on save, so it opens 1.5 read-only. A board
/// without layer colours stays 1.4, byte for byte.
pub const SNXPCB_FORMAT_LAYER_COLORS: &str = "snxpcb/1.5";

/// Written when the board sketch carries a slot, a pour, a mechanical
/// drawing or a component anchor (decision 2, 2026-11-20), or any of the
/// `snxfpt/1.2` sketch lists (reference dimensions, pattern links and
/// mirrors, pad overrides): an older build would drop those from
/// `[board_sketch.sketch]` on save, so it opens 1.7 read-only. 1.6 is the rooms token of another change.
pub const SNXPCB_FORMAT_BOARD_SKETCH_TARGETS: &str = "snxpcb/1.7";

/// The token a save of `board` writes, from its content: the board sketch
/// targets need [`SNXPCB_FORMAT_BOARD_SKETCH_TARGETS`], layer colours
/// [`SNXPCB_FORMAT_LAYER_COLORS`], anything else [`SNXPCB_FORMAT_V1`]. A
/// board whose last such item is gone goes back to the lower token.
pub fn snxpcb_token(board: &PcbBoard) -> &'static str {
    let targets = board
        .sketch
        .as_ref()
        .is_some_and(|stored| {
            stored.sketch.carries_board_targets() || stored.sketch.needs_format_1_2()
        });
    if targets {
        SNXPCB_FORMAT_BOARD_SKETCH_TARGETS
    } else if board.options.layer_colors.is_empty() {
        SNXPCB_FORMAT_V1
    } else {
        SNXPCB_FORMAT_LAYER_COLORS
    }
}

/// `.snxpcb` version policy: a newer minor opens read-only. A save writes
/// the lowest token its content needs ([`snxpcb_token`]).
pub const SNXPCB_VERSION: FormatVersion = FormatVersion {
    current: SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
    readable: &[
        "snxpcb/1",
        "snxpcb/1.1",
        "snxpcb/1.2",
        "snxpcb/1.3",
        SNXPCB_FORMAT_V1,
        SNXPCB_FORMAT_LAYER_COLORS,
        SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
    ],
    newer_minor: NewerMinor::ReadOnly,
    missing: MissingToken::Refused,
};
