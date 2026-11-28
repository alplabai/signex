//! Change Component's one dialog (mockup row 155): what the list compares
//! rows with, the Same symbol / Same value chips, the pin counts of the
//! listed rows (read off the update thread) and the Now / After table.
//! Pure helpers, then the handlers of the dialog's own messages.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use iced::Task;
use signex_engine::ComponentReplacement;
use signex_library::{ComponentRow, PrimitiveRef, RowPricing, TemplateRegistry, best_stock};
use signex_types::schematic::{LibSymbol, SchematicSheet, Symbol, normalized_lib_id, pin_key};
use uuid::Uuid;

use crate::app::contracts::Message;
use crate::app::state::Signex;
use crate::library::messages::{LibraryMessage, PlacedPartMsg, PlacedPartTarget};
use crate::library::place::lib_id_of;
use crate::library::state::{
    ChangeFilter, ChangeFilters, ChangeScope, ChangeSession, CompareField, CompareKind, CompareRow,
    OpenLibrary, PickerMode, PinMatch, PlacedNow,
};

/// The field a placement stamps the MPN under (`library::place`).
const MPN_FIELD: &str = "MPN";

/// Distinct pins of a drawing, over every unit and body style.
pub(super) fn distinct_pins(lib: &LibSymbol) -> usize {
    lib.pins.iter().map(pin_key).collect::<HashSet<_>>().len()
}

/// The parameter that is a class's value: the first required slot of its
/// template (Resistance of a resistor).
fn value_parameter(registry: &TemplateRegistry, library_id: Uuid, class: &str) -> Option<String> {
    registry
        .resolve(library_id, class)
        .and_then(|t| t.required_params.first())
        .map(|slot| slot.name.clone())
}

/// The row `symbol` was placed from, when its library is open.
fn placed_row<'a>(libs: &'a [OpenLibrary], symbol: &Symbol) -> Option<(Uuid, &'a ComponentRow)> {
    let (library_id, row_id) = (symbol.library_id?, symbol.row_id?);
    let lib = libs.iter().find(|l| l.library_id == library_id)?;
    lib.tables
        .values()
        .flatten()
        .find(|row| row.row_id == row_id)
        .map(|row| (library_id, row))
}

/// The placed part as the list compares rows with it.
pub(super) fn placed_now(
    doc: &SchematicSheet,
    symbol: &Symbol,
    libs: &[OpenLibrary],
    registry: &TemplateRegistry,
) -> PlacedNow {
    let mpn = symbol.fields.get(MPN_FIELD).map_or("", |s| s.trim());
    let value = placed_row(libs, symbol).and_then(|(library_id, row)| {
        let key = value_parameter(registry, library_id, row.class.as_str())?;
        let placed = symbol
            .fields
            .get(&key)
            .cloned()
            .or_else(|| row.parameters.get(&key).map(|v| v.display()))
            .filter(|v| !v.trim().is_empty())?;
        Some((key, placed))
    });
    PlacedNow {
        part: if mpn.is_empty() {
            symbol.value.clone()
        } else {
            mpn.to_string()
        },
        footprint: symbol.footprint.clone(),
        lib_id: normalized_lib_id(&symbol.lib_id).to_string(),
        value,
        pins: doc.body_of(symbol).map(distinct_pins),
    }
}

/// Whether `row` draws the placed part's symbol, as its default drawing or
/// an alternate.
fn draws_placed_symbol(row: &ComponentRow, placed: &PlacedNow) -> bool {
    std::iter::once(&row.symbol_ref)
        .chain(row.alternate_symbols.iter().map(|a| &a.symbol_ref))
        .any(|r| lib_id_of(r) == placed.lib_id)
}

/// Whether a distributor last reported `row` in stock: a current cached
/// price with a stock above zero, from any distributor.
fn in_stock(row: &ComponentRow, pricing: &HashMap<Uuid, RowPricing>) -> bool {
    pricing
        .get(&row.row_id)
        .and_then(|cached| best_stock(row, cached))
        .is_some_and(|n| n > 0)
}

/// Whether the switched-on chips leave `row` of `lib` out.
fn row_hidden(
    row: &ComponentRow,
    lib: &OpenLibrary,
    placed: &PlacedNow,
    filters: ChangeFilters,
    registry: &TemplateRegistry,
) -> bool {
    if filters.same_symbol && !draws_placed_symbol(row, placed) {
        return true;
    }
    if filters.in_stock && !in_stock(row, &lib.pricing) {
        return true;
    }
    let library_id = lib.library_id;
    if filters.same_value
        && let Some((key, value)) = &placed.value
    {
        let same_key = value_parameter(registry, library_id, row.class.as_str())
            .is_some_and(|own| own == *key);
        let same = same_key
            && row
                .parameters
                .get(key)
                .is_some_and(|v| v.display() == *value);
        return !same;
    }
    false
}

