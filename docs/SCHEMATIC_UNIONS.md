# Named unions on a schematic sheet

A union (mockup row 158) is a named group of objects on one sheet, with tags. It selects
as one, can be locked or hidden, and is listed in the Unions panel. This page
is the reference for how unions behave; the code is in
`signex_engine::unions`, `signex-engine/src/exec/unions.rs`,
`signex-app/src/app/unions/`, `signex-app/src/panels/unions/` and
`signex-app/src/schematic_runtime/unions.rs`.

## What a union is

- A union has an id, a name, tags and a list of members, each an object id
  of the sheet: a part, a wire, a bus, a bus entry, a junction, a no-connect,
  a No-ERC directive, a directive, a label, a text note, a sheet symbol, a
  drawing, a harness connector or a harness wire. A sheet entry belongs to
  its sheet symbol, a harness entry to its connector and a part's fields to
  the part, so none is a member of its own. The loader and the engine share
  this rule (`signex_types::schematic::union_member_kinds`).
- It is saved with the sheet (`unions`, format 1.3). A member that is no such
  object is dropped when the sheet is read, and Messages lists each one.
- **One union per object.** Create Union and Add Selection refuse an object
  that is already in another union, and say which union holds it.
- **One sheet per union.** A union lists objects of its own sheet only; tags
  give the cross-sheet view (the panel's Project scope lists every open sheet
  of the project, grouped by sheet).
- Tags are stored without a leading `#`, trimmed, each once.

## Editing

Every change is an engine command, so it is one undo step:

| Action | Command | Notes |
|---|---|---|
| Create Union… (`Ctrl+G`, right-click ▸ Unions, the panel's + button) | `CreateUnion` | needs a name and at least one object |
| Rename, retag | `UpdateUnion` | the keystrokes of one union's name are one undo step |
| Lock, unlock | `UpdateUnion` | a step of its own |
| Add Selection, Remove Selection | `SetUnionMembers` | an empty list breaks the union |
| Break Union | `BreakUnion` | the objects stay on the sheet |

Deleting an object removes it from its union in the same undo step; a union
left with no members goes in that step too. The same holds for any edit that
removes objects (a wire merged into another, a replaced document). A union
that was already empty (one the loader kept) is left alone by an unrelated
edit.

## Selecting

A click on any member selects every member of its union, and a click on the
union's name tab selects the union. The union whose members are all selected
is drawn in the accent colour and becomes the panel's picked union.

## Locking

A locked union freezes its members the way the object lock does: they cannot
be moved, turned, mirrored, edited or deleted until the union is unlocked. A
selection that holds them skips them and says why ("union Input filter is
locked ..."). A locked union also refuses Add / Remove Selection and Break
Union; unlocking is the way out.

## Hiding

Hide is a view setting of one sheet in this window: it is not saved, it is not
an undo step, and printing and exporting are unchanged (unions are never
printed). A hidden union keeps its dashed outline and a grey name tab; its
members fade to about 15 % behind a veil in the paper colour. The panel row is
dimmed, and the panel foot says the union is hidden with a Show button instead
of member editing.

## On the canvas

The outline is drawn in the selection layer, above the drawings: a dashed
rounded rectangle (6 / 4 px, 1.2 px) 10 px around the members' boxes, with the
name tab at its top-left, the tags beside the tab and a padlock on the tab
(closed when locked, open when not). It follows the camera and never enters
the content cache, a print or an export.

## The Unions panel

View ▸ Unions opens it (docked on the right). From the top: the filter field
(`name` matches the name or a tag, `#tag` a tag's start) and the Create from
selection button; the This sheet / Project switch; the Tags chips, which keep
the unions that carry any chip that is on. Each row shows the union's name,
member count and tags, and three actions: Select members, Lock / Unlock and
Hide / Show. The foot edits the picked union: its name (a rename is one undo
step), the member pills and wire count, Add selection, Remove selection and
Break union.
