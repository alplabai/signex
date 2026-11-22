//! Function audit, wave B: the library rows m118 to m123 and m127 to m139,
//! with Variant Manager (m32) and the planned panels (m60). Each row's
//! controls are driven through `Signex::update` and checked by what they
//! change in the library, the document and the files on disk.

#[path = "b_library/browser.rs"]
mod browser;
#[path = "b_library/closing.rs"]
mod closing;
#[path = "b_library/dialogs.rs"]
mod dialogs;
#[path = "b_library/pickers.rs"]
mod pickers;
#[path = "b_library/planned.rs"]
mod planned;
#[path = "b_library/rows.rs"]
mod rows;
#[path = "b_library/support.rs"]
mod support;
#[path = "b_library/symbols.rs"]
mod symbols;
#[path = "b_library/variants.rs"]
mod variants;
