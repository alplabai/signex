# Golden snapshots

`commands.json` locks the stable command-id surface (`id` + `group` +
`category`) covered by `keymap::catalog::tests::command_id_surface_matches_golden_snapshot`
in `crates/signex-app/src/keymap/catalog/mod.rs` — see signex#276.

Regenerate ONLY for an intentional additive/aliased change to the catalog:

```
UPDATE_GOLDEN=1 cargo test -p signex-app command_id_surface_matches_golden_snapshot
```

Never regenerate to make a rename/removal diff disappear — that's the
breakage this test exists to catch. Add an alias + a deprecation note
instead.

`advanced_schematic/` holds the outputs of the Advanced Schematic
exit-criteria projects (`crates/signex-types/tests/corpus/advanced/`): a BOM
(CSV, LF line ends, no byte order mark) and a PDF per variant of
`four-channel-variants`, and what Update PCB reads and stores for
`net-class-directives`. Written by `handlers::menu::export::tests::exit_fixtures`;
regenerate with a filter so the command golden above is left alone:

```
UPDATE_GOLDEN=1 cargo test -p signex-app --lib exit_fixtures
```
