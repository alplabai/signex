# Project setup (New Project dialog)

File > New Project, the Start screen button, the command palette, Ctrl+N and
the Projects panel's empty-area menu (New Project...) open one dialog
(#2232). Nothing is written to disk until **Create project**. The same choices are available without the dialog
through the `new_project_with_profile` command.

## What the dialog asks

- **Project name** and **Location**, with **Browse...**. The checkbox
  `Create "<name>" folder` sits under Location, is on by default and its label
  follows the name as it is typed; while the name is empty it reads
  `Create a folder named after the project`. There is no path preview line
  (maintainer, 2026-11-09: keep it simple).
  - On: `<Location>/<name>/<name>.snxprj`, the folder created by Create.
  - Off: `<Location>/<name>.snxprj`, for a folder the user already made.
  - Create is off, with the reason shown inline, when the target `.snxprj`
    (or, with the checkbox off, its `<name>.snxsch`) already exists, when the
    project folder exists and is not empty, when the location does not exist
    or lies inside another project, when (with the checkbox off) the location
    is itself a project folder, or when the name cannot be a file name.
  - The dialog stays open while Create writes. If the writes fail, it stays
    open with the reason and the setup as it was.
- **Start from a profile** (#2537): four cards in a 2 x 2 grid, the fab
  tiers **Standard** (the default), **Advanced** and **HDI**, and **Custom**.
  The tier sets the rules; the copper layer count is a separate choice, a
  stepper on every card (2 to 32, in steps of 2). Stepping a card picks it on
  the new count; each card keeps its own count, and its glyph draws it.

  | Profile | Layers (start) | Thickness | Clearance | Track | Via / drill |
  |---|---|---|---|---|---|
  | Standard | 4 | 1.6 mm | 0.15 mm | 0.2 mm | 0.5 / 0.25 mm |
  | Advanced | 6 | 1.6 mm | 0.1 mm | 0.125 mm | 0.4 / 0.2 mm |
  | HDI | 8 | 1.6 mm | 0.075 mm | 0.075 mm | 0.25 / 0.1 mm |
  | Custom | 4 | 1.6 mm | 0.15 mm | 0.2 mm | 0.5 / 0.25 mm |

  This table is the canonical tier table of the setup cards. Custom starts
  from Standard's rules and keeps the values shown when it is picked.
- **Customize settings** (folded by default): the PCB rules (the **Fab tier**
  field, Import rules..., the copper-layer stepper, clearance, track width,
  via size, via drill, all in mm and validated as you type) and the schematic
  defaults (sheet size, title block, designator standard). The Fab tier field
  mirrors the picked card. The layer stepper moves the picked card's count and
  keeps its tier.
- **Preview**: the board rules drawing (two tracks at their width and
  clearance, a via at its size and drill; click a value to edit it, Enter
  commits, Tab goes to the next, Esc cancels, handles drag it, and each edit
  is the rule field's own), the board line, the stack-up on the board ground,
  and the sheet drawing. Each drawing has an expand button; the larger board
  view adds the layer count and thickness. New PCB shows the same column
  without the sheet.

The dialog opens as its own native window ("New Project"), like the other
dialogs (`docs/DETACHED_MODALS.md`), and locks the main window while it is
open. Its title bar carries the title; the Cancel / Create project footer stays
in view at any window size and only the body scrolls. Enter creates. Esc, the
Cancel button and the window's close button cancel, except while Create is
writing: then they wait (the Messages panel notes it) and the dialog stays.

## Fab tier

Each tier card is a generic capability class of fabricator (Standard,
Advanced, HDI); none names a fabricator. Picking a tier in the **Fab tier**
field picks its card, fills the rules from the table above and keeps the layer
count shown. A tier picked there also writes its DRC floor; a card click
fills the rules only.

The DRC floor (#2311) is the seven `fab_*` rules of
`docs/DRC_RULES.md`, "Fab profiles", go into the project's rules beside the
rule profile's, and are edited afterwards like any rule. Where the class is
picked decides when:

- **New Project**: the class chosen when Create is pressed is stored with
  the rule profile copy in the new `.snxprj`.
- **Project tab, Project setup**: each pick writes the rules at once, one
  undo step with the values it fills; the `.snxprj` is dirty and written by
  Save.
- **New PCB**: the class chosen goes into the project's rules when the board
  is made, one step of the rule history, written by Save.

Picking a class again replaces only the rules it writes, by name; every
other rule, and its severity entry, stays. Picking the class already applied
brings back a `fab_*` rule edited since.

## Rule file (Import rules...)

A rule file is TOML, at most 64 KiB. Unknown keys are refused so a typo is
caught.

```toml
name = "Team rules"          # optional

[pcb]
clearance_mm    = 0.127      # required, 0.05 to 5
track_width_mm  = 0.127      # required, 0.05 to 25
via_diameter_mm = 0.45       # required, 0.1 to 10
via_drill_mm    = 0.2        # required, 0.05 to 9.9, smaller than the via
copper_layers   = 6          # optional, 1 to 32
thickness_mm    = 1.6        # optional, 0.1 to 10
```

A file that cannot be used is reported in the dialog with the key and the
reason, for example "track_width_mm is missing from [pcb]".

## What is stored

The new `.snxprj` gets, in its first write and only when set (the rule
profile copy is added after, as for every new project):

- `project_defaults.pcb`: copper layers, thickness, clearance, track width,
  via size and via drill (mm).
- `project_defaults.sheet`: paper size token and title block on or off.
- `project_defaults.profile`: the id of the profile picked (a built-in token
  such as `standard`, or a saved profile's `user-...` id). Only the id is
  stored, never the saved profile itself.
- `annotation.designator_standard`, the existing Project Options setting.

A PCB or sheet added to the project starts from these through its setup
dialog (below): `project_defaults.pcb` (layers,
thickness and the board setup rules), and a schematic sheet added to it
starts with the paper size and title block of `project_defaults.sheet`.
A new board's snap grid is 0.1 mm.

A `project_defaults` section that cannot be used (a shape this build does not
read, or PCB values out of range) does not stop the project opening: it is
kept unchanged for the next save, not applied, and a warning says so.

Grid and units are not part of the project setup: they are app preferences,
not stored per sheet, so a project value would do nothing.

## Editing later

Project Options has a **Project setup** section with the dialog's own profile
cards, PCB rules and schematic defaults, filled in from the
project. An edit is a project edit like the other sections: it marks the
`.snxprj` dirty, is written by Save, and Ctrl+Z undoes it. The section's page
header holds **Save as profile...**, Your profiles is one line under the
cards, and the PCB rules and Schematic fields sit in two titled cards.

- Only the part an edit changes is stored: a board pick stores
  `project_defaults.pcb`, a sheet pick `project_defaults.sheet`, a designator
  pick only the designator standard.
- Every pick (a profile, a stepper, the Fab tier, Import rules..., a sheet
  size, the title block, the designator standard) is its own undo step;
  typing in one rule field is one step.
- Clicking the card that is already selected changes nothing.
- A rule value that does not parse keeps its inline reason; the last good
  value stays stored and the other picks are still stored.

## The command

```text
new_project_with_profile name=<text> location=<absolute path>
    [make_folder=true|false] [profile=standard|advanced|hdi|custom]
    [overrides=key=value,key=value]
```

`make_folder` is the dialog's folder checkbox and defaults to `true`
(`<location>/<name>/<name>.snxprj`); `make_folder=false` writes
`<location>/<name>.snxprj`. The same never-overwrite refusals apply.

`overrides` keys: `layers`, `thickness`, `clearance`, `track`, `via`, `drill`
(mm), `sheet` (such as `A4` or `ANSI A portrait`), `title_block`
(`true`/`false`), `designators` (`ieee315`, `din40719_2`, `iec81346_2`,
`iec81346_2_full`) and `fab` (`standard`, `advanced`, `hdi`: the tier's
values and its `fab_*`
rules). Keys apply in written order, so `fab=hdi,clearance=0.2` keeps 0.2.
`layers` keeps the tier. A built-in the user edited takes the edited values.
It answers with the id, path and kind of the project it made. Without
operands, the palette entry opens the dialog. The command needs the `save`
capability in `signex-cli` (`--save`), and the folder it creates must stay
inside the allowed folders.

## Saved profiles

**Save as profile...** sits above the profile cards in the New Project, New
PCB and New Schematic dialogs and in Project Options' setup section. It asks
for a name and stores the values shown (board rules, sheet values, designator
standard) as a profile in the user's settings (`prefs.json`, key
`setup_profiles`), not in the project. Saving under a name that exists (case
ignored) replaces that profile's values. A rule value that is not valid
refuses the save with the reason under the name field. Saved profiles appear
under **Your profiles**, one compact row each (name and a caption such as
`4L · 0.15/0.2`), in every project. A row's menu button offers **Edit
profile...**, **Rename** (refused when another saved profile has the name),
**Duplicate** (a new profile, `<name> copy`, with a fresh `user-` id) and
**Delete**, which asks first (Remove "name"? Remove / Keep); each is a
settings change and stays when the dialog is cancelled.

### Editing a built-in profile

The built-ins are editable too (#2536). Hovering a tier card shows a pencil,
**Edit profile...**: the card's values load into the fields below and a bar
says which profile is edited, with **Save profile** and **Cancel**. Save
profile stores the values shown as the user's override of that built-in: an
entry in `setup_profiles` with the tier's own id (`advanced`) and
`"edited": true`, holding its board rules. The card then shows an **edited**
tag and its values, everywhere the cards appear. The pencil of an edited
built-in opens its menu: Edit profile..., **Duplicate as my profile**, and
**Reset to default**, which drops the override so the shipped values come
back. Custom has no pencil: it is the blank start.

An override is never numbered: it always replaces the entry of its id.
Projects keep the values they copied; an edit or a reset of a built-in is not
re-applied to any project, and a project that names the tier keeps naming it.
Saving, renaming, duplicating and resetting are written like every profile
edit, below, and taken back in the app when the write fails.

Each save or removal is merged into the list the file holds now, by id, read-
modify-write and atomic, off the UI thread: entries this build cannot read,
and profiles another running Signex saved, are kept. Edits run in the order
they were made, whatever the disk speed. A new profile whose id is already in
the file is stored under the next free number (`user-team-2`) and replaces
nothing; a `setup_profiles` value that is not a list refuses the edit and is
left as it is. Messages says "Saved the
setup profile" only once the file is written; when the write fails it says
why and the change is taken back in the app.
At start-up, stored entries that cannot be offered are counted once in
Messages; they stay in the file.

A project names its default profile by id (`project_defaults.profile`),
written by New Project and by a profile card pick in Project Options (a rule
edit does not change it). A saved-profile pick in Project Options stores its
board rules, sheet values, designator standard and id as one edit. Removing
the project's own default profile, in Project Options or in the New PCB or
New Schematic dialog of that project, clears the id in the
same edit, so it does not dangle. The project's stored values stay its own:
they win over the named profile, and the id picks the card; no built-in card
is highlighted while a saved profile is picked. A part the project does not
store comes from the named profile. A project that names a saved profile the
user does not have (shared from someone else, or removed) uses its stored
values (else Standard); Messages says so once per profile per session, by the
name it had if it was removed this session. A Signex build from before the id
existed drops `project_defaults.profile` when it re-saves the project (it keeps
no unknown keys inside `project_defaults`); the stored values stay.

## New PCB and New Schematic

**New PCB** (File > New > PCB, Projects > Add New > PCB, the Update PCB
dialog's Create PCB, the palette) opens the New PCB dialog, prefilled from
the project: the profile cards, the PCB rules (Fab tier, Import rules...,
copper layers, clearance, track, via, drill, thickness) and the stack-up
preview. **Create PCB** makes the board with those values in its board setup
(the design rule check reads its clearance) and opens it unsaved, as before;
**Cancel** and Esc make nothing. A project holds one board: when its board is
already open the dialog does not open and the reason is shown.

When the project already saved a setup (a profile id or board rules), the
cards are not shown (#2538). One summary row says what the project uses,
"From project setup: Standard · 4 layers · 0.15 mm clearance · 0.2 mm
tracks", with **Use another profile...**, which unfolds the cards and Your
profiles under it and then reads **Keep project setup** (it folds them and
takes the project's setup back). The Fab tier field is not shown with the
summary row; the rule fields below stay editable. A project with no saved
setup shows the cards and opens on valid Standard values, Create on.
New Schematic does the same with the sheet defaults ("A4 landscape · Signex
default title block · IEEE 315"), unfolding Your profiles.

**New Schematic** (File > New > Schematic, Projects > Add New > Schematic,
the palette) opens the New Schematic dialog: the saved profiles (the built-in
cards differ only in board rules, so they are not offered), sheet size,
orientation, title block and designator standard. **Create schematic...**
asks where to save the sheet and writes it with those values; cancelling the
file dialog makes nothing. On a project that stores no sheet defaults (and
names no saved profile) the dialog starts from the blank sheet Add New always
made (A4), so pressing Create unchanged writes the same sheet as before. The
designator standard is the project's: it is applied only when it was picked
in the dialog, or when a saved profile was picked there and the project
stored none. A change
is a project edit (dirty, undoable), made only once the sheet is written: a
refused or failed write leaves it as it was. There is no title-block template list,
so the title block is on or off. Grid and units are app preferences, not
dialog fields. Closing the project closes its open setup dialog, with a note
in Messages; nothing is made.

Headless and agent runs make the document with no dialog:

```text
new_pcb_document [profile=<project|standard|advanced|hdi|custom|saved profile>]
new_schematic_sheet [profile=<...>] [name=<file stem>]
```

`profile=project` is the project's own setup; a built-in token changes only
the board rules; a saved profile (id or name) sets its rules, sheet values
and designator standard. `name` defaults to the first `Sheet<n>` the project
does not list. Without `profile` the project's own setup is used, as before
the dialog existed.
`new_schematic_sheet` writes a file and needs the `save` capability in
`signex-cli`.
