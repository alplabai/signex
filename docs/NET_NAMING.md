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

## Custom ERC rules

In the ERC rule language, `net.name` is the winning name and `net.class` comes
from its prefix. `net.names` lists every name on the net and matches when any
one does, so `net.names matches "^I2C_"` still fires when `+3V3` wins. A saved
rule that matches `net.name` may need to use `net.names` after the naming change.

The rule lives in `signex-net` (`best_net_name_of`), and the ERC rule is
`crates/signex-erc/src/rules/multiple_names.rs`.