/// Every open row the chips leave out.
pub(super) fn hidden_rows(
    libs: &[OpenLibrary],
    registry: &TemplateRegistry,
    placed: &PlacedNow,
    filters: ChangeFilters,
) -> HashSet<Uuid> {
    libs.iter()
        .flat_map(|lib| {
            lib.tables
                .values()
                .flatten()
                .filter(|row| row_hidden(row, lib, placed, filters, registry))
                .map(|row| row.row_id)
        })
        .collect()
}

/// The Now / After table: value, footprint and datasheet, every field the
/// new part supplies, then the user parameters it does not.
pub(super) fn compare_rows(
    symbol: &Symbol,
    replacement: &ComponentReplacement,
    user_parameters: &[String],
) -> Vec<CompareRow> {
    let row = |field: CompareField, now: &str, after: &str| CompareRow {
        field,
        now: now.to_string(),
        after: after.to_string(),
        kind: if now == after {
            CompareKind::Same
        } else {
            CompareKind::Replaced
        },
    };
    let mut rows = vec![
        row(CompareField::Value, &symbol.value, &replacement.value),
        row(
            CompareField::Footprint,
            &symbol.footprint,
            &replacement.footprint,
        ),
    ];
    if !(symbol.datasheet.is_empty() && replacement.datasheet.is_empty()) {
        rows.push(row(
            CompareField::Datasheet,
            &symbol.datasheet,
            &replacement.datasheet,
        ));
    }
    for (key, after) in &replacement.fields {
        let now = symbol.fields.get(key).map_or("", String::as_str);
        if !(now.is_empty() && after.is_empty()) {
            rows.push(row(CompareField::Named(key.clone()), now, after));
        }
    }
    for name in user_parameters {
        rows.push(CompareRow {
            field: CompareField::Named(name.clone()),
            now: symbol.fields.get(name).cloned().unwrap_or_default(),
            after: String::new(),
            kind: CompareKind::User,
        });
    }
    rows
}

/// Pin counts known without a read, and the drawings still to read.
type PinReads = (Vec<(Uuid, usize)>, Vec<(Uuid, PrimitiveRef)>);

/// The rows whose pin counts must be read: their drawings, deduplicated.
/// Rows drawing the placed symbol take its count without a read.
fn pin_reads(libs: &[OpenLibrary], placed: &PlacedNow) -> PinReads {
    let (mut known, mut reads) = (Vec::new(), Vec::new());
    for row in libs.iter().flat_map(|l| l.tables.values().flatten()) {
        match placed.pins {
            Some(pins) if lib_id_of(&row.symbol_ref) == placed.lib_id => {
                known.push((row.row_id, pins));
            }
            _ => reads.push((row.row_id, row.symbol_ref)),
        }
    }
    (known, reads)
}

/// Read the drawings `reads` names and count their pins. A drawing that
/// cannot be read leaves its rows without a count (no tag).
fn read_pin_counts(
    reads: Vec<(Uuid, PrimitiveRef)>,
    roots: Vec<(Uuid, PathBuf)>,
) -> Vec<(Uuid, usize)> {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    let mut adapters: HashMap<Uuid, Option<LocalGitAdapter>> = HashMap::new();
    let mut counts: HashMap<PrimitiveRef, Option<usize>> = HashMap::new();
    let mut out = Vec::new();
    for (row_id, reference) in reads {
        let count = *counts.entry(reference).or_insert_with(|| {
            let adapter = adapters
                .entry(reference.library_id)
                .or_insert_with(|| {
                    let (_, root) = roots.iter().find(|(id, _)| *id == reference.library_id)?;
                    LocalGitAdapter::open_to_read(root).ok()
                })
                .as_ref()?;
            let symbol = adapter.get_symbol(reference.uuid).ok()?;
            Some(distinct_pins(&symbol.to_lib_symbol(lib_id_of(&reference))))
        });
        if let Some(count) = count {
            out.push((row_id, count));
        }
    }
    out
}

