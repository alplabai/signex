//! The application core (ADR-0006): UI-free types the rest of the app builds
//! on. Nothing here names iced, an async runtime, blocking IO or a module of
//! `signex-app` outside `app::core`; `scripts/check_core_guard.sh` enforces it.

pub(crate) mod case_fold;
pub(crate) mod command_log;
pub(crate) mod compiled_design;
pub(crate) mod component_class;
pub(crate) mod document_key;
pub(crate) mod document_times;
pub(crate) mod editor_address;
pub(crate) mod event;
pub(crate) mod held_engine;
pub(crate) mod locked_items;
pub(crate) mod mutation_outcome;
pub(crate) mod new_library_defaults;
pub(crate) mod origin;
pub(crate) mod parts_read_error;
pub(crate) mod pin_selection_mode;
pub(crate) mod project_id;
pub(crate) mod resolved_placement;
pub(crate) mod rule_list_filter;
