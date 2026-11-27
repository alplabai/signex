//! A footprint pad's own thermal relief values typed in the pad Properties
//! (#2593, mockup row 125): Air gap, Spoke width and Spokes, each Rule or
//! Manual. Text no pour can use stays in the field with its reason and the
//! pad keeps the stored value, as the other pad fields do (`pad_input`).

use signex_types::pad_thermal::PadThermal;

use crate::app::Signex;
use crate::library::editor::footprint::PadThermalField;

impl Signex {
    /// `which` of the selected pad `idx`, or of the next pad placed when
    /// `None`, from the typed `value`: empty follows the pour, a usable value
    /// is stored, other text is refused and kept. One undo step per field.
    pub(in crate::app::handlers::dock::sch_library) fn fp_pad_thermal_typed(
        &mut self,
        idx: Option<usize>,
        which: PadThermalField,
        value: &str,
    ) -> bool {
        let field = which.field();
        let Some(typed) = which.typed(PadThermal::RULE, value) else {
            self.pad_field_refused(idx, field, value);
            return true;
        };
        self.pad_field_typed(idx, field, None);
        match idx {
            Some(idx) => {
                self.with_selected_pad_field(idx, Some(field), move |pad| {
                    which.copy(&typed, &mut pad.stack.thermal)
                });
            }
            None => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    which.copy(&typed, &mut editor.state.next_pad_defaults.stack.thermal);
                }
                self.refresh_panel_ctx();
            }
        }
        true
    }
}
