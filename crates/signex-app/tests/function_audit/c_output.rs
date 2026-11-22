//! Function audit, wave C: the output and workspace rows m161 to m168 and
//! m175 (Command Log, Compare Board with Schematic, Rooms, the board status
//! counters, Annotations, Workspace Layouts, the Export Job name, the PDF
//! options and the 3D toolbar). Each row's controls are driven through
//! `Signex::update` and checked by what they change.

#[path = "c_output/annotations.rs"]
mod annotations;
#[path = "c_output/command_log.rs"]
mod command_log;
#[path = "c_output/compare.rs"]
mod compare;
#[path = "c_output/layouts.rs"]
mod layouts;
#[path = "c_output/pdf.rs"]
mod pdf;
#[path = "c_output/rooms.rs"]
mod rooms;
#[path = "c_output/status.rs"]
mod status;
#[path = "c_output/support.rs"]
mod support;
#[path = "c_output/view3d.rs"]
mod view3d;
