//! Compile-time feature gates for shipping incomplete subsystems dark.
//!
//! These are plain `const bool`s, not Cargo features — the code stays
//! compiled (so it can't bit-rot) but its user-facing entry points are
//! gated behind the flag. Flip a flag to `true` to light the feature
//! back up; no other change is required.

/// Footprint / sketch editor master switch.
///
/// **Enabled as of v0.14.0 ("Footprint Editor").** v0.13.0 shipped this
/// OFF while the `.snxfpt` pad/sketch editor was finished; v0.14 wires
/// the remaining active-bar tools (Align/Distribute, Move/Drag,
/// Fill/Region, Text Frame, selection-filter All toggle), exposes the
/// full sketch-constraint set, and fixes the pad shape-param bug — so
/// the editor is now reachable.
///
/// When ON, this enables:
/// - opening a `.snxfpt` as an editable `TabKind::FootprintEditor` tab
///   ([`crate::app::Signex::handle_open_primitive`]);
/// - the "New Footprint / PCB Library" create flow
///   ([`crate::app::Signex::add_project_footprint_library`]);
/// - the matching command-palette entry and project-tree menu items.
///
/// Set back to `false` to ship the editor dark again; the
/// `opening_snxfpt_does_not_create_editable_tab_when_gated` regression
/// test branches on this flag so it stays valid either way.
pub const FOOTPRINT_EDITOR_ENABLED: bool = true;
