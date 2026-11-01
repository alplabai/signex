# Decisions log

Behaviour the maintainers decided on purpose. A change to a row below needs a
new decision that names it (maintainer, date, issue); an agent or contributor
PR cannot make it silently. Each row names the test that pins the behaviour.

Two golden fixtures pin the whole command surface and fail on any added,
removed or changed row: `crates/signex-app/src/menu_bar/testdata/menu_bar.golden.tsv`
(menus) and `crates/signex-app/src/keymap/testdata/{catalog,builtin_profiles}.golden.tsv`
(catalog and built-in shortcuts, regenerate with `UPDATE_GOLDEN=1`).

| Date | Decision | Decided by | Issue | Pinned by |
|---|---|---|---|---|
| 2026-10-31 | File gains Close Document, Close Project, Save All and Print in the shared menu model; the native File menu drops Exit because Quit Signex is the app-menu row on macOS. | Hakan | #1765 | `menu_bar::native::tests::the_native_file_menu_has_the_approved_rows`, `menu_bar::native::tests::app_menu_has_the_approved_content`, `menu_bar.golden.tsv` |
| 2026-10-31 | Ctrl+P is the shortcut of File ▸ Print only; Close Document is bound to `close_active_document` (Ctrl+F4 in the Altium profile). | Hakan | #1765 | `menu_bar.golden.tsv`, `builtin_profiles.golden.tsv`, `regression::file_menu_close::close_active_document_command_closes_the_tab_and_parks_a_dirty_draft` |
| 2026-10-31 | A pasted, duplicated or stamped part whose designator is taken gets the next free number; a saved legacy `keep` preference is migrated to "Keep unless taken" once. | Hakan | #1768 | `regression::paste_keep_if_free`, `regression::prefs::a_legacy_keep_is_migrated_once_and_an_explicit_keep_afterwards_stays` |
| 2026-10-31 | Tools ▸ Up Hierarchy, New Part and Remove Part are greyed where they cannot act. | Hakan | #1772 | `app::command::enablement::tests::{up_hierarchy_needs_a_parent_sheet, part_commands_need_a_symbol_editor, remove_part_needs_a_second_part_to_be_current}` |
| standing | Menu content (adding, removing, renaming or reordering a row) is decided only by the maintainers (Caner, Hakan). | Caner, Hakan | #1896 | `menu_bar.golden.tsv` |
