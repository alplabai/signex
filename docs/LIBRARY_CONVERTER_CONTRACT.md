# Library converter contract (v1)

Signex imports component libraries from other tools through **library
converters**: separate programs that read a foreign library and write a
Signex library directory. Signex never links a converter; the two share only
files on disk and the process contract below. A converter may be written in
any language and published under any licence.

This document is the contract. The machine-checkable part is the JSON Schema
[`schemas/library-converter-v1.json`](schemas/library-converter-v1.json);
example outputs live in [`fixtures/library-converter/`](fixtures/library-converter/).
What a converter writes is specified in [`LIBRARY_FORMAT.md`](LIBRARY_FORMAT.md).

The existing single-file project importer keeps its own contract (one input
file, one JSON summary line); a converter can offer both.

## 1. Discovery

Each converter is installed separately. Signex finds it by:

1. an environment variable naming the binary's path, if set (the variable
   name is the converter's, listed in the user guide),
2. `SIGNEX_LIBRARY_CONVERTERS`, a list of converter programs in `PATH`
   syntax, and
3. every executable on `PATH` (absolute entries only) named
   `signex-<format>-import` (plus `.exe` on Windows).

Signex lists no converter by name: each candidate is asked `describe --json`
(§2.1, a few seconds at most) and offered only when it answers with a
supported contract, so a single-file importer without `describe` is not
offered for libraries. The same program found twice (by canonical path) is
listed once; when two converters read the same extension, both are offered.
A converter path set in Preferences is a follow-up; there is no such
preference yet.

Signex never ships a converter with a licence that differs from its own; a
user installs it. A missing converter is reported with its install hint and
is never fatal to the app.

## 2. Commands

### 2.1 `describe`

```text
<converter> describe --json
```

Prints one JSON object (schema `#/$defs/describe`) and exits 0:

```json
{
  "contract": "1.0",
  "converter": "example-import",
  "version": "0.16.0",
  "inputs": [
    { "label": "Integrated library", "extensions": ["lib1"], "kind": "library" },
    { "label": "Database library",   "extensions": ["dblink"], "kind": "database" }
  ]
}
```

- `contract` is `<major>.<minor>`. Signex refuses a converter whose major is
  newer than the one it implements and accepts any minor of a known major.
- `inputs[].extensions` are lower-case, without the leading dot. A source file that always has one fixed name (a library table, say) is listed by its whole lower-case file name, e.g. `"lib-table"`; Signex matches an entry against a picked file's extension first, then against its whole name. Signex builds its
  file-picker filters from them at run time, so no foreign extension is
  compiled into Signex.
- `kind`: `library` (symbol, footprint or combined library file or folder) or
  `database` (a file describing a database of parts that points at other
  libraries).

### 2.2 `library`

```text
<converter> library <input>... --output <dir>
    [--name <name>] [--library-id <uuid>]
    [--db <path | connection string | csv-folder>]
    [--search-path <dir>]... [--map <mapping.toml>]
    [--models include|skip] [--models-dir <dir>]
    [--dry-run] [--update] [--json] [--progress]
```

| Option | Meaning |
|---|---|
| `<input>...` | One or more source files or folders. Several inputs go into **one** library. |
| `--output <dir>` | The library directory to write. Without `--update` it must not exist or be empty; otherwise the run fails without writing. |
| `--name` | Library display name. Default: derived from the first input. |
| `--library-id` | Overrides the derived library id (UUID). |
| `--db` | Database sources only: overrides the stored connection (a file path, a connection string, or a folder of per-table CSV files). |
| `--search-path` | Extra folders searched for libraries a database or source refers to. Repeatable, searched in order. |
| `--map` | Field-mapping overrides (converter-defined TOML). The mapping actually used is written next to the report. |
| `--models` | `include` (default): embed 3D models in footprint files. `skip`: write none and report each skipped model. |
| `--models-dir` | Folder that model paths in the source resolve against. |
| `--dry-run` | Read and map everything, write **only** the report into `--output` (the preview Signex shows before importing). |
| `--update` | `--output` holds a library this converter wrote from the same source: overwrite it in place (§5). Refused if the library id differs. |
| `--json` | Print the summary (§3.2) as the last stdout line. |
| `--progress` | Print progress events (§3.1) on stdout. Implies `--json`. |

Unknown options are an error (exit 1, summary with `ok: false`). A converter must also turn an internal failure (a crash while reading a malformed file) into a `failed` summary rather than exiting without one. `--help` and `--version` print their text and exit 0 with no summary; Signex never passes them.

## 3. Output streams

### 3.1 Progress (stdout, NDJSON)

With `--progress`, stdout carries one JSON object per line, flushed per line:

```text
{"event":"phase","phase":"read","total":312}
{"event":"progress","done":120,"total":312,"item":"Passives:R0603"}
{"event":"phase","phase":"write","total":1204}
...
{"event":"summary", ...}
```

- `phase` (`read`, `convert`, `write`, `models`): a new phase starts; `total`
  is its item count, or `null` when unknown.
- `progress`: `done` of `total` in the current phase; `item` names the
  source item being worked on (display only).
- Events come at least every 10 s while work continues (repeat the last
  `progress` if one item takes longer). **Signex cancels a run after 120 s
  without any event.**
