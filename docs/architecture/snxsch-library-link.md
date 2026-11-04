# Library link of a placed symbol in `.snxsch` (#1338, #1331)

A symbol placed from a library row remembers which row it came from and the
row version it is pinned to. The link is what the Library Updates flow and
drift detection compare against the library's current row, so losing it on
save silently turns both off for that symbol.

## Stored fields

| Field | On disk (`[extras.symbols.<uuid>]`) |
|---|---|
| `Symbol.library_id` | `library_id = "<uuid>"` |
| `Symbol.library_version` | `library_version = "1.0.2"` |
| `Symbol.row_id` | `row_id = "<uuid>"` |

Each key is written only when set (`library_id` / `row_id` not `None`,
`library_version` not empty). A symbol with no link adds nothing to the file,
so a file with no linked symbols is byte-identical to what a build without the
keys wrote. `SymbolExtras::is_default` counts the three fields, otherwise a
symbol whose only extras are the link would be elided on save. A file without
the keys loads with `library_id = None`, `row_id = None` and an empty version,
as before.

## Format decision

No format-token bump: the token stays `snxsch/1`. `[extras.symbols.<uuid>]`
keeps unknown keys on builds since #658 (`b217775a`, trunk), so those builds
keep the link when they open and re-save the file (the same placement as the
font families in `snxsch-font-family.md`). v0.15.0, the only release, has no
unknown-key capture and drops the link keys on re-save, as it drops the font
families; it never kept the link, so that is not a regression. The fields are
declared in alphabetical key order because a capturing older build re-emits
unknown keys sorted; this makes its re-save of our file byte-identical when the
symbol carries no other unknown extras keys. With other unknown keys present
the key order can differ, but the link is still kept.

A hand-edited `library_id` or `row_id` that is not a UUID (`""`, `"nope"`, a
non-string) does not fail the sheet. It loads as no link, the symbol keeps
working as an unlinked symbol, and the invalid value is reported with a
`tracing::warn!` (target `signex::format`) that reaches Messages. The link is
lost on the next save. This differs from a malformed `pin_uuids` entry, which
fails the load: the link only feeds drift detection and Update from Library, so
it must not make a design unopenable.

`crates/signex-types/tests/fixtures/library_link/` holds a file this build
writes and the same file after trunk at `e5fc9378` (no link keys) parsed and
re-saved it; `format/library_link_tests.rs` asserts the link survives and the
bytes match. The tests' `regenerate_link_fixtures` explains how to rebuild them.

## Value stamp (#2004)

The version says only that the row moved. To tell a part's deliberate
override of footprint, Manufacturer or MPN from a field that simply has not
caught up, a part also stamps the row values it last agreed with.

```toml
[extras.symbols.<uuid>.library_stamp]
footprint = "SOIC-8"        # the bound footprint's resolved NAME, not its ref
manufacturer = "TI"         # row.primary_mpn.manufacturer
mpn = "LM358"               # row.primary_mpn.mpn, never the alias key
row_id = "<uuid>"           # the row the values came from; required
```

- **Absent is not empty.** A missing value key means the base is unknown;
  `""` means the row offered nothing. Only `None` is skipped on write.
- **Valid only for its row.** A stamp whose `row_id` is not the symbol's
  `row_id` is ignored, and the part is treated as one with no stamp.
- **Own unknown-key bag.** Keys a later build adds survive an older one. The
  table is written among the symbol's unknown tables, which sort
  alphabetically, and its keys are sorted: so an older build that keeps it as
  an unknown table re-saves it byte for byte, even beside a foreign table that
  sorts before it.
- **No format bump**: still `snxsch/1`, as for the link. A malformed table (a
  wrong type, a `row_id` that is not a UUID) loads as no stamp with a
  `tracing::warn!` (target `signex::format`); the sheet opens.
- **Nothing is rewritten on load**, and load does no library IO. A stamp is
  written at a stamping event: placement, Change Component, an update from
  the library or the modal's Apply, the open-time Personal restamp, and an
  answered conflict. The pinned version and the values are one value
  (`LinkStamp`) so no site can write one without the other.
- A production "unlink" action, if one is added, must clear the stamp.

### The decision, per field

base `b` is the stamp's value, ours `o` the part's, theirs `t` the row's now
(`signex_types::schematic::resolve`, the one decision the drift flag, the
modal and "Update from library" share). Values are compared as written; a
blank counts as empty. MPN `o` is read across `MPN_FIELDS`.

| # | Condition | Flag | Update from library | Base after |
|---|---|---|---|---|
| 1 | `t` unreadable (a footprint library not open) | no | no change | leave `b` |
| 2 | `t` empty, `o` not | no | never clears | leave `b` |
| 3 | `o = t` (includes re-typing the new value) | no | nothing | `b := t` |
| 4 | `b` known, `o = b`, `t != b` | yes | applies `t` | `b := t` |
| 5 | `b` known, `o != b`, `t = b` (an override, or a cleared field) | no | keeps `o` | leave `b` |
| 6 | `b` known, `o != b`, `t != b`, `o != t` | yes, a conflict | keeps `o`, logs `U1: kept your MPN "X"; library now "Y"` | leave `b` until answered |
| 7 | `b` known non-empty, `o` empty | as 5 or 6 | keeps it empty | as 5 or 6 |
| 8 | `b` unknown, `o` empty | yes | fills `t` | `b := t` |
| 9 | `b` unknown, `o` not empty, `o != t` | yes, a conflict | keeps `o`, never applies | leave `b` until answered |

A conflict is answered in the Library Updates modal, per part and field:
**Keep mine** (the default) keeps `o` and sets `b := t`, which silences the
flag until the row moves again; **Use library value** sets `o := t` and
`b := t`. All answers apply in one undo step with the rest of the update.
Answering ticks the part's entry; an answer on an unticked entry is ignored.
**Skip all** and **Cancel** answer nothing, so the modal asks again on every
open or mount until the conflict is answered.
"Update from library" on a placed part never answers a conflict: it keeps
`o` and also keeps the part on its old version, so the next scan lists the
part with its conflict again. The
open-time Personal restamp edits nothing: it advances a field's base only
where `o = t`, never that of a pending follow, and a part with a conflict
keeps its old version and is offered in the modal.

### Parts without a stamp

The base is derived and never stored. If `library_version` is not empty and
equals the row's version, `b` is the row as it is now (the part has not
missed a move, so a difference is an override), except that an empty field is
still filled (row 8). Otherwise `b` is unknown (rows 8 and 9). A stamping
event on a part with a row and no valid stamp writes a stamp even when it can
prove no value (all keys absent): that switches this rule off, so an
unanswered conflict is not read as "the row's value" and cleared. Once a valid
stamp exists the version is no longer consulted for that part: a missing key
is unknown. So an old file whose row moved and whose part differs in a
non-empty value gets a conflict choice, asked until answered, where it used to be
overwritten.

`library_link/written_with_value_stamp.snxsch` is a file this build writes and
`value_stamp_resaved_by_older_build.snxsch` the same file after trunk before
#2004 parsed and re-saved it (they are byte-identical);
`regenerate_stamp_fixtures` rebuilds them.
