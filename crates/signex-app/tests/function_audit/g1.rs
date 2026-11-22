//! Function audit, wave A batch g1: the schematic Properties panel, mockup
//! rows 40 to 51. Each row's controls are driven through `Signex::update`
//! and checked by what they change in the document.

#[path = "g1/directive.rs"]
mod directive;
#[path = "g1/document.rs"]
mod document;
#[path = "g1/drawing.rs"]
mod drawing;
#[path = "g1/multi.rs"]
mod multi;
#[path = "g1/net.rs"]
mod net;
#[path = "g1/part.rs"]
mod part;
#[path = "g1/placement.rs"]
mod placement;
#[path = "g1/sheet_symbol.rs"]
mod sheet_symbol;
#[path = "g1/support.rs"]
mod support;
#[path = "g1/text_note.rs"]
mod text_note;
