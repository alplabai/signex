# Net naming: when one net has several names

A net can carry more than one name, for example a local label `SDA` and a
global label `I2C_SDA` on the same wire, or two power ports `+3V3` and `VDD`
joined by a wire. Signex shows exactly one name for the net and keeps the others
as aliases.

## Which name wins

1. The kind of the name decides first: **Power** (power-port value or power
   label) beats **Global** label, which beats **Hierarchical** label, which
   beats a **local** net label.
2. If two names have the same kind, the one that sorts first wins (byte order of
   the text, so `+3V3` comes before `VDD`). Across sheets the comparison uses
   the full name, including the sheet path a hierarchical or local name gets
   (`A/TX` before `B/TX`).

The result does not depend on the order of wires, labels or child sheets in the
file, so the same design always gets the same name. The netlist exports and the
ERC net names use this rule. Net colour and highlight work from label text or
geometry, and the BOM has no net names, so they do not follow it.

An existing project can export different net names after this rule: Power now
beats Global, equal names tie-break on the full name, and hierarchical names
are qualified by the sheet that wins.

## Aliases still join

Every name stays a member of the net. A sheet that uses any of the names still
connects to it, including across sheets: if the root sheet carries a net as both
`SDA_A` and `SDA_B`, a child sheet using `SDA_A` and another using `SDA_B` end up
on the same net, shown as `SDA_A`.

Which names join across sheets depends on the kind. Global and Power names join
everywhere. A Hierarchical name joins only through a matching sheet pin. A local
net label stays within its sheet.

## ERC: "Net has multiple names"

ERC reports a net with two or more different names, lists each name with its
position, and says which one is used. The default severity is Info; change or
switch it off in the ERC dialog like any other rule.

To avoid reporting the same net twice, a net whose local, global or power
**labels** disagree is reported by **Net label conflict** only. "Net has
multiple names" covers the rest: a hierarchical label or a power-port symbol
whose name differs from the net's other names. It checks one sheet at a time and
does not compare names that meet only through a sheet pin. The rule is per
sheet for now; the cross-sheet version is tracked in
[#1061](https://github.com/alplabai/signex/issues/1061).

The naming rule lives in `signex-net` (`best_net_name_of`), and the ERC rule is
`crates/signex-erc/src/rules/multiple_names.rs`.

## Custom ERC rules

In the ERC rule language, `net.name` is the winning name and `net.class` comes
from its prefix. `net.names` lists every name on the net and matches when any
one does, so `net.names matches "^I2C_"` still fires when `+3V3` wins. A saved
rule that matches `net.name` may need to use `net.names` after the naming change.

An unnamed net has `net.name == ""` but an empty `net.names`, so
`net.names == ""` is false there; test `net.name == ""` to find unnamed nets.

## Net scope (fixed)

How far a name reaches is fixed by the kind of label. There is no setting for
it in the project file, the preferences, the UI or the command line, and
existing designs keep the nets they have.

| Kind | Reach |
|---|---|
| Net label | Its own sheet only. It never crosses to another sheet. |
| Port (global label) | Every sheet in the project, joined by name. |
| Sheet entry (hierarchical label) | Only through the matching pin on the parent's sheet symbol. Each placement of a child sheet keeps its own nets. |
| Power port and power label | Every sheet in the project, joined by name. |

In plain terms: a net label connects wires only on the sheet it is on. Two net
labels named `SDA` on two sheets are two separate nets. To carry a signal
across sheets, use a Port, or a sheet entry wired to a pin on the parent's
sheet symbol.

A child sheet placed twice is two placements of one file. A sheet entry `X` in
it joins whatever the parent wires to `X` on each placement, so the two
placements never short together through it.

ERC warns when this looks like a mistake: **Local label name on other sheets**
(default severity Warning) fires when a net label's name is also a net label on
a different sheet and no Port or power name carries it. The message names the
label and the sheets and says to use a Port or a sheet entry. It checks the
sheet being checked and the sheets below it. The rule is
`crates/signex-erc/src/rules/local_label_other_sheets.rs`; the stitching rule
is ADR-0002 D8.