- The `summary` event is always the last line.
- Nothing else is printed to stdout. Diagnostics go to stderr; Signex keeps
  only its tail for error messages.

### 3.2 Summary (last stdout line)

One JSON object, on success and on failure (schema `#/$defs/summary`):

```json
{
  "event": "summary",
  "ok": true,
  "error": null,
  "exit": "partial",
  "library": "/abs/out/Passives",
  "report": "/abs/out/Passives/import-report.json",
  "files": ["/abs/out/Passives/Passives.snxlib"],
  "counts": { "symbols": 1198, "footprints": 312, "parts": 1204, "models": 287, "skipped": 4 },
  "warnings": [{ "message": "octagonal pad written as a custom outline", "count": 37 }]
}
```

- `ok`, `error`, `files`, `warnings` keep the meaning they have in the
  single-file contract, so a reader of that contract still understands it.
- `exit` mirrors the exit code: `ok`, `partial`, `failed`.
- `library` and `report` are absolute paths inside `--output`; Signex refuses
  paths outside it.
- `warnings` aggregates the report's non-`info` items by message.

### 3.3 Exit codes

| Code | `exit` | Meaning |
|---|---|---|
| 0 | `ok` | Library written; report has only `info` / `approximated` items. |
| 2 | `partial` | Library written; some items were `dropped` or `error` (skipped). |
| 1 | `failed` | Nothing usable written (bad input, bad options, I/O error, internal error). Library files this run wrote are removed; `import-report.*` stays when the run got far enough to read the source, so the failure can be inspected. A library that existed before an `--update` run is left as it was. |

## 4. Loss report

Every run (including `--dry-run` and failed runs that got far enough to read
the source) writes `import-report.json` into `--output`, plus a
human-readable `import-report.md` with the same content. Schema
`#/$defs/report`.

```json
{
  "contract": "1.0",
  "converter": "example-import",
  "converter_version": "0.16.0",
  "started": "2026-11-26T21:00:00Z",
  "sources": [{ "path": "/abs/in/Passives.lib1", "sha256": "…" }],
  "library_id": "…",
  "counts": {
    "source":  { "symbols": 1200, "footprints": 312, "parts": 1208, "models": 287 },
    "written": { "symbols": 1198, "footprints": 312, "parts": 1204, "models": 287 }
  },
  "items": [
    {
      "kind": "part",
      "source": "Passives:R-ARRAY-4",
      "id": null,
      "code": "part.footprint_missing",
      "severity": "error",
      "message": "footprint SOIC-16W not found in the given libraries; part skipped",
      "detail": { "footprint": "SOIC-16W" }
    }
  ]
}
```

- `kind`: `symbol`, `footprint`, `part`, `model`, `row` (a database row), or
  `library`.
- `source`: the item's name in the source, `<library>:<item>` where it has one.
- `id`: the Signex UUID written for it, or `null` if it was not written.
- `code`: a stable machine key, `<area>.<what>` in snake_case. Each converter
  documents its codes; a code never changes meaning.
- `severity`:
  - `info`: nothing lost (e.g. a default applied);
  - `approximated`: written, with a documented approximation;
  - `dropped`: written, but part of it was not (e.g. a third body style);
  - `error`: the item was not written at all.
- `detail`: free JSON for the code's specifics.

**Completeness rule.** Every source item is either written or appears in
`items` with severity `error`; anything partially lost appears with
`dropped`. Converters test this rule.

## 5. Determinism and updates

- The same inputs, options and converter version produce a byte-identical
  library (see `LIBRARY_FORMAT.md` for canonical output).
- Ids are derived, not random: the library id from the source's identity;
  symbol, footprint and part ids from the library id plus the item's source
  name. Renaming an item in the source therefore changes its id.
- When two source items map to the same part number, the later ones get
  `~2`, `~3`… suffixes assigned after sorting the colliding items by
  (source file path, position in source), so adding an unrelated item never
  shifts a suffix. Each collision is a report item.
- `--update` over a library this converter wrote from the same source:
  - an imported library is a **snapshot** of its source: items no longer in
    the source are deleted, rows a user edited are overwritten, and rows or
    primitives a user added are deleted. Git history keeps only what was
    committed, so Signex refuses (or asks to commit first) when the library's
    working tree has uncommitted changes, and warns when it has local commits
    that are not imports. Users who want to edit parts copy them into their
    own library;
  - a part whose content hash changes gets its patch version bumped and its
    `updated` time set; its lifecycle state and `created` time are kept;
  - unchanged items keep identical bytes.
- Signex initialises the library's git repository and commits; the converter
  writes plain files and never runs git.

## 6. How Signex runs a converter

1. Discovery (§1) and `describe`; refuse a newer contract major.
2. Preview: `library … --output <temp> --dry-run --progress`; show the
   report.
3. Import: `library … --output <target> --progress` (plus `--update` when
   re-importing the same source).
4. On exit 0 or 2: initialise git, commit, mount the library (and the
   libraries its manifest `requires`), show the report.
5. On exit 1 or a 120 s silence: show the error and the stderr tail; Signex
   removes a target directory it created for this run.

## 7. Versioning this contract

Adding an optional field, event, option or report code is a minor bump.
Removing or changing the meaning of anything is a major bump and needs a
DECISIONS row.
