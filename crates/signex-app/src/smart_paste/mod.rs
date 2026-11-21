//! Smart Paste dialog (mockup row 155): state and messages (`state`), what the paste
//! places (`plan`, pure) and the view (`view`). The handler that opens the
//! dialog and puts the copies on the cursor lives in
//! `app/handlers/clipboard_workflows.rs`; the copies are placed by the
//! paste session's click, as one undo step.

pub mod plan;
mod state;
mod view;

pub use plan::{DesignatorChoice, NetChoice, PlaceMode};
pub use state::{ClipboardSummary, DIALOG_SIZE, PreviewShape, SmartPasteMsg, SmartPasteState};
pub use view::view;
