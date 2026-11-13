# `.snxrules` — portable ERC rule profile (#1085)

A `.snxrules` file is a named ERC rule set (and, since #2307, an optional DRC
section) that lives outside any project. A
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
| `drc` | object, optional | Same shape as the project's `rules.drc` (#2307): `severity` (DRC rule id such as `builtin::clearance` to `error` / `warning` / `info` / `off`) and `dsl` (DRC rule language text, [`DRC_RULES.md`](../DRC_RULES.md), read by each DRC run that uses this profile and written back as typed). Omitted when empty. Enters the content hash when present ([What the hash covers](#what-the-hash-covers)). Rule ids and layers: [`pcb-drc.md`](pcb-drc.md#rule-ids-and-severities-2307-step-2a). |

Every key is optional on load (`name` and `version` default to the empty
string). An explicitly empty `erc` or `drc` is the same as an absent one, and
`"drc": {}` is dropped on re-save. `drc` was added without a `schema` bump: an
older build keeps it as an unknown key, and a profile without it re-saves
byte-identical. A profile that already held a `drc` object before this build
re-saves with equal values, but `drc` may move among the top-level keys. The
shipped
`Signex Default` profile has no `drc` section, so its version (`1.1`) and its
hash are unchanged; the built-in DRC severities live in `signex-drc` as the
built-in layer.

## Schema token and newer files

`schema` is the file-format version, independent of the project's
`rules.schema`. The current value is `1`. A file whose `schema` is greater
than `1` is refused as a whole (`RuleProfileError::UnsupportedSchema`):
applying part of a rule set would silently change ERC results. The refusal is
reported to the user (Messages panel) naming the file; it is never reported as
"profile not found", and the file is left untouched on disk. A file that is not
valid JSON, or whose values have the wrong shape, is refused the same way with
the parse error.

This now covers `drc` (#2307), a behaviour change. Before #2307 the key was
reserved and any value rode along as an unknown key. Now it is typed, so a
`drc` of another shape (`"drc": "legacy"`, `null`, a list, a severity or `dsl`
that is not a string) refuses the whole profile. The file is kept byte for
byte and never overwritten. For `User Default` this means the session runs on
the built-in ERC and DRC defaults and says so in the Messages panel.

## Unknown keys

Keys this build does not know are kept verbatim and written back, so loading
and saving a profile never drops what a newer build added:

- top level, inside `erc` and inside `drc`: captured in an `UnknownJson` bag
  (`#[serde(flatten)]`), ordered by key;
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
asks for a name and a version. A name that already exists is never replaced
silently: the dialog asks first, and only the confirmed save overwrites that
profile's file. A file that cannot be read as a profile is never overwritten,
whatever the name; the save fails and names the file. `Signex Default` and
`User Default` cannot be used as names in Save as, New, Duplicate or Rename.
A profile placed in the folder by hand is found by its `name` field, not its
file name; two files carrying the same `name` make that name ambiguous and are
reported (both paths named) rather than one being picked.

`User Default` is the machine's own rule set: the rules a project that stores
none runs with, and what the ERC dialog edits when no project is open. It is an
ordinary profile file, edited from the ERC dialog and from Preferences ▸ Rule
Profiles alike; nothing writes the old `erc_severity` / `pin_matrix` keys of
`prefs.json` any more. The first run after upgrading folds the old
`erc_severity` / `pin_matrix` keys into `User Default` once: for every rule this
build knows the preferences' value wins (a v0.15 build kept writing them after
it first made the profile, so they are the newer state), and keys of the
profile this build does not know are kept. The marker `erc_rules_in_profile` in
`prefs.json` records the fold and is written first, so a preferences file that
cannot be updated never causes a second fold over later edits. The old keys
stay in `prefs.json`, unread; a build from before this change reads them and
does not see edits made after the upgrade.

Which profile a new project starts from is the `default_rule_profile` key of
`prefs.json` (a profile name). With none chosen a new project copies `User
Default` when the machine has one, else `Signex Default`. The new `.snxprj`
stores a copy of that profile's rules and records it in `rules.profile`; a
chosen profile that cannot be read falls back to `Signex Default` with a note in
the Messages panel.

When a project is opened, the profile it recorded is compared with the local
copy (off the update loop) and a drifted profile is reported once, the same note
the ERC dialog and an ERC run give.

## What the hash covers

A profile's content hash (`RuleProfile::content_sha256`, computed by
`signex_types::project::rule_set_sha256`) covers the rules that decide check
results, its `erc` and `drc` sections (#2307 step 2b-2, maintainer decision
https://github.com/alplabai/signex/issues/2307#issuecomment-6824136804):

- **no `drc` section** (absent or empty): the lower-case hex SHA-256 of the
  canonical JSON of the `erc` object (`severity`, `pin_matrix` and everything
  in `erc.unknown`). This is the hash every build before step 2b-2 recorded,
  so a profile or project without `drc` keeps its exact hash (the shipped
  `Signex Default` stays `89b152e9…dd76`), and every recorded provenance and
  drift result stays as it was;
- **a non-empty `drc` section**: the SHA-256 of the canonical JSON
  `{"erc": <erc>, "drc": <drc>}`, so a change to a DRC severity, the rule text
  (`dsl`) or a foreign key under `drc` changes the hash.

The maps are ordered, so equal rule sets hash equal on every machine. `name`,
`version`, `schema` and the top-level unknown keys are labels and are not
hashed, so renaming a profile or bumping its `version` is not drift.
`DrcRuleSet::sha256_hex` (the `drc` object alone) is not recorded anywhere.

**One-time effect on older records.** A profile that already held a `drc`
section (written by a step 2a or 2b-1 build) now hashes differently from the
ERC-only value its projects recorded. Such a project reads as drifted once
("The rule profile … has changed since this project's ERC rules were taken
from it"), its ERC report names the profile `<profile> (modified)`, and Update
from profile shows the comparison with no listed differences when the rules
are equal; applying it changes no rule and records the new hash. Profiles and
projects without `drc` see none of this.

What each profile action does with `drc`:

- **Save as profile** writes the project's `erc` and `drc` (not its waivers);
  over an existing profile it replaces that file, `drc` included, and a
  project without `drc` writes a profile without one. The project records the
  new hash.
- **Update from profile** adopts the profile's `drc` together with its `erc`
  as one undo step. A `drc` severity id this build does not know and foreign
  keys only the project holds are kept; `user::` severities follow the
  project's rule text, which the profile's replaces. The confirmation lists
  the DRC changes after the ERC ones: one line per severity
  (`DRC severity builtin::unconnected: default -> error`), one for the rule
  text (`DRC rule text: changed (3 lines -> 5 lines)`, `added (…)`,
  `removed (…)`) and one per foreign key.
- **Duplicate** in Preferences ▸ Rule Profiles copies the whole profile, `drc`
  included.

The hash is recorded in two places:

- a project's `rules.profile.sha256` (see [`snxprj-format.md`](snxprj-format.md)),
  taken when the project was seeded from or saved to the profile. If the local
  profile's hash later differs, a one-time note says the profile changed; the
  project's rules are never touched without the user confirming a diff;
- the ERC report header (`Rule set: <profile> (sha256 <hash>)` in the text
  report) carries the hash of the `erc` rules the run actually used, which is
  the profile hash whenever there is no `drc` section. A project whose rules
  (`erc` or `drc`) have left the recorded profile is named
  `<profile> (modified)`. The CSV report carries a `# Rule set,<profile>,<hash>`
  first row only when the project records a profile; otherwise the CSV is
  unchanged.
