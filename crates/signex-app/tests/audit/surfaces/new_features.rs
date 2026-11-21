//! Mockup groups `new-*` (rows m151-m193f), one file for all of them.
//!
//! Still to add:
//! - new-sch: m151 Multi-channel, m152 Signal harnesses, m153 Net
//!   directives, m154 Compiled Design, m155 Change Component + Smart Paste,
//!   m156 Schematic tables + table of contents, m157 Bezier + dimension
//!   annotation, m158 Named unions.
//! - new-release: m161 Command Log, m162 Compare Board with Schematic, m163
//!   Rooms from Sheets, m164 PCB Status-bar Counters, m165 Annotations, m166
//!   Workspace Layouts, m167 Export Job naming, m168 PDF options: watermark,
//!   layers, bookmarks.
//! - new-advanced: m171 Block Diagram + Power Tree, m172 Simulation View,
//!   m173 Plugin Manager, m174 Render Manager, m175 3D View Toolbar Extras,
//!   m176 Board Materials, m177 Layer Stack Material Library.
//! - new-pour: m178 Polygon Manager.
//! - new-boardsketch: m179 Board sketch mode (full sketcher), m179b
//!   References to board features, m179c Import DXF into the sketch, m179d
//!   Outline checks, m179e Mechanical constraints, m179f Edit in place with
//!   a live 3D outline.
//! - new-pcbsim: m181 Simulation setup wizard, m182 DC IR drop results, m183
//!   Thermal results, m184 Signal integrity, m185 PDN impedance, m186
//!   Antenna & RF.
//! - new-fpsketch: m187 Footprint sketch mode workspace, m188 Constraints
//!   and snap glyphs, m189 Sketch parameters table, m190 Bake: roles,
//!   preview, errors, result, m191 Mirror (B1) and patterns, m192 Pads <->
//!   Sketch.
//! - new-gerber: m193 Gerber Viewer tab, m193b Opened from an Export Job
//!   run, m193c Measure, m193d Compare with board, m193e Read errors, m193f
//!   Empty state.

use crate::surface::Surface;

pub fn rows() -> Vec<Surface> {
    vec![]
}
