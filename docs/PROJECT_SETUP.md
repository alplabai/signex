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
- **Start from a profile**: Hobby 2-layer, Standard 4-layer (the default),
  HDI 6-layer and Custom.

  | Profile | Layers | Thickness | Clearance | Track | Via / drill |
  |---|---|---|---|---|---|
  | Hobby | 2 | 1.6 mm | 0.2 mm | 0.25 mm | 0.6 / 0.3 mm |
  | Standard | 4 | 1.6 mm | 0.15 mm | 0.2 mm | 0.5 / 0.25 mm |
  | HDI | 6 | 1.2 mm | 0.1 mm | 0.1 mm | 0.3 / 0.15 mm |
  | Custom | 1 to 32 (starts at 8) | 1.6 mm | 0.15 mm | 0.2 mm | 0.5 / 0.25 mm |

- **Customize settings** (folded by default): the PCB rules (a rule template,
  the copper-layer stepper, clearance, track width, via size, via drill, all
  in mm and validated as you type) and the schematic defaults (sheet size,
  title block, designator standard). Moving the layer stepper
  switches the profile to Custom.
- **Preview**: the stack-up on the board ground, the board line and the sheet
  line.

The dialog opens as its own native window ("New Project"), like the other
dialogs (`docs/DETACHED_MODALS.md`), and locks the main window while it is
open. Its title bar carries the title; the Cancel / Create project footer stays
in view at any window size and only the body scrolls. Enter creates. Esc, the
Cancel button and the window's close button cancel, except while Create is
writing: then they wait (the Messages panel notes it) and the dialog stays.

## Rule templates

**Standard fab**, **Advanced fab** and **HDI fab** are generic capability
classes. Each sets the minimum clearance, track and via sizes of a class of
fabricator; none names a fabricator.

| Class | Clearance | Track | Via / drill |
|---|---|---|---|
| Standard fab | 0.15 mm | 0.15 mm | 0.5 / 0.25 mm |
| Advanced fab | 0.1 mm | 0.1 mm | 0.4 / 0.2 mm |
| HDI fab | 0.075 mm | 0.075 mm | 0.3 / 0.15 mm |

A class does not change the layer count.

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
- `annotation.designator_standard`, the existing Project Options setting.

A PCB added to the project starts from `project_defaults.pcb` (layers,
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
`.snxprj` dirty, is written by Save, and Ctrl+Z undoes it.

- Only the part an edit changes is stored: a board pick stores
  `project_defaults.pcb`, a sheet pick `project_defaults.sheet`, a designator
  pick only the designator standard.
- Every pick (a profile, the stepper, a template, Import rules..., a sheet
  size, the title block, the designator standard) is its own undo step;
  typing in one rule field is one step.
- Clicking the card that is already selected changes nothing.
- A rule value that does not parse keeps its inline reason; the last good
  value stays stored and the other picks are still stored.

## The command

```text
new_project_with_profile name=<text> location=<absolute path>
    [make_folder=true|false] [profile=hobby|standard|hdi|custom]
    [overrides=key=value,key=value]
```

`make_folder` is the dialog's folder checkbox and defaults to `true`
(`<location>/<name>/<name>.snxprj`); `make_folder=false` writes
`<location>/<name>.snxprj`. The same never-overwrite refusals apply.

`overrides` keys: `layers`, `thickness`, `clearance`, `track`, `via`, `drill`
(mm), `sheet` (such as `A4` or `ANSI A portrait`), `title_block`
(`true`/`false`) and `designators` (`ieee315`, `din40719_2`, `iec81346_2`).
It answers with the id, path and kind of the project it made. Without
operands, the palette entry opens the dialog. The command needs the `save`
capability in `signex-cli` (`--save`), and the folder it creates must stay
inside the allowed folders.
