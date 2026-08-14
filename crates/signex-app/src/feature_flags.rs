//! Compile-time constants for shipping incomplete subsystems dark.
//!
//! These are plain `const bool`s, not Cargo features — the code stays
//! compiled (so it can't bit-rot) while its user-facing surface is held
//! back.
//!
//! **They are not all the same kind of switch, and the difference decides
//! whether editing one does anything.**
//!
//! - A **gate** is read at the entry points themselves, so flipping it is
//!   the whole change. [`FOOTPRINT_EDITOR_ENABLED`] is one: four call sites
//!   branch on it to hide the tab, the create flow and the menu items.
//! - A **default** is only the fallback for a preference the user can set.
//!   Flipping it changes what a *fresh* profile starts with and nothing
//!   else — anyone whose `prefs.json` already carries the key keeps their
//!   value. [`PCB_GPU_RENDER_DEFAULT`] is one.
//!
//! This doc used to say every const here gated its feature's entry points
//! and that flipping one required "no other change". That was true of the
//! gate and false of the default — the kind of mistake that gets made once
//! under pressure, reaching for a const to turn a feature off for everyone
//! and shipping a release where it is still on.

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

/// **Factory default, not a gate** — see the module docs.
///
/// Route the PCB editor canvas content through the GPU (`signex_gfx`
/// pipelines via iced's shader widget, [`crate::scene_shader`]) instead of
/// CPU `canvas::Frame` tessellation. When ON, the PCB view mounts a `stack!`
/// of an opaque background+grid `canvas` beneath a `shader` that draws the
/// traces/pads/vias/zones on the GPU, while pan/zoom/cursor/fit stay on the
/// CPU canvas layer (events fall through the non-capturing shader). See
/// [`crate::pcb_canvas`].
///
/// The only reader is
/// [`crate::fonts::read_pcb_gpu_render_pref_at`], as the `unwrap_or` of the
/// `prefs.json` key `"pcb_gpu_render"`. The feature itself is reachable from
/// **Preferences ▸ Appearance ▸ PCB Editor ▸ GPU Render (experimental)**,
/// deliberately — it is how the path gets exercised before parity is signed
/// off. So:
///
/// - Flipping this to `true` only changes what a profile with no saved value
///   starts with.
/// - Flipping it back to `false` **does not turn the feature off** for anyone
///   who has ticked the box; their saved `true` wins. There is no kill
///   switch today. If one is ever needed, it has to be a real gate read where
///   the shader is mounted, not an edit here.
///
/// Still `false` because GPU visual parity is unconfirmed on hardware
/// (background clear colour, ortho Y-orientation) and the base-bucket draw
/// order deliberately diverges from the CPU path — see
/// `signex_gfx::scene::order` and issue #645.
pub const PCB_GPU_RENDER_DEFAULT: bool = false;
