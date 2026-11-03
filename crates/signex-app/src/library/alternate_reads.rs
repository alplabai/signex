//! Adding an alternate symbol to a Component Preview row (#1230) needs the
//! row's default symbol and the picked one, to compare their pin numbers.
//! Resolving them reads library files, so it runs on the blocking pool behind
//! the library's write order (#859, #1756) and is applied only to the row it
//! was planned for, as that row still is when the answer lands.

use signex_library::{PrimitiveRef, Symbol};
use uuid::Uuid;

use super::shared_resolver::{OneShotCell, SharedResolver};
use super::state::{EditorAddress, LibraryState};

/// What [`LibraryState::plan_alternate_symbol`] decided to look up.
pub struct AlternatePlan {
    address: EditorAddress,
    library_id: Uuid,
    root: std::path::PathBuf,
    generation: u64,
    default_ref: PrimitiveRef,
    picked: PrimitiveRef,
    resolver: SharedResolver,
    /// The preview's `edit_epoch` when planned: an edit made while the
    /// symbols were read means the answer describes a row that is gone.
    epoch_at_plan: u64,
}

impl AlternatePlan {
    /// Resolve both symbols; safe on any thread.
    pub fn read(self) -> AlternateOutcome {
        AlternateOutcome {
            default: resolve(&self.resolver, &self.default_ref, "the default symbol"),
            alternate: resolve(&self.resolver, &self.picked, "the picked symbol"),
            address: self.address,
            library_id: self.library_id,
            generation: self.generation,
            picked: self.picked,
            epoch_at_plan: self.epoch_at_plan,
            default_ref: self.default_ref,
        }
    }
}

/// The answers of one [`AlternatePlan::read`], not yet applied.
pub struct AlternateOutcome {
    address: EditorAddress,
    library_id: Uuid,
    generation: u64,
    picked: PrimitiveRef,
    epoch_at_plan: u64,
    /// The default symbol the pin check ran against.
    default_ref: PrimitiveRef,
    default: Result<Symbol, String>,
    alternate: Result<Symbol, String>,
}

/// The symbol `reference` names, or why it could not be read.
fn resolve(
    resolver: &SharedResolver,
    reference: &PrimitiveRef,
    what: &str,
) -> Result<Symbol, String> {
    match resolver.resolve_symbol(reference) {
        Ok(Some(symbol)) => Ok(symbol),
        Ok(None) => Err(format!(
            "{what} is not in any open library, so pin compatibility cannot be checked"
        )),
        Err(e) => Err(format!("{what} could not be read ({e})")),
    }
}

impl LibraryState {
    /// Plan adding `picked` as an alternate of the row open at `address`.
    /// `None` when no such preview (or its library) is open.
    pub fn plan_alternate_symbol(
        &mut self,
        address: &EditorAddress,
        picked: PrimitiveRef,
    ) -> Option<AlternatePlan> {
        let (library_id, root) = self
            .library_at(&address.library_path)
            .map(|lib| (lib.library_id, lib.root.clone()))?;
        let preview = self.editors.get(address)?;
        let (default_ref, epoch_at_plan) = (preview.row.symbol_ref, preview.edit_epoch);
        self.refresh_generation_counter += 1;
        let generation = self.refresh_generation_counter;
        self.alternate_generations
            .insert(address.clone(), generation);
        Some(AlternatePlan {
            address: address.clone(),
            library_id,
            root,
            generation,
            default_ref,
            picked,
            resolver: self.shared_resolver(),
            epoch_at_plan,
        })
    }

    /// Add the alternate the worker checked, unless the preview changed while
    /// it ran. A refusal (an unresolved symbol, a pin-number mismatch, a
    /// repeat) is shown in the editor and logged to the Messages panel; the
    /// row is left as it was.
    pub fn apply_alternate_symbol(&mut self, outcome: AlternateOutcome) {
        let AlternateOutcome {
            address,
            library_id,
            generation,
            picked,
            epoch_at_plan,
            default_ref,
            default,
            alternate,
        } = outcome;
        let same_mount = self
            .library_at(&address.library_path)
            .is_some_and(|lib| lib.library_id == library_id);
        if !same_mount {
            crate::diagnostics::log_warning(
                "Alternate symbol not added: the library was closed or replaced while the symbols were read",
            );
            return;
        }
        // Closing a preview forgets its generation too, so the closed preview
        // is looked for first or the pick would vanish without a word.
        if !self.editors.contains_key(&address) {
            self.alternate_generations.remove(&address);
            crate::diagnostics::log_warning(
                "Alternate symbol not added: the component preview was closed while the symbols were read",
            );
            return;
        }
        // A newer pick superseded this one: it owns the outcome.
        if self.alternate_generations.get(&address) != Some(&generation) {
            return;
        }
        self.alternate_generations.remove(&address);
        let Some(state) = self.editors.get_mut(&address) else {
            return;
        };
        let pn = state.row.internal_pn.to_string();
        let outcome = if state.edit_epoch != epoch_at_plan || state.row.symbol_ref != default_ref {
            // The pin check ran against a row that has since changed.
            Err(
                "this row was edited while the symbols were read; pick the alternate again"
                    .to_string(),
            )
        } else {
            match (default, alternate) {
                (Ok(default), Ok(alternate)) => signex_library::add_alternate_symbol(
                    &mut state.row,
                    &default,
                    picked,
                    &alternate,
                )
                .map_err(|e| e.to_string()),
                (Err(why), _) | (_, Err(why)) => Err(why),
            }
        };
        match outcome {
            Ok(()) => {
                state.alternate_error = None;
                state.mark_dirty();
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Alternate symbol not added to {pn}: {why}."
                ));
                state.alternate_error = Some(why);
            }
        }
    }
}

/// Look up the symbols of an alternate pick off the UI thread.
pub fn alternate_symbol_task(
    state: &mut LibraryState,
    address: &EditorAddress,
    picked: PrimitiveRef,
) -> iced::Task<crate::app::Message> {
    let Some(plan) = state.plan_alternate_symbol(address, picked) else {
        return iced::Task::none();
    };
    let turn = state.writes.read_turn(&plan.root);
    iced::Task::perform(
        async move {
            let outcome = tokio::task::spawn_blocking(move || {
                turn.wait();
                plan.read()
            })
            .await
            .ok();
            OneShotCell::new(outcome)
        },
        |readout| {
            crate::app::Message::Library(super::LibraryMessage::AlternateSymbolResolved { readout })
        },
    )
}
