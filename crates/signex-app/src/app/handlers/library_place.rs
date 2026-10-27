//! Place-from-library flow handler.
//!
//! The place flow is keyed by `(library_path, table, row_id)`:
//!
//!   1. `PlaceLibraryComponent` validates cheaply and returns a `Task`
//!      running [`resolve_placement`] off the update thread: read the
//!      picked row and its `.snxsym`, convert with `Symbol::to_lib_symbol`.
//!   2. `LibraryComponentResolved` arms a placement session (see
//!      `placement_session.rs`): the part follows the cursor, Space / X / Y
//!      turn it, and each click registers the `LibSymbol` and places a
//!      schematic `Symbol` referencing it as ONE engine batch, so a single
//!      undo removes both. The session stays armed for the next copy.
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
//! `update` does none. [`resolve_placement`] runs in `spawn_blocking` and
//! opens its own adapter on the `.snxlib`. The completion carries the sheet
//! that was active when the gesture started and refuses to arm if that
//! sheet is no longer the active one or the library was closed meanwhile:
//! "Place Failed", never a placement into the wrong sheet. It also carries
//! the placement epoch the gesture started under; Esc or a tool change in
//! between bumps it, and a read that finishes after that arms nothing.
//! A sheet written by a newer Signex is read-only: no session is armed. A symbol
//! reference into a *different* library is resolved through the other open
//! libraries (the worker gets an id-to-path map); if that library is not
//! open the failure says so.
//!
//! Drift (#912) is tracked in `library/drift.rs`. Re-placing keeps the
//! sheet's existing copy of the `LibSymbol` so existing instances never
//! change silently; when it differs from the library's, a warning points
//! at Properties > "Update from library".

use iced::Task;
use std::path::{Path, PathBuf};

use signex_engine::PlacementPayload;
use signex_library::RowId;

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::messages::{LibraryMessage, ResolvedPlacement};
use crate::library::place::resolve_placement;

