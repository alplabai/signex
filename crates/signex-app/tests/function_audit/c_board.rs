//! Function audit, wave C: the board outputs, mockup rows 109 (3D view), 111
//! (Export PDF), 112 (BOM export), 115 (Fabrication Outputs), 116 (Board
//! Print / PDF) and 117 (Board Reports, with its Net Lengths state). Each
//! row's controls are driven through `Signex::update` and checked by what
//! they change.

#[path = "c_board/bom.rs"]
mod bom;
#[path = "c_board/fab.rs"]
mod fab;
#[path = "c_board/pdf.rs"]
mod pdf;
#[path = "c_board/print.rs"]
mod print;
#[path = "c_board/reports.rs"]
mod reports;
#[path = "c_board/support.rs"]
mod support;
#[path = "c_board/view_3d.rs"]
mod view_3d;
