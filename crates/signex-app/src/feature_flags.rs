//! Compile-time feature gates for shipping incomplete subsystems dark.
//!
//! These are plain `const bool`s, not Cargo features — the code stays
//! compiled (so it can't bit-rot) but its user-facing entry points are
//! gated behind the flag. Flip a flag to `true` to light the feature
//! back up; no other change is required.

/// Footprint / sketch editor master switch.
///
/// **v0.13.0 ("Symbol & Library") ships with this OFF.** The footprint
/// editor (`.snxfpt` pad/sketch canvas) is feature-incomplete and was
/// under heavy daily churn through v0.27; it is hidden for this release
/// so users land on the polished symbol + library surfaces instead.
///
/// What the gate turns off:
/// - opening a `.snxfpt` as an editable `TabKind::FootprintEditor` tab
///   ([`crate::app::Signex::handle_open_primitive`]);
/// - the "New Footprint / PCB Library" create flow
///   ([`crate::app::Signex::add_project_footprint_library`]);
/// - the matching command-palette entry and project-tree menu items.
///
/// What stays available (footprints are still first-class *data*):
/// - read-only footprint preview in the Component Preview tab;
/// - Pick Footprint binding of existing `.snxfpt` files into rows;
/// - the footprint column in the Library Browser;
/// - the bake / library backend.
///
/// Flip to `true` when the editor is ready; the
/// `opening_snxfpt_does_not_create_editable_tab_when_gated` regression
/// test is written to flip with it.
pub const FOOTPRINT_EDITOR_ENABLED: bool = false;
