//! Rendering for the schematic canvas, split by draw layer (bottom-to-top):
//! background, scene (content + auto-focus dim + selection), then the Layer-4
//! overlay broken into previews, ghosts, drag guides, and cursor HUD.
//!
//! Each file adds `impl SchematicCanvas` methods; `canvas::Program::draw`
//! assembles them in the original z-order.

mod background;
mod drag;
mod ghosts;
mod overlay;
mod previews;
mod scene;
