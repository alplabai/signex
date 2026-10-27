//! Undo-history storage for one sheet state.
//!
//! The engine records the whole sheet before every edit. Holding a plain
//! `SchematicSheet` per step costs one deep copy of every collection per step
//! (up to `MAX_HISTORY_ENTRIES` copies), although a typical edit changes one
//! collection. A [`SheetSnapshot`] keeps each big collection behind an
//! `Arc`, and capturing it reuses the previous entry's `Arc` for every
//! collection that compares equal, so consecutive entries share the
//! collections the step did not touch. Restoring yields exactly the sheet
//! that was captured.

use std::sync::Arc;

use signex_types::schematic::{
    Bus, BusEntry, ChildSheet, Junction, Label, NoConnect, SchDrawing, SchematicSheet, Symbol,
    TextNote, Wire,
};

macro_rules! shared_collections {
    ($($field:ident : $ty:ty),+ $(,)?) => {
        #[derive(Debug)]
        pub(super) struct SheetSnapshot {
            /// Every field except the shared collections, which are emptied.
            rest: SchematicSheet,
            $($field: Arc<Vec<$ty>>,)+
        }

        impl SheetSnapshot {
            /// Capture `sheet`, sharing each collection with `previous` when
            /// the two are equal.
            pub(super) fn capture(mut sheet: SchematicSheet, previous: Option<&Self>) -> Self {
                $(
                    let $field = share_or_wrap(
                        std::mem::take(&mut sheet.$field),
                        previous.map(|p| &p.$field),
                    );
                )+
                Self { rest: sheet, $($field,)+ }
            }

            /// The sheet exactly as captured.
            pub(super) fn into_sheet(self) -> SchematicSheet {
                let mut sheet = self.rest;
                $(sheet.$field = unwrap_or_clone(self.$field);)+
                sheet
            }

            /// Collections (by field name) this snapshot shares with `other`.
            #[cfg(test)]
            pub(super) fn shared_with(&self, other: &Self) -> Vec<&'static str> {
                let mut shared = Vec::new();
                $(
                    if Arc::ptr_eq(&self.$field, &other.$field) {
                        shared.push(stringify!($field));
                    }
                )+
                shared
            }
        }
    };
}

shared_collections! {
    symbols: Symbol,
    wires: Wire,
    junctions: Junction,
    labels: Label,
    child_sheets: ChildSheet,
    no_connects: NoConnect,
    text_notes: TextNote,
    buses: Bus,
    bus_entries: BusEntry,
    drawings: SchDrawing,
    no_erc_directives: NoConnect,
}

fn share_or_wrap<T: PartialEq>(items: Vec<T>, previous: Option<&Arc<Vec<T>>>) -> Arc<Vec<T>> {
    match previous {
        Some(prev) if **prev == items => Arc::clone(prev),
        _ => Arc::new(items),
    }
}

fn unwrap_or_clone<T: Clone>(items: Arc<Vec<T>>) -> Vec<T> {
    Arc::try_unwrap(items).unwrap_or_else(|shared| (*shared).clone())
}
