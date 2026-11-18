# Format versions

Every native file Signex writes starts with a format token,
`<kind>/<major>[.<minor>]` (`snxsch/1.1`). The symbol and simulation files
spell the major with a `v` (`snxsym/v3`). One policy, `FormatVersion` in
`signex_types::format_version` (#2207), checks the token of every format; a
format declares only its own table row below.

## The rules

- **Save** writes the format's current token. The exceptions are `.snxsym`
  and `.snxsch`: each writes the lowest token the content needs. A symbol
  file without polygons or De Morgan bodies stays `snxsym/v1`; a schematic
  stays `snxsch/1.1`, byte for byte, unless a No-ERC directive names the
  rules it silences, which writes `snxsch/1.2` (#1699).
- **Load** reads every token in the format's readable list. `snxsch/1` and
  `snxsch/1.0` are the same version.
- **A newer token** is refused on load with an error that names the token,
  says a newer Signex wrote it and lists what this build reads. The caller
  adds the file name (`parse schematic <path>`, `project file <path>`).
- **A newer minor** of `.snxsch` / `.snxpcb` opens for viewing instead: the
  document remembers the token and every save, Save As included, is refused,
  so content this build cannot represent is never dropped (#977, #1402).
- **An older or foreign token** that is not in the list, or a garbled one
  (`snxsch/01`, `snxsch/1.`, a number in a `.snxprj`), is refused naming it.
- **No token** is refused, except in `.snxprj`, where a file with no token
  predates it and reads as `snxprj/1` (#788).

## Formats

| File | Writes | Reads | Newer minor | No token |
|---|---|---|---|---|
| `.snxsch` | `snxsch/1.1`, or `1.2` with a No-ERC rule list | `snxsch/1`, `1.1`, `1.2` | opens read-only | refused |
| `.snxpcb` | `snxpcb/1.4` | `snxpcb/1` to `snxpcb/1.4` | opens read-only | refused |
| `.snxprj` | `snxprj/1` | `snxprj/1` | refused | read as `snxprj/1` |
| `.snxlib` | `snxlib/2` | `snxlib/2` | refused | refused |
| `.snxfpt` | `snxfpt/1.1` | `snxfpt/1`, `snxfpt/1.1` | refused | refused |
| `.snxsym` | `snxsym/v1`, `v2` or `v3` by content | `snxsym/v1`, `v2`, `v3` | refused | refused |
| `.snxsim` | `snxsim/v1` | `snxsim/v1` | refused | refused |

`.snxrules` profiles carry a numeric `schema` key instead of a token
(`docs/architecture/snxrules-format.md`); a newer schema is refused there too.

## Changing a format

A change an older build would misread takes a new token: add it as the
format's current token, keep the old one in the readable list while files
in it still load, and add a row to `docs/DECISIONS.md`. A minor bump suits
an addition the older build of the same major can show but not save; a
major bump suits a change it cannot show at all.
