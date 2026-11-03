# A placed part follows its library row: limits of the version guard (#1867, #1939)

A placed symbol is stamped with the row's `version` when it takes the row's
footprint, Manufacturer and MPN. Editing one of those three on a row in the
app moves the row's `version` (`library/writes/part_version.rs`). "Update from
library" and the open-time scan compare the part's fields with the row; the
stamp tells a row that moved from a part the user overrode.

The guard reads the stamp, nothing more, so it has these limits. None of them
loses data: each one shows or hides an offer, and nothing is rewritten without
the user's click.

## Limits

- **Edits made outside the app.** A git pull or another tool can change the
  footprint, Manufacturer or MPN without moving `version`. Part and stamp then
  still agree on the version, so the difference reads as the user's override
  and is not flagged. Move the row's `version` by hand when editing it
  outside the app.
- **An override is lost when the row's version moves for another reason.** A
  symbol-save cascade, for one, moves the version. The guard cannot tell why,
  so it reads "the row changed since this part took its fields", and Update /
  Apply then offers the row's footprint, Manufacturer and MPN over a
  deliberate per-part value.
- **A footprint rename in another library.** Renaming a footprint moves the
  version of the rows of its own library that bind it. A row of a different
  library that binds the footprint keeps its version, so parts placed from it
  are not flagged (`app/dispatch/library/editor/cascade.rs`).
- **A cascade still owed after a failed save.** The rows a failed save could
  not move stay unbumped until the next successful save of that primitive,
  which runs the cascade again.
- **A cleared field is filled again.** An empty field is not an override: if
  the user blanks Manufacturer, MPN or the footprint on one part, Update /
  Apply fills it from the row. A value the row leaves empty never clears the
  part's own.

## Cost

The open-time scan checks each stale instance against its row. The stamps are
built once per `lib_id`, each library opens once and each row's footprint name
is read once (`library/drift/check.rs`). The reads still run synchronously in
`update` (#665).

## Row edits

A row edit reads the stored row once. A row that is not stored yet is a plain
first write; any other read failure refuses the edit, because writing without
the read would skip the version bump with no word of it
(`library/writes/mod.rs`).

## Library deletes

A Component Preview edited while its library's delete runs is reported in
Messages ("unsaved edits ... were discarded"), not kept. Refusing editor edits
while `library_rename_running()` would keep them; that needs a product
decision (#1939 item 3).
