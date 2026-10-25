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
   the text, so `+3V3` comes before `VDD`).

The result does not depend on the order of wires or labels in the file, so the
same design always gets the same name. The netlist, the net-colour and
highlight tools, the BOM and every export all use this one name.

## Aliases still join

Every name stays a member of the net. A sheet that uses any of the names still
connects to it, including across sheets: if the root sheet carries a net as both
`SDA_A` and `SDA_B`, a child sheet using `SDA_A` and another using `SDA_B` end up
on the same net, shown as `SDA_A`.

## ERC: "Net has multiple names"

ERC reports a net with two or more different names, lists each name with its
position, and says which one is used. The default severity is Info; change or
switch it off in the ERC dialog like any other rule.

To avoid reporting the same net twice, a net whose local, global or power
**labels** disagree is reported by **Net label conflict** only. "Net has
multiple names" covers the rest: a hierarchical label or a power-port symbol
whose name differs from the net's other names. It checks one sheet at a time and
does not compare names that meet only through a sheet pin.

The rule lives in `signex-net` (`best_net_name_of`), and the ERC rule is
`crates/signex-erc/src/rules/multiple_names.rs`.
