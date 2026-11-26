# Signex library format

Status: normative for `snxlib/2`, `snxsym/v1` to `v4` and `snxfpt/1.1` to
`1.3` (#3105). Section 8, embedded 3D models, is planned and not yet
implemented.

This is the byte-level specification of a Signex component library, written
so that an external converter can produce a library Signex opens and that
Signex re-saves byte for byte, without reading Signex source. Every rule
below is what the writer in `crates/signex-library` does today; where the
writer accepts more than it writes, the spec says what it writes.

**Conformance fixtures.** `docs/fixtures/library-format/library/` is a
complete small library written by Signex's own writer: two symbols (one with
two units and De Morgan bodies), three footprints (one with a STEP
attachment, one with an asymmetric pad layout) and three part rows covering every structured cell form.
`docs/fixtures/library-format/content-hash/<row_id>.json` holds the exact
bytes each row's `content_hash` is computed over. The test
`crates/signex-library/tests/library_format_conformance/` builds the same
library in code and fails when the writer's output, a re-save of any fixture
file, or a content hash no longer matches. When the format changes on
purpose, regenerate the fixtures with

```text
SIGNEX_BLESS_LIBRARY_FORMAT=1 cargo test -p signex-library --test library_format_conformance
```

and update this document in the same change. A converter's own test should
diff its output for equivalent input against these files.

**Writer and check.** A Rust converter can write through
`signex_library::writer::LibraryWriter`, which writes a whole library in one
pass with these serialisers and reproduces the fixture library byte for byte.
Any converter can verify its output with `signex-cli library check <dir>`
(`docs/CLI.md`, "Library check"), which tests the rules below.

The fixture data is synthetic: it exercises the format, not real parts.

## 1. Library directory

```text
<Name>/                         the library root
├── <Name>.snxlib               manifest and part tables (section 2)
├── symbols/*.snxsym            symbol envelopes (section 3)
├── footprints/<uuid>.snxfpt    footprint envelopes (section 4)
├── sims/<uuid>.snxsim          simulation models (not specified here)
├── step/<sha256>.step          STEP models of footprint attachments (4.3)
└── assets/datasheets/<sha256>.pdf   pinned datasheets
```

- The `.snxlib` file is the library: Signex opens a library by that path,
  and its folder is the root. The file name is free; the extension must be
  `.snxlib`.
