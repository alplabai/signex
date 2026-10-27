# `.snxrules` — portable ERC rule profile (#1085)

A `.snxrules` file is a named ERC rule set that lives outside any project. A
project is seeded from one, can be compared against it, and can be updated
from it after the user reviews the differences. Parsing and writing is
`signex_types::project::RuleProfile`; the shipped profile, the diff and the
provenance record are `signex_erc::project_rules::profile`; the library on
disk is `signex-app`'s `rule_profiles` module.

## Shape

JSON, pretty-printed with ordered keys and one trailing newline, so a rule
change is a small git diff.

```json
{
  "schema": 1,
  "name": "Company",
  "version": "1.0",
  "erc": {
    "severity": { "unused_pin": "warning" },
    "pin_matrix": { "output/output": "error" }
  }
}
```

| Key | Type | Meaning |
|---|---|---|
| `schema` | integer, optional | Version of the file format. Absent reads as `1` and stays absent on save. |
| `name` | string | The profile's identity. Projects find a profile by this name. |
| `version` | string | Content version chosen by the author; recorded in a project's provenance. |
| `erc` | object, optional | Same shape as the project's `rules.erc`: `severity` (rule id to `error` / `warning` / `info` / `off`) and `pin_matrix` (`"<row axis>/<column axis>"` to severity, only cells that differ from the factory matrix). Omitted when empty. |

Every key is optional on load (`name` and `version` default to the empty
string). An explicitly empty `erc` is the same as an absent one.

## Schema token and newer files

`schema` is the file-format version, independent of the project's
`rules.schema`. The current value is `1`. A file whose `schema` is greater
than `1` is refused as a whole (`RuleProfileError::UnsupportedSchema`):
applying part of a rule set would silently change ERC results. The refusal is
reported to the user (Messages panel) naming the file; it is never reported as
"profile not found", and the file is left untouched on disk. A file that is not
valid JSON, or whose values have the wrong shape, is refused the same way with
the parse error.

## Unknown keys

Keys this build does not know are kept verbatim and written back, so loading
and saving a profile never drops what a newer build added:

- top level (a future `drc` section, say) and inside `erc`: captured in an
  `UnknownJson` bag (`#[serde(flatten)]`), ordered by key;
- unknown rule ids in `erc.severity` and unknown keys in `erc.pin_matrix`: kept
  as ordinary map entries.

"Update from profile" keeps what the project holds and the profile lacks. For a
key both hold with different values, the profile's value wins and the change
is listed in the confirmation panel (as `other setting <key>`, `severity <id>`
or `pin matrix <key>`), so the update applies exactly the listed changes.

## Location and file names

User profiles live in `rule-profiles/` under the Signex config folder
(`<OS config dir>/signex/rule-profiles/`). The shipped profile, `Signex
Default`, is built in and has no file; a user profile cannot take that name.

The file name is derived from the profile name: lower-cased, every run of
characters that are not letters or digits collapsed to one `-`, trimmed of
leading and trailing `-`, plus `.snxrules` (`Acme Corp / Rules!` becomes
`acme-corp-rules.snxrules`; an empty result becomes `profile.snxrules`).

Writes go through `atomic_write` (temp file, fsync, rename). "Save as profile"
never replaces a file: it reserves the file name with an exclusive create
first, and a taken name gets the next free suffix (`Company`, `Company 2`, ...).
A profile placed in the folder by hand is found by its `name` field, not its
file name; two files carrying the same `name` make that name ambiguous and are
reported (both paths named) rather than one being picked.

The first run after upgrading moves the machine's old preferences rules into a
`User Default` profile in this folder, once.

## What the hash covers

A profile's content hash is the lower-case hex SHA-256 of the canonical JSON of
its `erc` object: `severity`, `pin_matrix` and everything in `erc.unknown`.
The maps are ordered, so equal rule sets hash equal on every machine. `name`,
`version`, `schema` and the top-level unknown keys are labels and are not
hashed, so renaming a profile or bumping its `version` is not drift.

The same hash is recorded in two places:

- a project's `rules.profile.sha256` (see [`snxprj-format.md`](snxprj-format.md)),
  taken when the project was seeded from or saved to the profile. If the local
  profile's hash later differs, a one-time note says the profile changed; the
  project's rules are never touched without the user confirming a diff;
- the ERC report header (`Rule set: <profile> (sha256 <hash>)` in the text
  report) as the hash of the rules the run actually used. A project whose rules
  have left the recorded profile is named `<profile> (modified)`. The CSV
  report carries a `# Rule set,<profile>,<hash>` first row only when the project
  records a profile; otherwise the CSV is unchanged.
