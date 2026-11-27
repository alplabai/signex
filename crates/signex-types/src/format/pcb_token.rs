//! The `.snxpcb` format tokens and the one rule that picks the token a save
//! writes. Policy: `docs/FORMAT_VERSIONS.md`.

use crate::format_version::{FormatVersion, FormatVersionError, MissingToken, NewerMinor};
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

/// Written when the board has rooms (`[[rooms]]`, #2457, 2026-11-20): a
/// build that knows only 1.5 would drop them on save, so it opens 1.6
/// read-only. A board without rooms keeps 1.4 or 1.5, byte for byte.
pub const SNXPCB_FORMAT_ROOMS: &str = "snxpcb/1.6";

/// Written when the board sketch carries a slot, a pour, a mechanical
/// drawing or a component anchor (decision 2, 2026-11-20), or any of the
/// `snxfpt/1.2` sketch lists (reference dimensions, pattern links and
/// mirrors, pad overrides): an older build would drop those from
/// `[board_sketch.sketch]` on save, so it opens 1.7 read-only.
pub const SNXPCB_FORMAT_BOARD_SKETCH_TARGETS: &str = "snxpcb/1.7";

/// Written when the stackup has a name (`stack_name` in `[stackup]`, mockup
/// row 105, 2026-11-22), when a board sketch pattern repeats a baked target
/// (#2517), or when an array has a non-default numbering sweep (mockup row
/// 126): a build that knows only 1.7 would lose each on save or re-bake, so
/// it opens 1.8 read-only. A board with none keeps its token, byte for byte.
pub const SNXPCB_FORMAT_V1_8: &str = "snxpcb/1.8";

/// Written when a pour is shelved (#2738, 2026-11-23): a build that does not
/// know the key would flood the pour and export copper the author took out,
/// so it opens 1.9 read-only. A board with no shelved pour keeps its token,
/// byte for byte.
pub const SNXPCB_FORMAT_V1_9: &str = "snxpcb/1.9";

/// Written when the stackup carries a finish (`[stackup.finish]`) or a row
/// carries a loss tangent from the material library (mockup rows 176 and
/// 177, 2026-11-24): a build that knows only 1.9 would drop the row's Df on
/// save, so it opens 1.10 read-only. A board with neither keeps its token,
/// byte for byte.
pub const SNXPCB_FORMAT_V1_10: &str = "snxpcb/1.10";

/// Written when the board has a pour cutout (`[[pour_cutouts]]`, #2871), a
/// dimension with a non-default look or a kind other than aligned
/// ([`super::board_uses_dimension_style`], #2868, #2507), DECISIONS
/// 2026-11-24, or drawn copper: a board drawing or string
/// on a copper layer ([`super::board_has_drawn_copper`], #2883), DECISIONS
/// 2026-11-25, or an unfilled pour (`Zone::unfilled`, #2506), DECISIONS
/// 2026-11-25. A build that knows only 1.10 would drop the look keys on
/// save, has never seen a cutout, does not count drawn copper as copper and
/// would flood an unfilled pour, so it opens 1.11 read-only. A board with
/// none keeps its token, byte for byte.
pub const SNXPCB_FORMAT_V1_11: &str = "snxpcb/1.11";

/// Written when the board has a layout plan (`[board_options.layout_plan]`,
/// #2942), a length-matching group (`[[board_options.length_groups]]`,
/// #2797), a teardrop rule other than the default
/// (`[board_options.teardrops]`), or a pour with corners other than Sharp
/// (#2578) or a hatched fill (#2579) (`Zone::has_pour_shape`). A build that
/// knows only 1.11 would drop the plan, the groups and the teardrop rule on
/// save, and would fill a shaped pour solid with sharp corners, so it opens
/// 1.12 read-only. A board with none keeps its token, byte for byte.
pub const SNXPCB_FORMAT_V1_12: &str = "snxpcb/1.12";

/// Written while the board holds any 1.13 content ([`needs_1_13`]): a build
/// that knows only 1.12 would lose it on save, so it opens 1.13 read-only.
/// A board without any keeps its token, byte for byte.
pub const SNXPCB_FORMAT_V1_13: &str = "snxpcb/1.13";

/// Whether `board` holds content only [`SNXPCB_FORMAT_V1_13`] carries. One rule for
/// every 1.13 trigger; today a board sketch whose linear pattern has a
/// suppression list (#2688).
fn needs_1_13(board: &PcbBoard) -> bool {
    board
        .sketch
        .as_ref()
        .is_some_and(|stored| stored.sketch.has_linear_suppression())
}

