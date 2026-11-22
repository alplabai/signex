//! Function audit, wave B: mockup rows m19 to m38 (Projects panel and menu,
//! New Project, New PCB / Schematic, Rename, Remove, Close Project, Enable
//! Version Control, Create Sheet, Netlist incomplete and every Project tab
//! page). Each control is driven through `Signex::update` on a project opened
//! from disk and checked by what it changes.

#[path = "b_project/child_netlist.rs"]
mod child_netlist;
#[path = "b_project/close_vc.rs"]
mod close_vc;
#[path = "b_project/new_documents.rs"]
mod new_documents;
#[path = "b_project/panel.rs"]
mod panel;
#[path = "b_project/rename_remove.rs"]
mod rename_remove;
#[path = "b_project/requirements.rs"]
mod requirements;
#[path = "b_project/support.rs"]
mod support;
#[path = "b_project/tab_defaults.rs"]
mod tab_defaults;
#[path = "b_project/tab_general.rs"]
mod tab_general;
#[path = "b_project/tab_lists.rs"]
mod tab_lists;
