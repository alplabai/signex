//! The `.snxsch` format tokens and the rule that picks the one a save writes
//! (`docs/FORMAT_VERSIONS.md`).

use crate::format_version::{FormatVersion, MissingToken, NewerMinor};
use crate::schematic::{SchDirective, SchDrawing, SchematicSheet};

/// The `.snxsch` token a sheet without No-ERC rule lists writes; the policy
/// is [`SNXSCH_VERSION`] (`docs/FORMAT_VERSIONS.md`). 1.1 (#2209):
/// `lib_symbols` is keyed by body ref (`<lib_id>#<hash>`) and a symbol names
/// its body in `body_ref`. A 1.0 build reads that table by `lib_id` and would
/// show parts without pins, so 1.0 builds open 1.1 files read-only
/// (`is_newer_minor`) instead.
pub const SNXSCH_FORMAT_V1: &str = "snxsch/1.1";

/// Written instead of [`SNXSCH_FORMAT_V1`] when a No-ERC directive names the
/// rules it silences (#1699). A 1.1 build would drop the list on its next
/// save and the marker would silence every rule, so it opens 1.2 read-only.
/// A sheet without lists stays 1.1, byte for byte.
pub const SNXSCH_FORMAT_NO_ERC_RULES: &str = "snxsch/1.2";

/// Written when the sheet holds Schematic Refinement content: a harness
/// object, a union, a Bezier, dimension or table drawing, a Net Parameters
/// directive, or a Blanket or Differential Pair directive using a field
/// added with them. A 1.2 build would drop it on its next save, so it opens
/// 1.3 read-only. Format: `docs/architecture/snxsch-refinement-objects.md`.
pub const SNXSCH_FORMAT_REFINEMENT: &str = "snxsch/1.3";

/// Written when a bus stores its own width or colour (#2813). A 1.3 build
/// drops unknown keys of a typed bus row on save, so it opens 1.4 read-only.
pub const SNXSCH_FORMAT_BUS_STYLE: &str = "snxsch/1.4";

/// `.snxsch` version policy: a newer minor opens read-only. A save writes the
/// lowest token its content needs ([`snxsch_token`]).
pub const SNXSCH_VERSION: FormatVersion = FormatVersion {
    current: SNXSCH_FORMAT_BUS_STYLE,
    readable: &[
        "snxsch/1",
        SNXSCH_FORMAT_V1,
        SNXSCH_FORMAT_NO_ERC_RULES,
        SNXSCH_FORMAT_REFINEMENT,
        SNXSCH_FORMAT_BUS_STYLE,
    ],
    newer_minor: NewerMinor::ReadOnly,
    missing: MissingToken::Refused,
};

/// The token a save of `sheet` writes: [`SNXSCH_FORMAT_BUS_STYLE`] when a bus
/// has its own style, else [`SNXSCH_FORMAT_REFINEMENT`] with Schematic
/// Refinement content, else [`SNXSCH_FORMAT_NO_ERC_RULES`] when a No-ERC
/// directive names rules, else [`SNXSCH_FORMAT_V1`]. From content, not the
/// token the file loaded with, so a sheet whose last such object is removed
/// goes back to the lower token.
pub fn snxsch_token(sheet: &SchematicSheet) -> &'static str {
    if sheet.buses.iter().any(crate::schematic::Bus::has_own_style) {
        SNXSCH_FORMAT_BUS_STYLE
    } else if needs_refinement(sheet) {
        SNXSCH_FORMAT_REFINEMENT
    } else if sheet
        .no_erc_directives
        .iter()
        .any(|directive| !directive.rules.is_empty())
    {
        SNXSCH_FORMAT_NO_ERC_RULES
    } else {
        SNXSCH_FORMAT_V1
    }
}

/// Whether the save writes Schematic Refinement content. An entry type whose
/// sheet pin is gone is not written, so it does not count.
fn needs_refinement(sheet: &SchematicSheet) -> bool {
    !sheet.harness.connectors.is_empty()
        || !sheet.harness.wires.is_empty()
        || !super::refinement_extras::entry_types_for_write(sheet).is_empty()
        || !sheet.unions.is_empty()
        || sheet.drawings.iter().any(SchDrawing::needs_format_1_3)
        || sheet.directives.iter().any(SchDirective::needs_format_1_3)
}
