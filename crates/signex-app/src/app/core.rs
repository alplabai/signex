//! The application core (ADR-0006): UI-free types the rest of the app builds
//! on. Nothing here names iced, an async runtime, blocking IO or a module of
//! `signex-app` outside `app::core`; `scripts/check_core_guard.sh` enforces it.

pub(crate) mod block_diagram_editor;
pub(crate) mod block_diagram_panes;
pub(crate) mod block_diagram_view;
pub(crate) mod branch_switch;
pub(crate) mod case_fold;
pub(crate) mod case_probe;
pub(crate) mod check_runs;
pub(crate) mod compiled_design;
pub(crate) mod dir_case;
pub(crate) mod dirty_documents;
pub(crate) mod document_ids;
pub(crate) mod document_key;
pub(crate) mod document_times;
pub(crate) mod editor_address;
pub(crate) mod event;
pub(crate) mod file_stamp;
pub(crate) mod joined_sheet;
pub(crate) mod loaded_project;
pub(crate) mod locked_items;
pub(crate) mod mutation_outcome;
pub(crate) mod origin;
pub(crate) mod parked_boards;
pub(crate) mod pending_library;
pub(crate) mod project_id;
pub(crate) mod rail_rename;
pub(crate) mod resolved_placement;
pub(crate) mod rule_list_filter;
pub(crate) mod rule_state;
pub(crate) mod rules_history;
pub(crate) mod undo_groups;
pub(crate) mod unreadable_cause;