impl Signex {
    /// The session of a Change Component on `target`, and the read of the
    /// listed rows' pin counts.
    pub(super) fn change_session(
        &self,
        target: PlacedPartTarget,
        reference: String,
        doc: &SchematicSheet,
        symbol: &Symbol,
    ) -> (ChangeSession, Task<Message>) {
        let libs = &self.library.open_libraries;
        let registry = &self.library.template_registry;
        let mut session = ChangeSession::new(target.clone(), reference);
        session.placed = placed_now(doc, symbol, libs, registry);
        session.hidden = hidden_rows(libs, registry, &session.placed, session.filters);
        let (known, reads) = pin_reads(libs, &session.placed);
        session.row_pins.extend(known);
        if reads.is_empty() {
            return (session, Task::none());
        }
        let roots: Vec<(Uuid, PathBuf)> = libs
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_pin_counts(reads, roots))
                    .await
                    .map_err(|e| e.to_string())
            },
            move |counts| {
                Message::Library(LibraryMessage::PlacedPart(
                    PlacedPartMsg::ChangePinCountsRead {
                        target: target.clone(),
                        counts,
                    },
                ))
            },
        );
        (session, task)
    }

    /// The open Change Component's session, in either stage.
    fn change_session_mut(&mut self) -> Option<&mut ChangeSession> {
        match &mut self.library.picker.as_mut()?.mode {
            PickerMode::ChangeComponent(session) => Some(&mut **session),
            PickerMode::ChangePreview(change) => Some(&mut change.session),
            PickerMode::Place | PickerMode::Datasheets(_) => None,
        }
    }

    /// The pin counts read for `target`'s list. A read whose thread
    /// panicked says so in Messages, as a symbol read that failed; its
    /// rows stay without a count.
    pub(super) fn store_row_pins(
        &mut self,
        target: PlacedPartTarget,
        counts: Result<Vec<(Uuid, usize)>, String>,
    ) -> Task<Message> {
        let counts = match counts {
            Ok(counts) => counts,
            Err(error) => {
                let i18n = &self.ui_state.i18n;
                let reason = signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::LIBRARY_WORKER_PANICKED,
                    "error" => error
                );
                crate::diagnostics::log_warning(
                    signex_i18n::t!(
                        i18n,
                        signex_i18n::ids::LIBRARY_ROW_SYMBOL_FAILED,
                        "error" => reason.into_string()
                    )
                    .into_string(),
                );
                return Task::none();
            }
        };
        if let Some(session) = self.change_session_mut()
            && session.target == target
        {
            session.row_pins.extend(counts);
        }
        Task::none()
    }

    /// A chip was pressed: rebuild the hidden rows. A picked row the chips
    /// now hide is dropped with its comparison, as a search that hides it.
    pub(super) fn toggle_change_filter(&mut self, filter: ChangeFilter) -> Task<Message> {
        let libs = &self.library.open_libraries;
        let registry = &self.library.template_registry;
        let Some(picker) = self.library.picker.as_mut() else {
            return Task::none();
        };
        let session = match &mut picker.mode {
            PickerMode::ChangeComponent(session) => &mut **session,
            PickerMode::ChangePreview(change) => &mut change.session,
            PickerMode::Place | PickerMode::Datasheets(_) => return Task::none(),
        };
        match filter {
            ChangeFilter::SameSymbol => session.filters.same_symbol ^= true,
            // Same value needs the placed part's value; without it the chip
            // is drawn inert and changes nothing.
            ChangeFilter::SameValue if session.placed.value.is_none() => return Task::none(),
            ChangeFilter::SameValue => session.filters.same_value ^= true,
            ChangeFilter::InStock => session.filters.in_stock ^= true,
        }
        session.hidden = hidden_rows(libs, registry, &session.placed, session.filters);
        let hidden = session.hidden.clone();
        let picked_hidden = picker
            .selected
            .as_ref()
            .is_some_and(|(_, summary)| hidden.contains(&summary.row_id));
        if picked_hidden {
            picker.selected = None;
            if let PickerMode::ChangePreview(change) = &picker.mode {
                picker.mode = PickerMode::ChangeComponent(Box::new(change.session.clone()));
            }
        }
        Task::none()
    }

    /// A row of the list was clicked: pick it and read it. A row the chips
    /// hide or whose pins differ is never picked.
    pub(super) fn click_change_row(&mut self, row_id: Uuid) -> Task<Message> {
        let summary = self.library.open_libraries.iter().find_map(|lib| {
            lib.cached_components
                .iter()
                .find(|c| c.row_id == row_id)
                .map(|summary| (lib.root.clone(), summary.clone()))
        });
        let Some(picker) = self.library.picker.as_mut() else {
            return Task::none();
        };
        let session = match &picker.mode {
            PickerMode::ChangeComponent(session) => &**session,
            PickerMode::ChangePreview(change) => &change.session,
            PickerMode::Place | PickerMode::Datasheets(_) => return Task::none(),
        };
        let refused = session.hidden.contains(&row_id)
            || matches!(session.pin_match(row_id), Some(PinMatch::Differs { .. }));
        if refused || summary.is_none() {
            return Task::none();
        }
        picker.selected = summary;
        self.resolve_change_part()
    }

    pub(super) fn toggle_change_keep(&mut self, name: String) -> Task<Message> {
        if let Some(picker) = self.library.picker.as_mut()
            && let PickerMode::ChangePreview(change) = &mut picker.mode
            && change.rows.iter().any(|r| {
                r.kind == CompareKind::User && r.field == CompareField::Named(name.clone())
            })
        {
            match change.dropped.iter().position(|d| *d == name) {
                Some(at) => {
                    change.dropped.remove(at);
                }
                None => change.dropped.push(name),
            }
        }
        Task::none()
    }

    pub(super) fn pick_change_scope(&mut self, scope: ChangeScope) -> Task<Message> {
        if let Some(picker) = self.library.picker.as_mut()
            && let PickerMode::ChangePreview(change) = &mut picker.mode
        {
            change.scope = scope;
        }
        Task::none()
    }
}

#[cfg(test)]
#[path = "change_session_tests.rs"]
mod tests;
