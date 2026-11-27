//! The widths the canvas hits and boxes its items at, with this sheet's
//! special strings and the active variant's values: a value field is boxed
//! on the text it is drawn with (#1986, #2948), `20kΩ` rather than the stored
//! `= 2 * R1.Value`, the variant's `47k` rather than the sheet's `10k`.

use crate::schematic_runtime::WireWidths;

use super::SchematicCanvas;

impl SchematicCanvas<'_> {
    /// [`super::CanvasViewPrefs::wire_widths`] with the sheet's special
    /// strings and the active variant's values.
    pub(super) fn wire_widths(&self) -> WireWidths<'_> {
        WireWidths {
            special_strings: self.slot.special_strings.as_ref(),
            variant_values: Some(&self.slot.variant_values),
            ..self.prefs.wire_widths()
        }
    }
}