- **Symbols.** Every `*.snxsym` directly in `symbols/` is read (no
  subfolders). The file name is not an identifier: a symbol is found by its
  `uuid` inside the envelope. One envelope may hold any number of symbols.
  Signex names a new envelope after its `display_name` (or, when that is
  empty, its first symbol's name): ASCII letters and digits in lower case,
  every run of other characters replaced by one `-`, with no `-` at the
  start or end; a name with no letter or digit becomes `untitled`
  (`Conformance Symbols` becomes `conformance-symbols.snxsym`). When that
  file already exists it uses `<file_uuid>.snxsym`. A converter may use
  either form.
- **Footprints.** A footprint is found by file name:
  `footprints/<footprint uuid>.snxfpt`, the `uuid` of the footprint, not the
  envelope's `file_uuid`. Signex reads the **first** footprint in that file
  and does not check its uuid. An envelope may parse with several
  footprints, but only the first is reachable from a part row, so a
  converter writes **one footprint per `.snxfpt`**, named by its uuid. See
  Open points.
- **Sims.** `sims/<model uuid>.snxsim`, same rule as footprints.
- **STEP models.** A footprint's `step_attachment` (4.3) names
  `step/<content_hash>.step`: the raw bytes of the STEP file, named by their
  lower-case hex SHA-256. The footprint editor writes the file when a model
  is attached; the board 3D view reads it and reports the footprint's model
  as missing when the file is absent.
- **Pinned datasheets.** A row's `hash_pinned` document resolves to
  `assets/datasheets/<hash>.pdf`, `<hash>` being the lower-case hex SHA-256
  of the file.
- Other folders (`classes/`, `3dmodels/`) are recognised as part of the
  library by the app but carry no format of their own.
- **Text.** Every file is UTF-8 without a byte-order mark, LF line endings,
  ending in exactly one `\n`.

### Git

A library works with or without version control. Without `.git/`, Signex
reads and writes plain files. When the user enables version control, Signex
runs `git init` in the root, writes a managed block into `.gitignore` and,
when Git LFS is asked for, `.gitattributes` rules for `*.step`, `*.stp`,
`*.wrl` and `*.iges`. A converter should not create `.git/` itself unless
asked to; if it does, the library is still valid. Signex keeps a write-lock
file (inside `.git/signex/`, or a hidden sidecar next to the `.snxlib`
without git); a converter must not create or commit one.

### Unknown keys

- **`.snxlib`:** a file holding keys this build does not model keeps them.
  Signex then writes by editing the text it read, so unknown keys, their
  order and every untouched row survive a save.
- **`.snxsym` and `.snxfpt`:** unknown keys are **dropped** on the next save.
  A converter must write only the keys below.

## 2. `.snxlib` (`snxlib/2`)

A TOML document: a manifest, then one `[tables.<name>]` block per part table.
`snxlib/2` is the only token read or written; any other token, or none, is
refused.

### 2.1 Manifest, in write order

```toml
format = "snxlib/2"
library_id = "<uuid>"

[library]
name = "<text>"
description = "<text>"            # omitted when absent
license = "<SPDX id>"             # omitted when absent
source = "<text>"                 # omitted when absent
converted_by = "<text>"           # omitted when absent
requires = ["<uuid>"]             # omitted when empty

[mode]
kind = "local_git"

[workflow]
mode = "personal"                 # personal | team | enterprise
review_required = false
reviewers_required = 1
auto_lifecycle_promote = "Released"

[users]
default_role = "Designer"
allow_self_signup = false

[users."<email>"]                 # zero or more, sorted by email
display_name = "<text>"
roles = ["<role>", ...]

[[classes]]                       # zero or more, in list order
key = "<machine key>"
label = "<display label>"
```

- `library_id` is a UUID, written lower-case with hyphens. It identifies the
  library everywhere (rows and designs reference primitives by
  `(library_id, uuid)`); two mounted libraries may not share one.
- Write every key shown. A missing key reads with a default and the next
  save writes it, so the file would not re-save byte for byte. Note that a
  missing `[users]` table reads `default_role` as `""`, while a `[users]`
  table without the key reads it as `"Designer"` (Open points).
- **Provenance** (#3112), for a library a converter wrote. `license` is the
  SPDX identifier of the content's licence (`CC-BY-SA-4.0`); `source` says
  where it came from, a URL or a description with its version;
  `converted_by` names the converter and its version. `requires` lists the
  `library_id`s of the libraries this one's rows reference, for example a
  parts library over a separate footprint library. All four are optional and
  written only when set, in this order, so a manifest without them re-saves
  unchanged. The Library Browser shows `license` and `source`; mounting a
  library whose required library is not open reports it in Messages and
  mounts it anyway. A `requires` entry that is not a UUID still opens and is
  kept, and `library check` reports it. No token change: older builds keep
  the keys as unknown keys.
- `[mode]` may instead be `kind = "database"` with `url` and `auth`, for a
  library served by a Signex library server; a converter writes `local_git`.
- `[[classes]]` keys must be unique; a duplicate key opens but refuses every
  save.
- Layout is TOML as section 5.3 describes: one blank line before each table
  header; an array of strings with one element inline
  (`roles = ["Librarian"]`), with two or more one element per line as the
  number arrays of 5.3.

### 2.2 Tables

After the manifest, for each table in byte order of its name:

```text
<blank line>
[tables.<name>]
tsv = '''
<header line>
<one line per row>
'''
```

then, when present, the two sidecars of section 2.6. `<name>` is written
bare when it uses only `A-Z a-z 0-9 _ -`, else as a TOML basic string
(`[tables."Discrete Passives"]`). Signex refuses to create a table whose
name is empty or contains `/ \ . : * ? " < > |`.

The `tsv` value is a TOML literal multi-line string, so its text is taken
verbatim: no escapes, backslashes are literal. The first line is the
header, then one line per row, each ending in `\n`. Cells are separated by
one tab. A reader skips blank lines; the writer writes none.

**Cells.** A cell is any UTF-8 text without a tab, without any control
character (U+0000 to U+001F, U+007F, so no CR or LF either) and without
`'''`. There is no quoting and no escaping: a value that needs one of those
is refused on save. Structured cells are compact JSON (section 2.4), where
JSON's own escapes carry control characters. An empty cell is the empty
string; what it means depends on the column (section 2.3).

**Row order.** When the header has a `row_id` column, rows are written
sorted by the `row_id` cell text in byte order (the in-memory order does not
matter). Otherwise insertion order is kept. With lower-case hyphenated UUIDs
this is UUID order.

### 2.3 Part-table columns (row schema 5)

Every part table has exactly these 22 columns, in this order, optionally
followed by a 23rd, `alternate_symbols`. Any other header is refused when
the rows are read. The schema level (5) is not written in the file.

| # | Column | Written as | Empty cell reads as |
|---|---|---|---|
| 1 | `row_id` | UUID, lower case, hyphens; unique in the library | refused |
| 2 | `internal_pn` | text, verbatim; unique in the library after trimming, case-sensitive; a blank PN never clashes | empty PN |
| 3 | `class` | text, a key from `[[classes]]` | empty |
| 4 | `datasheet` | JSON, see 2.4 | no document; re-saved as `{"kind":"url","url":""}` |
| 5 | `state` | `Draft`, `InReview`, `Released`, `Deprecated` or `Obsolete` | refused |
| 6 | `symbol_ref` | JSON primitive reference | refused |
| 7 | `footprint_ref` | empty, JSON reference, or JSON array (2.4) | no footprint |
| 8 | `sim_ref` | empty or JSON reference | no model |
| 9 | `primary_mpn` | JSON manufacturer part | refused |
| 10 | `alternates` | JSON array of manufacturer parts, `[]` for none | refused |
| 11 | `supply` | JSON array of distributor listings, `[]` for none | refused |
| 12 | `parameters` | JSON object, `{}` for none | refused |
| 13 | `pin_map_overrides` | JSON array, `[]` for none | refused |
| 14 | `created` | RFC 3339 UTC, `+00:00` (section 5.4) | refused |
| 15 | `updated` | as `created` | refused |
| 16 | `content_hash` | 64 lower-case hex digits (section 6) | all zero; re-saved as 64 `0` |
| 17 | `version` | text, `X.Y.Z` | `0.0.1`, and re-saved so |
| 18 | `released` | `true` or `false` | `false`, re-saved as `false` |
| 19 | `symbol_version` | text; the symbol version the row was built against | empty |
| 20 | `footprint_version` | text; empty when there is no footprint | empty |
| 21 | `sim_version` | text; empty when there is no model | empty |
| 22 | `description` | text, one line | empty |
| 23 | `alternate_symbols` | optional column; empty or a JSON array (2.4) | none |

Rules:

- Where the table says "re-saved", the empty cell is valid input but not a
  stable one: write the value instead.
- `row_id` and `content_hash` are read case-insensitively and re-saved in
  lower case; other UUID spellings (braces, no hyphens) are read and
  re-saved canonical. Write the canonical form.
- **Column 23.** Signex adds `alternate_symbols` to a table the first time a
  row in it gets an alternate symbol and never removes it. A converter adds
  it when at least one row of the table has alternate symbols; rows without
  any leave the cell empty. A table that has the column with every cell
  empty is also valid and re-saves unchanged.
- A row whose `footprint_ref` is empty has empty `footprint_version`; the
  versions pin which revision of each primitive the row was built against
  (`version` in the primitive's envelope).

### 2.4 Structured cells (JSON)

Every structured cell is one line of compact JSON as `serde_json` writes it:
no spaces, keys in the order given here (struct order, **not** sorted, except
the `parameters` map), strings with `"` `\` and control characters escaped
(`\n`, `\t`, `\r`, `\b`, `\f`, `\u00XX` with lower-case hex), other
characters raw UTF-8 and `/` not escaped. `null` is written where a value is
absent unless the rule says the key is omitted. Numbers follow section 5.2.

**Primitive reference** (`symbol_ref`, `sim_ref`, inside others):

```json
{"library_id":"<uuid>","uuid":"<uuid>"}
```

**`datasheet`.** A row with one unlabelled document writes the object form:

```json
{"kind":"url","url":"https://example.com/ds.pdf"}
{"kind":"hash_pinned","hash":"<sha256 hex>","filename":"ds.pdf"}
```

No datasheet is `{"kind":"url","url":""}`. As soon as the primary document
has a label or there is more than one document, the cell is an array, the
primary first; each element is the object above with `"label"` first,
omitted when empty:

```json
[{"label":"Datasheet","kind":"url","url":"https://..."},{"label":"Errata","kind":"hash_pinned","hash":"...","filename":"errata.pdf"}]
```

A one-element array is valid when that element has a label. A reader takes
an array when the cell, after leading whitespace, starts with `[`.

**`footprint_ref`.** Empty for no footprint. One footprint and no
alternates: the primitive reference object. A default plus alternates: an
array whose element 0 is the default and every later element an alternate,
each a footprint binding:

```json
[{"footprint":{"library_id":"..","uuid":".."}},{"footprint":{"library_id":"..","uuid":".."},"version":"1.0.0","pin_map_overrides":[{"symbol_pin_number":"1","footprint_pad_number":"2"}]}]
```

`version` and `pin_map_overrides` are omitted when empty. Element 0 carries
neither (the default's version and pin map are columns 13 and 20). An empty
array, a one-element array, an unknown key, or a footprint bound twice is
refused.

**Manufacturer part** (`primary_mpn`, elements of `alternates`):

```json
{"manufacturer":"Example Corp","mpn":"EX-1","status":"Primary","notes":null}
```

`status` is `Primary`, `Approved`, `Conditional` or `Disqualified`; `notes`
is a string or `null`.

**Distributor listing** (elements of `supply`):

```json
{"distributor":"ExampleDistributor","sku":"ED-123","url":"https://...","moq":100}
```

`url` is a string or `null`, `moq` an integer or `null`.

**`parameters`.** A JSON object keyed by parameter name, keys sorted in byte
order; each value is tagged:

```json
{"power":{"kind":"Measurement","value":{"value":0.1,"unit":"W"}},"resistance":{"kind":"Measurement","value":{"value":10000.0,"unit":"ohm"}},"tolerance":{"kind":"Text","value":"1%"}}
```

| `kind` | `value` |
|---|---|
| `Text` | a string |
| `Number` | a number |
| `Bool` | `true` or `false` |
| `Measurement` | `{"value": <number>, "unit": "<unit>"}` |

A measurement is stored as the value in the **base unit** with the base
unit's token: `ohm`, `F`, `H`, `V`, `A`, `W`, `Hz`, `%`, `ppm/C`, `V/us`,
`AWG` (4.7 kilo-ohm is `{"value":4700.0,"unit":"ohm"}`). Signex also reads a
prefixed form (`{"value":4.7,"unit":"kΩ"}`) as the same quantity but keeps
it as written, and keeps any other unit text as written. Non-finite numbers
are refused. The parameter `designator_prefix` (`Text`, ASCII letters) sets
the part's reference prefix.

**`pin_map_overrides`**: an array of
`{"symbol_pin_number":"<pin>","footprint_pad_number":"<pad>"}`; empty means
pin `n` lands on pad `n`.

**`alternate_symbols`** (column 23): an array, in display order, of

```json
{"symbol_ref":{"library_id":"..","uuid":".."},"symbol_version":"1.0.0","label":"IEC"}
```

`symbol_version` is always written (empty string allowed). A label must be
non-blank, unique in the row (trimmed, case-insensitive) and not `Default`
or `(no longer offered)`; an alternate must not be the default symbol, the
nil UUID, or listed twice. Such rows are refused on save.

### 2.5 Row validation on save

Signex refuses a row with a non-finite parameter, an invalid alternate
symbol (2.4), alternate footprints without a default, or a footprint bound
twice. The reader does not re-check these, so a converter must.

### 2.6 Sidecars

**Column types**, written after a table's `tsv` when the table declares any:

```toml
[tables.resistors.column_types]
released = "bool"
```

Keys sorted in byte order, quoted like table names. Values: `string`,
`number`, `int`, `bool`, `uuid`, `date`, `datetime`, `url`, `version`,
`tags`, or `enum:<value>,<value>,...`. A key naming a column the header does
not have refuses the load. They drive display and sorting only.

**Cached prices**, written after the column types, one table per row,
distributor and distributor part number, in byte order of each:

```toml
[tables.resistors.pricing.<row_id>.ExampleDistributor.ED-123]
sku = "ED-123"
currency = "EUR"
stock = 5000
fetched_at = "2026-01-03T10:00:00Z"
breaks = [{ qty = 1, unit_price = 0.1 }, { qty = 100, unit_price = 0.02 }]
```

Key segments are quoted like table names. `sku` equals the last segment;
`currency` is a three-letter code; `stock` is omitted when unknown;
`fetched_at` uses section 5.4 with `Z`; `breaks` is non-empty, ascending by
quantity (from 1), prices finite and not negative. `unit_price` uses Rust's
debug float form (`0.1`, `1.0`, `1e-7`). Prices are a cache refreshed from
distributors: a converter normally writes none. A record whose row is gone,
or whose part number the row no longer lists, is dropped on the next save.

Each sidecar section is preceded by one blank line.

## 3. `.snxsym` symbol envelope (`snxsym/v1` to `v4`)

A TOML document. Coordinates are millimetres in the symbol's frame, +y up;
angles are degrees counter-clockwise from +x.

### 3.1 Keys, in write order

```toml
format = "snxsym/v3"
file_uuid = "<uuid>"                # identifies the envelope, not a symbol
display_name = "<text>"
copied_from = "<uuid>"              # omitted unless a copy of a stock file
created = "<time>"
updated = "<time>"

[[symbols]]                         # one per symbol, panel order
uuid = "<uuid>"
name = "<text>"
anchor = [x, y]
pins_tsv = '''
<pin header>
<one line per pin>
'''
designator = "U?"
comment = "*"
designator_pos = [x, y]             # omitted when absent
comment_pos = [x, y]                # omitted when absent
description = "<text>"
component_type = "Standard"         # Standard Mechanical Graphical NetTie StandardNoBom Jumper
mirrored = false
local_fill_color = [r, g, b, a]     # omitted when absent; 0-255 each
local_line_color = [r, g, b, a]     # omitted when absent
local_pin_color = [r, g, b, a]      # omitted when absent
version = "1.0.0"
released = false
part_count = 1
created = "<time>"
updated = "<time>"

[[symbols.graphics]]                # zero or more; see below
...

[symbols.schematic_params]          # always written, may be empty
```

- `graphics` with no entries is written as `graphics = []` in the key
  section, right after `pins_tsv` (section 5.3).
- `schematic_params` uses the parameter values of 2.4 as TOML:
  `[symbols.schematic_params.<key>]` with `kind = "Text"` and `value = ...`;
  a measurement's value is the sub-table `[symbols.schematic_params.<key>.value]`.
- Symbols are written in their list order; nothing sorts them.

### 3.2 Pins (`pins_tsv`)

A TOML literal multi-line string holding a TSV block: header, then one line
per pin, each ending in `\n`, no blank lines. Columns:

| # | Column | Cell |
|---|---|---|
| 1 | `number` | text; the pin-to-pad binding key |
| 2 | `name` | text |
| 3 | `electrical` | `Input`, `Output`, `Bidirectional`, `Power`, `Passive`, `OpenCollector`, `OpenEmitter`, `NotConnected`, `Tristate`, `Unspecified`, `PowerOutput`, `GroundReference`, `Differential`, `Clock` |
| 4, 5 | `pos_x`, `pos_y` | number (5.2), the connection point |
| 6 | `orientation` | `Right`, `Up`, `Left`, `Down`: the direction the pin extends from its connection point toward the body |
| 7 | `length` | number |
| 8 | `description` | text |
| 9 | `function` | alternative names joined with `\|`; empty for none; a name may not contain `\|` |
| 10 | `pin_package_length` | number or empty |
| 11 | `propagation_delay_ns` | number or empty |
| 12 | `designator_visible` | `true` / `false` |
| 13 | `name_visible` | `true` / `false` |
| 14-17 | `inside_symbol`, `inside_edge_symbol`, `outside_edge_symbol`, `outside_symbol` | `None`, `Dot`, `ClockEdge`, `ActiveLowInput`, `ActiveLowOutput`, `SchmittTrigger`, `Analog`, `Digital`, `ShiftRight`, `ShiftLeft`, `Pi`, `Sigma`, `OpenCollector`, `OpenEmitter`, `HiZ` |
| 18 | `hidden` | `true` / `false` |
| 19 | `locked` | `true` / `false` |
| 20 | `part_number` | integer 0-255: the unit; `0` = on every unit |
| 21 | `body_style` | only when present, see below: `0` both bodies, `1` normal, `2` alternate (De Morgan) |

**The `body_style` column** is appended to a symbol's header only when at
least one pin **of that symbol** has a body style other than 1; otherwise
the header has 20 columns. The choice is per symbol, while the token is per
file, so one envelope may hold both header widths (the fixture does).

Text cells may not contain a tab, a line break, `'''` or any other control
character (the writer refuses tab, LF and `'''`; see Open points for the
rest). There is no escaping.

### 3.3 Units

`part_count` is the number of units. A pin or graphic with `part_number` `n`
belongs to unit `n`, and `0` to every unit. On load `part_count` is raised to
the highest pin `part_number`, so write it at least that high.

### 3.4 Graphics

Each `[[symbols.graphics]]` entry:

```toml
[[symbols.graphics]]
stroke_width = 0.254        # mm; 0 = renderer default
fill = [r, g, b, a]         # omitted when unfilled; closed shapes only
part_number = 0             # unit, 0 = every unit
body_style = 2              # omitted when 1

[symbols.graphics.kind]
kind = "rectangle"
...
```

| `kind` | Fields |
|---|---|
| `line` | `from`, `to` |
| `rectangle` | `from`, `to` |
| `circle` | `center`, `radius` |
| `arc` | `center`, `radius`, `start_deg`, `end_deg` |
| `text` | `position`, `content`, `size` |
| `polygon` | `vertices` (closed implicitly; do not repeat the first vertex) |
| `bezier` | `points` (four points: start, two controls, end) |
| `pie` | `center`, `radius`, `start_deg`, `end_deg` |
| `elliptical_arc` | `center`, `radius_x`, `radius_y`, `start_deg`, `end_deg` |
| `round_rectangle` | `from`, `to`, `corner_radius` |

Points are `[x, y]` arrays. An arc sweeps counter-clockwise from
`start_deg` to `end_deg`, wrapping through 360 when `end_deg < start_deg`.
Write both angles in `[0, 360)`: on load an arc with a negative angle is
rewritten, and one spanning an exact multiple of 360 becomes a circle, so
such input does not re-save byte for byte.

### 3.5 Token

The writer chooses the lowest token the whole file needs:

| Token | When |
|---|---|
| `snxsym/v4` | any graphic is `bezier`, `pie`, `elliptical_arc` or `round_rectangle` |
| `snxsym/v3` | else, any pin or graphic has `body_style` 0 or 2 |
| `snxsym/v2` | else, any graphic is a `polygon` |
| `snxsym/v1` | otherwise |

A reader accepts all four; the token in the file is ignored on save and
recomputed.

## 4. `.snxfpt` footprint envelope (`snxfpt/1`, `1.1` to `1.3`)

A TOML document. Coordinates are millimetres, rotations and angles in
degrees.

**Footprint frame.** The footprint frame is the board's model frame: +x to
the right and **+y down the screen**, origin at the footprint's reference
point. The footprint editor draws a point `(x, y)` at screen
`(x·scale, y·scale)` with no flip, the board view does the same with board
coordinates, and placing a footprint on a board copies pad and graphic
coordinates unchanged, then (top side) rotates them about the origin and
moves them to the footprint's position; a bottom-side footprint negates x
first. The PCB view reports positions to the user with Y up, so a stored
`y` of `-1.1` reads as `+1.1` there. A rotation or an angle turns from +x
towards +y (`x' = x cos a − y sin a`, `y' = x sin a + y cos a`), which is
clockwise on screen. This is the opposite of symbols (section 3), whose +y
is up. In the fixture `SOT-23`, pads 1 and 2 at `y = 1.1` sit below pad 3
at `y = -1.1`, as on the real part with pin 1 at bottom left.

### 4.1 Keys, in write order

```toml
format = "snxfpt/1.1"
file_uuid = "<uuid>"
display_name = "<text>"             # Signex uses the footprint's name
copied_from = "<uuid>"              # omitted unless a copy of a stock file
created = "<time>"
updated = "<time>"

[[footprints]]                      # exactly one, see section 1
uuid = "<uuid>"                     # also the file name
name = "<text>"
anchor = [x, y]                     # [0.0, 0.0]
pads_tsv = '''
...
'''
silk_f = []                         # only while empty, else [[footprints.silk_f]]
silk_b = []
fab_f = []
fab_b = []
version = "1.0.0"
released = false
created = "<time>"
updated = "<time>"
schema_version = 3
description = "<text>"              # omitted when empty
default_designator = "R?"           # omitted when empty
component_type = "Mechanical"       # omitted when Standard
height_mm = 0.55                    # omitted when absent

[footprints.courtyard]
points = [[x, y], ...]

[[footprints.silk_f]]               # silk_f, silk_b, fab_f, fab_b when non-empty
...

[footprints.body_3d]
...

[footprints.step_attachment]        # omitted when absent
...

[footprints.pcb_params]             # always written, may be empty
```

- The four graphic lists are front silkscreen, back silkscreen, front
  fabrication and back fabrication outline. Each element:
  `stroke_width` (mm), `filled` (bool), then `[footprints.<list>.kind]` with
  `kind` = `line` (`from`, `to`), `rectangle` (`from`, `to`), `circle`
  (`center`, `radius`), `arc` (`center`, `radius`, `start_deg`, `end_deg`,
  increasing-angle sweep), `text` (`position`, `content`, `size`, optional
  `frame = [w, h]`) or `polygon` (`vertices`).
- `courtyard` is one polygon on the top courtyard.
- `component_type`: `Standard`, `StandardNoBom`, `Mechanical`, `Graphical`,
  `NetTie`, `NetTieInBom`, `Jumper`.
- Not specified here, so a converter omits them: `sketch` (the parametric
  sketch), `generated`, `pours`, `keepouts`, `cutouts`, `v_scores`,
  `mask_openings`, `mask_excludes`, `paste_apertures`. Each is omitted when
  empty. The board conversion refuses a footprint with pours, keepouts,
  cutouts, v-scores, standalone mask or paste shapes, or a non-zero anchor.

### 4.2 Pads (`pads_tsv`)

A TSV block like 3.2. The 13 base columns always lead, in this order:

| Column | Cell |
|---|---|
| `number` | text; the pin-map binding key |
| `kind` | `Smd`, `Tht`, `NptHole`, `ConnectorPad`, `Castellated`, `Fiducial` |
| `shape` | shape token, below |
| `size_x`, `size_y` | number |
| `pos_x`, `pos_y` | number, pad centre |
| `rotation` | number, degrees |
| `layers` | layer tokens joined with `\|` |
| `drill_diameter` | number, or empty for no drill |
| `drill_slot_length` | number for a slot, else empty; must be empty without a drill |
| `solder_mask_margin` | number or empty |
| `paste_margin` | number or empty |

**Shape tokens:** `round`, `rect`, `oval`, `round_rect:<ratio>` (corner
radius as a ratio of the short side, 0 to 0.5),
`chamfered:<ratio>:<bits>` (bits top-left, top-right, bottom-left,
bottom-right, each `0`/`1`), `trapezoid:<dx>,<dy>` (at most one non-zero),
`custom:<x>,<y>|<x>,<y>|...` (outline relative to the pad centre). Numbers
inside tokens follow section 5.2.

**Layer tokens.** The cell lists the layers the pad is on: copper, solder
mask and paste, each a separate token. Write the named tokens, in this
order, the sets the footprint editor's sketch bake writes:

| Pad | `layers` cell |
|---|---|
| `Smd` or `ConnectorPad`, top | `Top Layer\|Top Solder\|Top Paste` |
| `Smd` or `ConnectorPad`, bottom | `Bottom Layer\|Bottom Solder\|Bottom Paste` |
| `Tht` or `Castellated` | `Top Layer\|Bottom Layer\|Top Solder\|Bottom Solder` |
| `NptHole` | `Top Solder\|Bottom Solder` |
| `Fiducial`, top | `Top Layer\|Top Solder` |

The named tokens are `Top Layer`, `Bottom Layer` (copper), `Multi-Layer`
(top and bottom copper, **no** mask), `Top Solder`, `Bottom Solder` (solder
mask), `Top Paste`, `Bottom Paste`, `Top Overlay`, `Bottom Overlay`
(silkscreen). Signex also accepts the board's own tokens (`TopCopper`,
`BottomCopper`, `Inner<n>Copper`, `TopSolderMask`, `BottomSolderMask`,
`TopPaste`, `BottomPaste`, ...); any other token makes the footprint
unplaceable on a board. A token may not contain `|`. The conformance test
places every fixture footprint through the board conversion.

**Extended columns.** After the base columns come only those of the
following that some pad of the footprint sets to a non-default value, in
this order: `template`, `template_library` (text), `paste_margin_top`,
`paste_margin_bottom` (number or empty), `paste_enabled_top`,
`paste_enabled_bottom` (`1`/`0`, default `1`), `mask_margin_top`,
`mask_margin_bottom` (number or empty), `mask_tented_top`,
`mask_tented_bottom` (`1`/`0`, default `0`), `thermal_relief` (`1`/`0`,
default `1`), `corner_radius_pct` (number or empty), `feature_top`,
`feature_bottom` (`None`, `Counterbore`, `Countersink`), `testpoint` (four
bits: top assembly, top fabrication, bottom assembly, bottom fabrication;
default `0000`), `electrical_type` (`Load`, `Source`, `Terminator`; default
`Load`), `net` (text), `locked` (`1`/`0`), `hole_tolerance_plus_mm`,
`hole_tolerance_minus_mm`, `hole_rotation_deg`, `copper_offset_x_mm`,
`copper_offset_y_mm` (number or empty). Once a column is present every pad
writes its cell, default values included. Note the booleans here are
`1`/`0`, unlike the `true`/`false` of pin cells.

Pad cells may not contain any control character or `'''`.

### 4.3 3D body and STEP attachment

```toml
[footprints.body_3d]
shape = "Extrude"            # Extrude | Dome | Cylinder | Custom
height_mm = 0.6000000238418579
offset_z_mm = 0.0
top_color = [0.20000000298023224, ...]     # RGBA 0-1, four values
side_color = [0.30000001192092896, ...]

[footprints.step_attachment]
content_hash = "<sha256 hex of the STEP file>"
filename = "R_0805.step"
offset_xyz = [x, y, z]       # mm
rotation_xyz = [rx, ry, rz]  # degrees
```

`height_mm`, `offset_z_mm`, the colours and a text `frame` are 32-bit floats
in memory: the file holds the 64-bit value of the nearest 32-bit float,
written as section 5.2 says. A converter must round each value to 32 bits
first (0.6 becomes `0.6000000238418579`; the default colours are the
values above). An optional `outline` (`[footprints.body_3d.outline]` with
`points`) overrides the fabrication outline the body is extruded from.

Today the STEP bytes live outside the footprint file, in
`step/<content_hash>.step` under the library root (section 1): the raw file,
named by the lower-case hex SHA-256 that `content_hash` holds. A converter
that attaches a model writes that file too. Section 8 (format 1.4) moves the
bytes into the footprint file.

### 4.4 Token

| Token | When |
|---|---|
| `snxfpt/1.3` | any footprint has generated features or a sketch pattern numbered other than row by row |
| `snxfpt/1.2` | else, any sketch holds reference dimensions, pattern links or per-instance pad overrides |
| `snxfpt/1.1` | otherwise |

`snxfpt/1` is read but never written. A file whose footprints have no
sketch and no generated features always writes `snxfpt/1.1`.

## 5. Canonical output

### 5.1 Order

- `.snxlib`: manifest as 2.1, tables by name (byte order), within a table
  rows by `row_id`, sidecar keys in byte order.
- Envelopes: keys as 3.1 and 4.1; symbols and graphics in list order, pins
  and pads in list order. Nothing is sorted, so a converter picks a stable
  order (the fixture writes symbols in the order it lists them).
- JSON cells: struct order as 2.4; `parameters` keys in byte order.

### 5.2 Numbers

| Where | Form |
|---|---|
| Pin and pad TSV cells, shape tokens | `0` for zero (also negative zero); otherwise the shortest decimal that reads back to the same 64-bit float, never an exponent: `2.54`, `-0.9`, `0.0000001`, `1000000000000000000000`, `0.30000000000000004`. Integral values have no `.0`. |
| TOML floats (anchors, graphics, body, attachment) | the same digits, with `.0` appended to an integral value: `0.0`, `2.54`, `90.0`, `0.0000001`, `1000000000000000000000.0`; negative zero is `-0.0` |
| TOML integers | `part_count`, `part_number`, colour bytes, `schema_version`, `stock`, `qty`: plain integers |
| JSON numbers (row cells, hash input) | shortest round-trip; integral values with `.0` (`10000.0`); plain decimal for magnitudes from 1e-5 up to below 1e16 (`0.00001`, `1000000000000000.0`), exponent form outside (`1e-6`, `1e+16`, `1.5e+16`) |
| `unit_price` in prices | Rust debug form: `0.1`, `1.0`, `1e-7` |

NaN and infinity are refused everywhere.

### 5.3 TOML layout

The envelopes and the manifest are written by the `toml` crate's pretty
printer (0.8), so its layout is part of the format:

- One blank line before every `[table]` and `[[array of tables]]` header.
- Plain keys of a table come first, then its sub-tables and arrays of
  tables in field order. An empty array is a plain key (`silk_f = []`) and
  stays at its field position.
- A number array is written one element per line, indented four spaces,
  with a trailing comma:

  ```toml
  anchor = [
      0.0,
      0.0,
  ]
  ```

- An array of arrays (polygon points) keeps that shape with the inner
  arrays' closing brackets at column 0; copy it from the fixtures:

  ```toml
  points = [
      [
      -1.5,
      -0.75,
  ],
      [
  ...
  ]
  ```

- Strings: a basic string `"..."` with escapes by default. Then, by content
  (pinned by the conformance test):

  | Content | Written as |
  |---|---|
  | plain, or with `'` | `"a'b"` |
  | `"` or `\`, no `'` | literal `'a"b'`, `'a\b'` |
  | `'` and `"`, no `\` | `"""a'b"c"""` |
  | `'` and `\` | `'''a\b'c'''` |
  | a line break | `"""` + newline + text + `"""` |
  | tab, other control characters | escaped in a basic string: `"tab\tx"`, `"x\u007Fy"` |

- The `.snxlib` table blocks and the `pins_tsv` / `pads_tsv` values are
  written by Signex itself as shown in 2.2 and 3.1, not by the printer.

### 5.4 Times

| Where | Form |
|---|---|
| Envelope `created` / `updated`, price `fetched_at` | `2026-01-02T03:04:05Z` |
| Row cells `created` / `updated` | `2026-01-02T03:04:05+00:00` |

Always UTC. A fraction is written only when non-zero, with 3, 6 or 9 digits,
whichever is the fewest that hold it exactly (`.120`, `.123456789`). A
reader accepts any RFC 3339 offset and converts to UTC, so other offsets do
not re-save byte for byte.

## 6. `content_hash`

Column 16 of a part row is the lower-case hex SHA-256 of one JSON document,
the row's canonical view, written compact as in 2.4 with these keys in this
order:

| Key | Value | Present |
|---|---|---|
| `row_id` | string | always |
| `internal_pn` | string | always |
| `class` | string | always |
| `state` | `"Released"` etc. | always |
| `datasheet` | the **primary** document's object (`{"kind":"url","url":""}` when none), never the array | always |
| `datasheet_label` | the primary's label | only when not empty |
| `more_datasheets` | array of further documents (2.4 element form) | only when not empty |
| `description` | string | only when not empty |
| `symbol_ref` | reference object | always |
| `footprint_ref` | the **default** footprint's reference object, or `null` | always |
| `alternate_footprints` | array of footprint bindings, **without** the default | only when not empty |
| `sim_ref` | reference object or `null` | always |
| `pin_map_overrides` | array | always |
| `primary_mpn` | object | always |
| `alternates` | array | always |
| `alternate_symbols` | array | only when not empty |
| `supply` | array | always |
| `parameters` | object, keys sorted | always |
| `plm` | `{"plm_part_id":null,"eco_refs":[],"compliance":{}}` | always |

Not hashed: `created`, `updated`, `content_hash`, `version`, `released`,
`symbol_version`, `footprint_version`, `sim_version`. The digest's 32 bytes
are written as 64 lower-case hex digits. The fixture files
`content-hash/<row_id>.json` are these documents, without a trailing
newline.

Signex computes the hash when the editor saves a row and compares it to
tell whether a part's content changed; it does not check it on load. A
wrong hash is therefore not an error, but it defeats change detection, so
a converter must store the right one.

## 7. `.snxsim`

`sims/<uuid>.snxsim`, token `snxsim/v1`, one model per file as for
footprints. Its content is not specified here.

## 8. Embedded 3D models (format 1.4, planned by #3107/#3108)

**Planned; lands with #3107/#3108. Nothing below is implemented yet**, and
no Signex build reads or writes `snxfpt/1.4` today.

A footprint's STEP model will be stored inside the `.snxfpt` that uses it,
so the file is self-contained:

```toml
# ... envelope keys and [[footprints]] as section 4 ...

[[models]]
hash = "<sha256 hex of the raw STEP bytes>"
filename = "R_0805.step"
encoding = "deflate+base64"
size = 482113
data = '''
<base64 of the deflated bytes, wrapped at 76 columns>
'''
```

- `[[models]]` is written **last** in the file, and only when some
  footprint in the file has a `step_attachment`.
- `hash` is the SHA-256 of the raw STEP bytes, lower-case hex, and equals
  that footprint's `step_attachment.content_hash`. `filename` is the
  original name. `encoding` is `deflate+base64`, the only value. `size` is
  the raw (inflated) byte count. `data` is the base64 of the deflated bytes,
  wrapped at 76 columns, in a `'''` literal string. The string starts with
  a line break after the opening `'''` (which TOML drops) and its last line
  ends with `\n` before the closing `'''`. A reader removes every line
  break before decoding the base64.
- One entry per distinct hash per file, entries sorted by `hash`. Footprints
  with the same model share one entry; the same model in two files is
  stored twice.
- On save, entries no footprint in the file references are dropped.
- A reader checks `size` and the SHA-256 after inflating; a mismatch is a
  load error naming the footprint.
- **Token:** a file holding at least one model writes `snxfpt/1.4`. A file
  without one keeps the token its content selects today (`1.1` to `1.3`),
  byte for byte, so older builds refuse model files by name instead of
  dropping the models.
- The `step/` folder (4.3) stays a read fallback for libraries written
  before 1.4 (#3107).
- Boards never embed models: a board footprint reaches its model through
  its library reference.
- **Deflate level:** to be fixed by #3107. Byte-identical re-saves of
  converter output need the same level and the same deflate
  implementation's output; #3107 will specify both and add a fixture.

## 9. Open points

Things the code does not pin, or where it surprises:

1. **One footprint per `.snxfpt`.** The adapter reads the first footprint of
   `footprints/<uuid>.snxfpt` and does not check its uuid
   (`adapters/local_git/primitives.rs`, `read_footprint`). Several
   footprints in one envelope parse, but only the first is reachable, so
   grouping footprints per source library in one envelope does not work
   today. Symbols have no such limit.
2. **Footprint y axis.** Settled in section 4: +y down the screen, as the
   board model. The one place that draws it the other way is the footprint
   editor's 3D preview, which flips y so that +y goes up
   (`crates/signex-app/src/library/editor/footprint/preview3d.rs:68`); it is
   display only and writes nothing. A converter from a source whose +y is up
   negates every y and every angle, and swaps each arc's start and end
   angles so the arc keeps its sweep.
3. **Pin cells and control characters.** The pin writer refuses only tab,
   LF and `'''` (`primitive/symbol/serde_tsv.rs`, `pin_to_tsv_row`); a pin
   name with another control character (CR, U+0001) is written into a TOML
   literal string that then fails to parse, so the symbol file no longer
   opens. Pads refuse every control character. Converters must not write
   them.
4. **`[users]` default.** A `.snxlib` without a `[users]` table reads
   `default_role` as `""`; one with `[users]` but no `default_role` reads
   `"Designer"` (`manifest.rs`, `UsersConfig` derives `Default` while the
   serde default for the key is `"Designer"`). Write the key.
5. **Unstable empty cells.** Empty `datasheet`, `content_hash`, `version` and
   `released` cells are read and then re-saved as a value (2.3).
6. **Symbol load repairs.** Arcs with negative or full-turn angles and a
   `part_count` below the highest pin unit are rewritten on load (3.3, 3.4).
7. **STEP bytes.** Today the app stores STEP bytes in `step/`; section 8
   (format 1.4) embeds them in the footprint file, and `step/` stays a read
   fallback for older libraries (#3107).
8. **Stock library tokens.** The committed stock footprints carry
   `snxfpt/1`, which the writer never writes (it writes `1.1`), so they do
   not re-save byte for byte; the stock symbols do.
9. **Sketch and generated features.** `sketch`, `generated` and the board
   shapes of 4.1 have no specification here. A converter leaves them out.
10. **Order is the writer's input order.** Nothing sorts symbols within an
    envelope or pins and pads within a primitive, and part rows are sorted
    by `row_id` only. Converters that need a stable order (by name, by
    part number) must impose it themselves.
