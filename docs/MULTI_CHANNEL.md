# Multi-channel sheets and the compiled design

A design that repeats one circuit (four identical buck stages, eight sensor
inputs) draws the circuit once, on one child sheet, and places that sheet
once as a *repeated* sheet symbol. The project compiles it into one channel
per repeat, each with its own designators and its own local nets (#1279).
The Compiled Design panel shows the project as it compiles (#1308).

## Model

- A sheet symbol is repeated when its designator (`ChildSheet.name`) reads
  `Repeat(<channel>, <from>, <to>)`, for example `Repeat(CH, 1, 4)`: four
  channels, named `CH1` to `CH4`. The keyword is case-sensitive, spaces around
  the parts are allowed, `<channel>` is an identifier, `from <= to`, and a
  repeat has at most 64 channels. Anything else is a plain designator. The
  parser is `signex_types::schematic::ChannelRepeat`.
- A sheet entry named `Repeat(<net>)` gives one net per channel; the child
  sheet has a port named `<net>`. A plain entry is shared by every channel.
- Nothing new is stored: the repeat is the designator text and the entry
  names, so a sheet that only uses multi-channel keeps its format version and
  an older build reads a plain sheet symbol with plain entries.

## Channel naming

How a channel spells a designator and a local net is a project setting,
`ProjectData.schematic_design.channel_naming`, edited from a repeated sheet
symbol's Properties (Channel naming) and saved with the project. Each format
is text with tokens:

| Token | In CH1 of `Repeat(CH, 1, 4)` |
|---|---|
| `${Channel}` | `CH1` |
| `${Index}` | `1` (the channel's position, from 1) |
| `${ChannelAlpha}` | `A` (`Z` is the 26th, then `AA`, `AB`, ...) |
| `${Designator}` | the designator in the sheet (`R5`), designator format only |
| `${Net}` | the net name in the sheet, local net format only |

The defaults are `${Channel}_${Designator}` (`R5` becomes `CH1_R5`) and
`${Net}_${Channel}` (`VOUT_3V3` becomes `VOUT_3V3_CH1`). An unknown token
stays as written. A custom format typed in the menu is checked first
(`check_format`): it must not be empty or hold spaces, every token must be
closed and known, and it needs a channel token and the designator or net
token.

## Stitching

The project walk (`signex_net`, `project/channels.rs`) walks a repeated
placement once per channel:

- Each channel is its own occurrence. Its placement path ends in a channel
  uuid derived from the sheet symbol's uuid and the channel number (see
  "Channel identity"), so every map keyed by placement keeps the channels
  apart.
- In channel k every non-power part takes the channel designator. The netlist,
  the component report (BOM, PDF) and the per-placement checks see the channel
  designators: four channels of nine parts are 36 parts.
- A net named by a Hierarchical or local label inside the channel takes the
  local net format (`FB` becomes `FB_CH1`). A net named by a Global label or a
  power port keeps its name, and so does a label that names a plain entry:
  that net is shared with the parent.
- A plain entry joins every channel's port to the one net on the parent.
- A `Repeat(<net>)` entry joins channel k's port `<net>` to member k of the
  bus the entry meets on the parent, members in written order
  (`VOUT_3V3[1..4]` carries `VOUT_3V31` to `VOUT_3V34`). On that bus, a net of
  the parent named by a member and the member nets of every other bus-typed
  entry on it (a bus drawn from entry to entry) join the same member. A joined
  net keeps its best name by the usual rule (`docs/NET_NAMING.md`), so the
  channel's `VOUT_3V3_CH1` wins over a local member label.
- A project without repeats walks and stitches exactly as before.

The ERC rule `repeat_entry_needs_bus` (Error) reports a `Repeat(<net>)` entry
that meets no bus, or a bus with fewer members than the symbol has channels.
`bad_hier_sheet_pin` matches a `Repeat(<net>)` entry with the port `<net>`.

## Channel identity

A channel's placement uuid is a version-8 uuid made from a fixed 128-bit
mix of the sheet symbol's uuid and the channel number
(`signex_net::channel_uuid`). It is stable across runs and machines, so a
channel keeps its identity while the sheet symbol does. A version-5 uuid was
the first choice; it needs the uuid crate's SHA-1 feature and a new
dependency, which this does not.

## Canvas, Properties and output

- A repeated sheet symbol draws as a stack: two outlines offset 1.27 mm and
  2.54 mm down and right behind it, at 70 % and 45 % of its outline, and a
  `×N` pill at its top-right corner; its designator is drawn in the accent.
  PDF and SVG output print the stack and the pill too
  (`signex_types::schematic::RepeatStack` is the one geometry).
- Properties of a sheet symbol: a caption (`buck_channel.snxsch · repeated
  ×4`) and a line-art illustration under the header, the Sheet section (the
  Designator row was Name), a Repeat section whose header is its switch
  (Channel name, From / To and the list of channels), Channel naming (greyed
  while Repeat is off) and the Entries list. Typing `Repeat(CH, 1, 6)` as the
  designator and editing the Repeat fields write the same designator through
  the same Name edit, one undo step each. Turning Repeat off restores the
  designator the symbol had before; that memory lasts the session only.

## Channel tabs

A sheet that a repeated sheet symbol places gets a strip under its canvas:
Editor, then one tab per channel with its part count once compiled. Editor is
the editable sheet. A channel tab shows the compiled channels as cards and is
read-only: any edit from the canvas, menus or keys, undo and redo included,
is refused at the mutation gateway with the Messages line "Channel CH1 is a
compiled view; switch to Editor to edit". Properties show the channel and its
designator map (logical designator, value, channel designator).

## Bound tables

A table bound to a part's pins or to the project's sheets (`refresh` On
compile) is rebuilt after each successful compile of its project, never on
every edit (`app/sch_tables/refresh.rs`). The refresh is a normal edit: it
runs through the mutation gateway with each sheet as the edit target, one
undo step per sheet whose tables changed, nothing when none did. So the
gateway's refusals hold: a sheet showing a channel tab (a compiled view) or a
read-only sheet keeps its rows, and Messages says "Table refresh failed on
<file>" with the reason. The refreshed tables are the compile's own output,
so they do not mark that compile out of date. Decided by Caner (2026-11-20),
see `docs/DECISIONS.md`.

## Compiled Design panel

View ▸ Compiled Design (`open_compiled_design`) or the status bar's Panels
list opens it, docked on the left as a tab beside Projects.

- `compile_project` (the palette, or the panel's Recompile and Compile
  project buttons) compiles the project off the UI thread
  (`app/compiled_design/job.rs`) from the same sheet set and graph the Net
  Inspector's netlist is stitched from. A result that lands after the project
  closed, or after a newer compile was asked for, is dropped. Nothing is
  written to disk; the result lives in memory until the project closes.
- Sheets mode: one row per sheet occurrence, depth first, indented per level;
  a repeated sheet is one row with its repeat as caption that folds open into
  its channel rows. Parts and Nets are counted per occurrence: distinct
  designators, and the nets that touch it. The footer has the unique totals.
- Nets mode: every compiled net and the number of sheet occurrences it
  crosses; power nets are drawn in the power colour.
- Clicking a row opens its sheet; a channel row opens it on its channel tab.
- Out of date: an electrical edit to a compiled sheet (any `DocumentPatch`
  but a colour-only one) marks its rows amber with a dot, and a banner offers
  Recompile. Recompile is never automatic.
- Before the first compile the panel shows "Not compiled yet" with a Compile
  project button; the mode switch and search are disabled.

## Not built yet

- Per-channel variant state (a channel, or a part in one channel, not
  fitted) and its display in the channel cards and the panel: it is set in
  the Variant Manager tab, which does not exist yet (#2662).
- The channel cards list the channel's designators; the miniature drawing of
  the sheet at card scale with the channel's designators and net names is not
  drawn yet (#2658).
- Ctrl+click on a panel row (open in a new tab): a sheet has one tab, so a
  row click focuses it (#2658).
- The naming menu opens inline under its field, right-aligned to it, not as
  a floating layer over the dock.
- Sheet entry sync does not yet treat `Repeat(<net>)` as matching the port
  `<net>`; it offers to add `<net>` as a new entry (#2659).