/// The token a save of `board` writes, from its content, highest need
/// first: a linear pattern's suppression list needs
/// [`SNXPCB_FORMAT_V1_13`]; a layout plan, a length-matching group, a
/// teardrop rule or a shaped pour (corners or hatch)
/// [`SNXPCB_FORMAT_V1_12`]; a pour cutout, a
/// dimension look, drawn copper or an unfilled pour
/// [`SNXPCB_FORMAT_V1_11`],
/// stack materials [`SNXPCB_FORMAT_V1_10`], a shelved pour
/// [`SNXPCB_FORMAT_V1_9`], a stackup name, a
/// patterned baked target or a numbering sweep [`SNXPCB_FORMAT_V1_8`], the
/// board sketch targets [`SNXPCB_FORMAT_BOARD_SKETCH_TARGETS`], rooms
/// [`SNXPCB_FORMAT_ROOMS`], layer colours [`SNXPCB_FORMAT_LAYER_COLORS`],
/// anything else [`SNXPCB_FORMAT_V1`]. A board whose last such item is gone
/// goes back to the lower token.
pub fn snxpcb_token(board: &PcbBoard) -> &'static str {
    let sketch = board.sketch.as_ref().map(|stored| &stored.sketch);
    let patterned = sketch.is_some_and(|s| s.repeats_baked_target() || s.has_numbering_sweep());
    let targets = sketch.is_some_and(|s| s.carries_board_targets() || s.needs_format_1_2());
    let materials = board
        .stackup
        .as_ref()
        .is_some_and(|s| s.has_materials_data());
    let cutout = board.zones.iter().any(|zone| zone.is_pour_cutout());
    let unfilled = board.zones.iter().any(|zone| zone.is_unfilled_pour());
    let look = super::board_uses_dimension_style(board);
    let shaped = board.zones.iter().any(|zone| zone.has_pour_shape());
    let options = &board.options;
    if needs_1_13(board) {
        SNXPCB_FORMAT_V1_13
    } else if shaped
        || !options.layout_plan.is_empty()
        || !options.length_groups.is_empty()
        || !options.teardrops.is_default()
    {
        SNXPCB_FORMAT_V1_12
    } else if cutout || look || unfilled || super::board_has_drawn_copper(board) {
        SNXPCB_FORMAT_V1_11
    } else if materials {
        SNXPCB_FORMAT_V1_10
    } else if board.zones.iter().any(|zone| zone.is_shelved_pour()) {
        SNXPCB_FORMAT_V1_9
    } else if patterned || board.stackup.as_ref().is_some_and(|s| s.name.is_some()) {
        SNXPCB_FORMAT_V1_8
    } else if targets {
        SNXPCB_FORMAT_BOARD_SKETCH_TARGETS
    } else if !board.rooms.is_empty() {
        SNXPCB_FORMAT_ROOMS
    } else if !board.options.layer_colors.is_empty() {
        SNXPCB_FORMAT_LAYER_COLORS
    } else {
        SNXPCB_FORMAT_V1
    }
}

/// `.snxpcb` version policy: a newer minor opens read-only. A save writes
/// the lowest token its content needs ([`snxpcb_token`]).
pub const SNXPCB_VERSION: FormatVersion = FormatVersion {
    current: SNXPCB_FORMAT_V1_13,
    readable: &[
        "snxpcb/1",
        "snxpcb/1.1",
        "snxpcb/1.2",
        "snxpcb/1.3",
        SNXPCB_FORMAT_V1,
        SNXPCB_FORMAT_LAYER_COLORS,
        SNXPCB_FORMAT_ROOMS,
        SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
        SNXPCB_FORMAT_V1_8,
        SNXPCB_FORMAT_V1_9,
        SNXPCB_FORMAT_V1_10,
        SNXPCB_FORMAT_V1_11,
        SNXPCB_FORMAT_V1_12,
        SNXPCB_FORMAT_V1_13,
    ],
    newer_minor: NewerMinor::ReadOnly,
    missing: MissingToken::Refused,
};

/// [`FormatVersion::read`] for `.snxpcb`.
pub fn snxpcb_read(found: Option<&str>) -> Result<String, FormatVersionError> {
    SNXPCB_VERSION.read(found)
}

/// True when a board read as `token` opens read-only: a newer minor.
pub fn snxpcb_opens_read_only(token: &str) -> bool {
    SNXPCB_VERSION.is_newer_minor(token)
}

/// [`FormatVersion::refuse_save`] for `.snxpcb`.
pub fn snxpcb_refuse_save(source: &str) -> Result<(), FormatVersionError> {
    SNXPCB_VERSION.refuse_save(source)
}
