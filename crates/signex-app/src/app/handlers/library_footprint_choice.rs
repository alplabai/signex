//! Switching a placed library part between the footprints its row offers
//! (#1234): the default and the alternates.
//!
//! `PanelMsg::SwitchFootprintChoice` validates the pick against the open
//! library's row and applies it as ONE engine batch (a command per placed
//! unit of the part), so a single undo restores the previous footprint. The
//! footprint's name comes from the library's cached listing, so `update`
//! reads no file.

use iced::Task;
use uuid::Uuid;

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::drift::split_lib_id;
use crate::library::place::lib_id_of;
use crate::panels::FootprintChoiceItem;
use signex_library::primitive::PrimitiveRef;

impl Signex {
    /// The footprints the placed library part's row offers, for the
    /// Properties pick list. Empty unless the row has alternates.
    ///
    /// The part's own state decides what is selected: the binding in its
    /// value stamp (the default when it has none). A footprint name typed
    /// over it shows as an "(edited)" entry, and a binding the row dropped as
    /// "(no longer offered)", so the list never claims more than the part holds.
    pub(crate) fn footprint_choices_for(
        &self,
        symbol: &signex_types::schematic::Symbol,
    ) -> Vec<FootprintChoiceItem> {
        let Some((_, _, row)) = self.row_of_placed_symbol(symbol) else {
            return Vec::new();
        };
        let bindings = row.footprint_bindings();
        if bindings.len() < 2 {
            return Vec::new();
        }
        let current = chosen_binding(symbol).or(row.footprint_ref);
        let mut items: Vec<FootprintChoiceItem> = bindings
            .iter()
            .map(|b| FootprintChoiceItem {
                binding: lib_id_of(&b.footprint),
                label: self.footprint_label(&b.footprint),
                selected: Some(b.footprint) == current,
            })
            .collect();
        let selected = items.iter().position(|i| i.selected);
        match (selected, current) {
            (None, Some(gone)) => items.push(FootprintChoiceItem {
                binding: lib_id_of(&gone),
                label: signex_library::NO_LONGER_OFFERED_LABEL.to_string(),
                selected: true,
            }),
            (Some(at), Some(reference)) if self.footprint_typed_over(symbol, &reference) => {
                let held = symbol.footprint.trim();
                items[at].selected = false;
                items.push(FootprintChoiceItem {
                    binding: String::new(),
                    label: format!("{held} (edited)"),
                    selected: true,
                });
            }
            _ => {}
        }
        items
    }

    /// Whether the part's own footprint text was typed over the footprint
    /// `reference` names. Only a name the listing knows can be told apart
    /// from the part's own text.
    pub(crate) fn footprint_typed_over(
        &self,
        symbol: &signex_types::schematic::Symbol,
        reference: &PrimitiveRef,
    ) -> bool {
        let held = symbol.footprint.trim();
        self.library
            .footprint_name(reference)
            .is_some_and(|name| !held.is_empty() && held != name)
    }

    /// The library footprint the placed part is built with: the binding in
    /// its value stamp, else its row's default. `None` when the part has no
    /// open library row or the row binds no footprint.
    pub(crate) fn bound_footprint_of(
        &self,
        symbol: &signex_types::schematic::Symbol,
    ) -> Option<PrimitiveRef> {
        let (_, _, row) = self.row_of_placed_symbol(symbol)?;
        chosen_binding(symbol).or(row.footprint_ref)
    }

    /// Build `symbol_id` with the footprint `binding` names.
    pub(crate) fn handle_switch_footprint_choice(
        &mut self,
        symbol_id: Uuid,
        binding: &str,
    ) -> Task<Message> {
        let Some(doc) = self.document_state.active_engine().map(|e| e.document()) else {
            return Task::none();
        };
        // The "(edited)" entry has no binding of its own: picking it is a no-op.
        if split_lib_id(binding).is_none() || !doc.symbols.iter().any(|s| s.uuid == symbol_id) {
            return Task::none();
        }
        match self.footprint_binding_commands(doc, symbol_id, binding) {
            Ok(commands) => {
                // An engine error is already logged by the gateway.
                self.apply_engine_commands(commands, true, true);
            }
            Err(FootprintChoiceRefusal { reference, why }) => {
                self.report_footprint_choice_failure(&reference, why);
            }
        }
        Task::none()
    }

    /// The commands that build the part `symbol_id` of `doc` (every placed
    /// unit) with the footprint `binding` names, checked against the part's
    /// own library row: only the row's default and alternates are accepted.
    /// Properties and Tools ▸ Assign Footprints both go through here.
    pub(crate) fn footprint_binding_commands(
        &self,
        doc: &signex_types::schematic::SchematicSheet,
        symbol_id: Uuid,
        binding: &str,
    ) -> Result<Vec<signex_engine::Command>, FootprintChoiceRefusal> {
        let symbol =
            doc.symbols
                .iter()
                .find(|s| s.uuid == symbol_id)
                .ok_or(FootprintChoiceRefusal {
                    reference: String::new(),
                    why: "the part is no longer on its sheet",
                })?;
        let refuse = |why| FootprintChoiceRefusal {
            reference: symbol.reference.clone(),
            why,
        };
        let (library, footprint) =
            split_lib_id(binding).ok_or_else(|| refuse("the footprint is not a library one"))?;
        let chosen = PrimitiveRef::new(library, footprint);
        let (_, _, row) = self
            .row_of_placed_symbol(symbol)
            .ok_or_else(|| refuse("its library row is not open"))?;
        if !row.binds_footprint(&chosen) {
            return Err(refuse("its row does not offer it"));
        }
        let is_default = row.footprint_ref == Some(chosen);
        let name = self
            .library
            .footprint_name(&chosen)
            .ok_or_else(|| refuse("the library does not list its name; refresh the library"))?;
        let binding = (!is_default).then(|| lib_id_of(&chosen));
        Ok(signex_engine::placed_unit_ids(doc, symbol_id)
            .into_iter()
            .map(
                |symbol_id| signex_engine::Command::SetSymbolFootprintBinding {
                    symbol_id,
                    footprint: name.clone(),
                    binding: binding.clone(),
                },
            )
            .collect())
    }

    /// The footprint's listed name, else a short id.
    fn footprint_label(&self, reference: &PrimitiveRef) -> String {
        self.library.footprint_name(reference).unwrap_or_else(|| {
            let id = reference.uuid.simple().to_string();
            format!("Footprint {}", &id[..8])
        })
    }

    fn report_footprint_choice_failure(&self, reference: &str, detail: &str) {
        crate::diagnostics::log_warning(format!(
            "Could not switch {reference} to another footprint: {detail}."
        ));
    }
}

/// Why a footprint choice was refused, and for which part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FootprintChoiceRefusal {
    pub(crate) reference: String,
    pub(crate) why: &'static str,
}

/// The alternate footprint a part is built with, from its valid stamp; `None`
/// for the row's default.
fn chosen_binding(symbol: &signex_types::schematic::Symbol) -> Option<PrimitiveRef> {
    let key = symbol.valid_stamp()?.footprint_ref.as_deref()?;
    let (library, footprint) = split_lib_id(key)?;
    Some(PrimitiveRef::new(library, footprint))
}
