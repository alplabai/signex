# Layer ratchet (#664)

`scripts/check_layer_ratchet.sh` counts three kinds of layering debt with grep
and compares each count with `scripts/layer-ratchet-baseline.txt`. A count that
grows fails (new debt). A count that shrinks also fails until the baseline is
regenerated with `--generate`, so paying debt lowers the bar for good.
`scripts/test_check_layer_ratchet.sh` holds the positive and negative controls.

| Key | Counts | Owner |
|---|---|---|
| `symbol_transform_outside_types` | `struct SymbolTransform` outside `signex-types` | `signex-types` |
| `union_find_outside_net` | union-find implementations outside `signex-net` | `signex-net/src/uf.rs` |
| `app_library_editor_internals` | `library::editor::<x>::{state,canvas,updates,views}` named outside `signex-app/src/library` | the library editor slices |

Today's union-find hits are the private copies in `signex-library`
(`primitive/symbol/chain.rs`) and `signex-sketch` (`solver/redundancy.rs`).

## Blind spots

It is a grep, not a type checker. It sees `fn uf_find` / `uf_union` /
`find_root`, a `find` / `union` signature with a `parent:` parameter on one
line, and the path-halving statement `parent[x] = parent[parent[x]]`. A copy
written differently (a generic `find` with a wrapped signature, a recursive
form) is not counted. The import count is per line, so a `use` that groups
several paths counts once. Test files under `crates/*/tests` count like any
other file outside the slice.
