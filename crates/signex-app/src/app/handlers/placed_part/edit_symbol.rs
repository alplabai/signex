//! Edit Symbol (#1262): open the placed part's source symbol in the Symbol
//! Editor through the same tab-open path the project tree uses. Saving it
//! raises the library-drift check (`refresh_primitive_cache_for`), which is
//! what offers to refresh the placed instances.

use iced::Task;
use signex_library::PrimitiveRef;

use super::{PlacedPart, part_name};
use crate::app::contracts::Message;
use crate::app::state::Signex;
use crate::library::drift::split_lib_id;

impl Signex {
    pub(super) fn edit_part_symbol(&mut self, part: PlacedPart) -> Task<Message> {
        let name = part_name(&part.symbol);
        let Some((library_id, symbol_id)) = split_lib_id(&part.symbol.lib_id) else {
            crate::diagnostics::log_warning(format!(
                "Place {name} from a library part to edit its symbol: it has no library symbol."
            ));
            return Task::none();
        };
        let reference = PrimitiveRef::new(library_id, symbol_id);
        let Some(path) = self.library.primitive_file(&reference, "symbols", "snxsym") else {
            crate::diagnostics::log_warning(format!(
                "Open the library of {name} to edit its symbol: that library is not open."
            ));
            return Task::none();
        };
        self.handle_open_primitive(path)
    }
}
