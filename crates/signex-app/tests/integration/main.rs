// Several modules discard fallible test-setup calls with `let _ = ...`: test
// scaffolding, not a production `iced::Task` being dropped. `[lints]` in
// Cargo.toml is package-scoped, so each test crate root needs its own expect.
#![expect(
    clippy::let_underscore_must_use,
    reason = "test modules discard Results with `let _ = ...` routinely; scaffolding, not a dropped production `iced::Task`"
)]

mod async_library_mount;
mod command_reference;
mod footprint_pad_remint;
mod footprint_pad_rotation;
mod footprint_pad_roundtrip;
mod footprint_pad_side_drill;
mod footprint_pad_sketch_mirror;
mod help_menu_tests;
mod library_open_cache;
mod library_panel_search;
mod measure_library_open;
mod passive_calculator_tests;
mod schematic_non_finite_inputs;
mod shared_config_root;
mod sketch_qfn16_smoke;
mod stock_footprint_polarity;
mod stock_library_determinism;
mod stock_library_rows;
mod support;
mod user_guide;
