//! The widths the canvas hits and boxes its items at, with this sheet's
//! special strings: a value field is boxed on the text it is drawn with
//! (#1986), `20kΩ` rather than the stored `= 2 * R1.Value`.

use crate::schematic_runtime::WireWidths;

use super::SchematicCanvas;

impl SchematicCanvas<'_> {
    /// [`super::CanvasViewPrefs::wire_widths`] with the sheet's special
    /// strings.
    pub(super) fn wire_widths(&self) -> WireWidths<'_> {
        WireWidths {
            special_strings: self.slot.special_strings.as_ref(),
            ..self.prefs.wire_widths()
        }
    }
}
