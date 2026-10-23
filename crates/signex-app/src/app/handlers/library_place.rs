//! Place-from-library flow handler.
//!
//! The place flow is keyed by `(library_path, table, row_id)`:
//!
//!   1. Look up the open library by path and read the picked row through
//!      the mounted adapter.
//!   2. Resolve the row's `symbol_ref` through `LibrarySet::resolve_symbol`
//!      and convert it with `Symbol::to_lib_symbol`.
//!   3. Register that `LibSymbol` on the active sheet and place a
//!      schematic `Symbol` referencing it, as ONE engine batch so a single
//!      undo removes both.
//!
//! ## `lib_id` scheme
//!
//! `"<library uuid>:<symbol uuid>"` — the identity of the `.snxsym`
//! primitive the row binds, never the row (many rows may share a symbol).
//! It is both `LibSymbol.id` and the placed `Symbol.lib_id`, so
//! re-placing the same symbol reuses one `lib_symbols` entry.
//!
//! ## IO
//!
//! `read_row` and `resolve_symbol` read one row and one `.snxsym` from the
//! mounted adapter on the update thread. The handler already read the row
//! here before, the adapter borrow is not `Send` into a `Task`, and the
//! read is bounded; the heavy library scan is the mount, which is already
//! off-thread.
//!
//! Drift tracking (embedded primitive slice, content-hash comparison) is
//! out of scope: only `library_id`, `row_id` and `library_version` are
//! written onto the placed `Symbol`.

use iced::Task;
use std::path::{Path, PathBuf};

use signex_library::RowId;

use super::super::contracts::Message;
use super::super::state::Signex;

impl Signex {
    /// Resolve a `(library, table, row_id)` selection, register its
    /// `LibSymbol` on the active sheet and place the part at the view
    /// centre, then close the picker. Every failure reaches the Messages
    /// panel and the error card.
    pub(crate) fn handle_place_library_component(
        &mut self,
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        self.library.picker = None;
        if let Err(detail) = self.place_library_row(&library_path, &table, row_id) {
            self.report_place_failure(&detail);
        }
        Task::none()
    }

    fn place_library_row(
        &mut self,
        library_path: &Path,
        table: &str,
        row_id: RowId,
    ) -> Result<(), String> {
        if !self.document_state.has_active_engine() {
            return Err("Open a schematic sheet before placing a library part.".into());
        }
        let library_id = self
            .library
            .library_at(library_path)
            .ok_or_else(|| format!("Library {} is no longer open.", library_path.display()))?
            .library_id;
        let adapter = self.library.set.get(library_id).ok_or_else(|| {
            format!(
                "Library {} is not mounted; reopen it and try again.",
                library_path.display()
            )
        })?;
        let row = adapter
            .read_row(table, row_id)
            .map_err(|e| format!("Could not read row {row_id} of table {table}: {e}"))?;
        let symbol = self
            .library
            .set
            .resolve_symbol(&row.symbol_ref)
            .map_err(|e| format!("Could not read the symbol of {}: {e}", row.internal_pn))?
            .ok_or_else(|| format!("{} has no symbol in its library.", row.internal_pn))?;

        let lib_id = library_lib_id(&row.symbol_ref);
        let (x, y) = self.interaction_state.canvas.view_centre_world();
        let at = signex_types::schematic::Point::new(snap_mm(x), snap_mm(y));
        let reference = self.next_library_reference(&designator_prefix(&symbol.designator));
        let value = if row.primary_mpn.mpn.is_empty() {
            symbol.comment.clone()
        } else {
            row.primary_mpn.mpn.clone()
        };
        let placed = placed_symbol(&lib_id, reference, value, at, &row, library_id);
        let commands = vec![
            signex_engine::Command::RegisterLibSymbol {
                lib_symbol: symbol.to_lib_symbol(lib_id),
            },
            signex_engine::Command::PlaceSymbol { symbol: placed },
        ];
        if self.apply_engine_commands(commands, false, false) {
            Ok(())
        } else {
            Err(format!(
                "The schematic engine rejected {}.",
                row.internal_pn
            ))
        }
    }

    /// Next free `<prefix><n>` on the active sheet. Same rule as
    /// `next_designator_for_prefix` (max existing index + 1), which is
    /// private to `actions.rs`; read from the engine document directly.
    fn next_library_reference(&self, prefix: &str) -> String {
        let max = self
            .document_state
            .active_engine()
            .map(|engine| {
                engine
                    .document()
                    .symbols
                    .iter()
                    .filter_map(|s| s.reference.trim().strip_prefix(prefix)?.parse::<u32>().ok())
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        format!("{prefix}{}", max + 1)
    }

    fn report_place_failure(&mut self, detail: &str) {
        crate::diagnostics::log_warning(format!("Place from library failed: {detail}"));
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice {
            title: "Place Failed".to_owned(),
            detail: detail.to_owned(),
        });
    }
}

/// Grid the view-centre drop is rounded to (50 mil).
const DROP_GRID_MM: f64 = 1.27;

fn snap_mm(v: f64) -> f64 {
    (v / DROP_GRID_MM).round() * DROP_GRID_MM
}

/// `"<library uuid>:<symbol uuid>"` — see the module doc.
fn library_lib_id(r: &signex_library::PrimitiveRef) -> String {
    format!("{}:{}", r.library_id, r.uuid)
}

/// Reference prefix from a library designator: `"U?"` -> `"U"`, empty -> `"U"`.
fn designator_prefix(designator: &str) -> String {
    let prefix: String = designator
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    if prefix.is_empty() {
        "U".into()
    } else {
        prefix.to_ascii_uppercase()
    }
}

/// The schematic `Symbol` for a library row. Field set mirrors
/// `place_selected_component`, plus the three library-identity slots.
fn placed_symbol(
    lib_id: &str,
    reference: String,
    value: String,
    at: signex_types::schematic::Point,
    row: &signex_library::ComponentRow,
    library_id: uuid::Uuid,
) -> signex_types::schematic::Symbol {
    use signex_types::schematic::{HAlign, Point, SCHEMATIC_TEXT_MM, TextProp, VAlign};
    let text_at = |dy: f64| TextProp {
        position: Point::new(at.x, at.y + dy),
        rotation: 0.0,
        font_size: SCHEMATIC_TEXT_MM,
        justify_h: HAlign::Center,
        justify_v: VAlign::default(),
        hidden: false,
    };
    signex_types::schematic::Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: lib_id.to_owned(),
        reference,
        value,
        position: at,
        unit: 1,
        ref_text: Some(text_at(-2.54)),
        val_text: Some(text_at(2.54)),
        fields_autoplaced: true,
        library_id: Some(library_id),
        row_id: Some(row.row_id),
        library_version: row.version.clone(),
        ..Default::default()
    }
}