impl Signex {
    /// Start a place gesture: validate cheaply, then read the row off the
    /// update thread. Always closes the picker.
    pub(crate) fn handle_place_library_component(
        &mut self,
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        self.library.picker = None;
        let Some(target_sheet) = self.document_state.active_path.clone() else {
            self.report_place_failure("Open a schematic sheet before placing a library part.");
            return Task::none();
        };
        if self.library.library_at(&library_path).is_none() {
            self.report_place_failure(&format!(
                "Library {} is no longer open.",
                library_path.display()
            ));
            return Task::none();
        }
        let arm_epoch = self.interaction_state.placement_epoch;
        let path = library_path.clone();
        let open_libraries: Vec<(uuid::Uuid, PathBuf)> = self
            .library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    resolve_placement(&path, &table, row_id, &open_libraries)
                })
                .await
                .unwrap_or_else(|e| Err(format!("place worker failed: {e}")))
            },
            move |result| {
                Message::Library(LibraryMessage::LibraryComponentResolved {
                    target_sheet: target_sheet.clone(),
                    arm_epoch,
                    library_path: library_path.clone(),
                    result: result.map(Box::new),
                })
            },
        )
    }

    /// Completion of [`resolve_placement`]: attach the part to the cursor.
    pub(crate) fn handle_library_component_resolved(
        &mut self,
        target_sheet: PathBuf,
        arm_epoch: u64,
        library_path: PathBuf,
        result: Result<Box<ResolvedPlacement>, String>,
    ) -> Task<Message> {
        if let Err(detail) = self.arm_resolved(&target_sheet, arm_epoch, &library_path, result) {
            self.report_place_failure(&detail);
        }
        Task::none()
    }

    fn arm_resolved(
        &mut self,
        target_sheet: &Path,
        arm_epoch: u64,
        library_path: &Path,
        result: Result<Box<ResolvedPlacement>, String>,
    ) -> Result<(), String> {
        let resolved = *result?;
        if arm_epoch != self.interaction_state.placement_epoch {
            // Esc, a right-click or a tool change came in while the row was
            // being read: the user cancelled this gesture.
            return Ok(());
        }
        if self.library.library_at(library_path).is_none() {
            return Err(format!(
                "Library {} was closed before the part could be placed.",
                library_path.display()
            ));
        }
        if self.document_state.active_path.as_deref() != Some(target_sheet)
            || !self.document_state.engines.contains_key(target_sheet)
        {
            return Err(format!(
                "The sheet {} is no longer active; {} was not placed.",
                target_sheet.display(),
                resolved.internal_pn
            ));
        }
        if self
            .document_state
            .engines
            .get(target_sheet)
            .is_some_and(|e| e.document().unknown.newer_minor_source.is_some())
        {
            return Err(format!(
                "The sheet {} was written by a newer Signex and is read-only in this build; {} was not placed.",
                target_sheet.display(),
                resolved.internal_pn
            ));
        }
        let prefix = self.placement_prefix_for(target_sheet, &resolved);
        let reference = self.next_designator_in_document(&prefix);
        let placed = placed_symbol(
            &resolved,
            reference,
            signex_types::schematic::Point::new(0.0, 0.0),
        );
        self.warn_if_sheet_copy_differs(&resolved.lib_symbol, &resolved.internal_pn);
        let mut fragment = signex_types::schematic::SchematicSheet::default();
        fragment
            .lib_symbols
            .insert(resolved.lib_symbol.id.clone(), resolved.lib_symbol);
        fragment.symbols.push(placed);
        self.arm_placement_session(PlacementPayload::new(fragment), target_sheet, prefix);
        Ok(())
    }

    /// The reference prefix for `resolved` on `target_sheet`: the designator
    /// standard of the project that OWNS the sheet decides by component class;
    /// without one (or for a loose sheet) the symbol's own letters apply.
    fn placement_prefix_for(&self, target_sheet: &Path, resolved: &ResolvedPlacement) -> String {
        let project = self.document_state.document_project(target_sheet);
        if let Some(p) = project.filter(|p| p.data.has_unreadable_annotation()) {
            crate::diagnostics::log_warning(format!(
                "The designator standard in {} cannot be read by this version of Signex; it is \
                 kept unchanged, and the library symbol's designator letters are used.",
                p.path.display()
            ));
        }
        signex_types::designator::resolve_prefix(
            project.and_then(|p| p.data.designator_standard()),
            Some(&resolved.class),
            &resolved.prefix,
        )
    }

    /// `RegisterLibSymbol` keeps an existing entry under the same key, so
    /// a library symbol edited since the first placement is not picked up.
    /// Say so instead of silently using the stale copy.
    fn warn_if_sheet_copy_differs(
        &self,
        fresh: &signex_types::schematic::LibSymbol,
        internal_pn: &str,
    ) {
        let Some(existing) = self
            .document_state
            .active_engine()
            .and_then(|e| e.document().lib_symbols.get(&fresh.id))
        else {
            return;
        };
        let differs = match (
            serde_json::to_string(existing),
            serde_json::to_string(fresh),
        ) {
            (Ok(a), Ok(b)) => a != b,
            _ => false,
        };
        if differs {
            crate::diagnostics::log_warning(format!(
                "Library symbol for {internal_pn} changed since it was first placed; the sheet keeps its existing copy. Select a placed {internal_pn} and use \"Update from library\" in Properties to adopt the new one."
            ));
        }
    }

    pub(crate) fn report_place_failure(&mut self, detail: &str) {
        crate::diagnostics::log_warning(format!("Place from library failed: {detail}"));
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice {
            title: "Place Failed".to_owned(),
            detail: detail.to_owned(),
        });
    }
}

/// The schematic `Symbol` for a resolved row. Field set mirrors
/// `place_selected_component`, plus the three library-identity slots.
fn placed_symbol(
    resolved: &ResolvedPlacement,
    reference: String,
    at: signex_types::schematic::Point,
) -> signex_types::schematic::Symbol {
    use signex_types::schematic::{HAlign, Point, SCHEMATIC_TEXT_MM, TextProp, VAlign};
    let text_at = |dy: f64| TextProp {
        position: Point::new(at.x, at.y + dy),
        rotation: 0.0,
        font_size: SCHEMATIC_TEXT_MM,
        justify_h: HAlign::Center,
        justify_v: VAlign::default(),
        hidden: false,
        font_family: None,
    };
    signex_types::schematic::Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: resolved.lib_id.clone(),
        reference,
        value: resolved.value.clone(),
        is_power: resolved.is_power,
        position: at,
        unit: 1,
        ref_text: Some(text_at(-2.54)),
        val_text: Some(text_at(2.54)),
        fields_autoplaced: true,
        library_id: Some(resolved.library_id),
        row_id: Some(resolved.row_id),
        library_version: resolved.library_version.clone(),
        ..Default::default()
    }
}
