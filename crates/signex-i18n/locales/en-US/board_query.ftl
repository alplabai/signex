# Measure, Find, Find Similar and the nets on a board (#2403).

## Measure Distance on a board
board-measure-hint = Measure: click a point or an object, then another. Esc ends.
# { $distance }, { $dx } and { $dy } are lengths with their unit.
board-measure-distance = Distance { $distance } (dx { $dx }, dy { $dy })
# { $gap } is a length with its unit.
board-measure-gap = Edge to edge { $gap }

## Find Similar Objects on a board
board-similar-no-seed = Find Similar: hover one board object or select a single one to use as the seed
board-similar-gone = Find Similar: the seed object is no longer on the board
board-similar-none = Find Similar: nothing on the board matches those properties
board-similar-selected = Find Similar: selected { $count } on the board

## Find and Replace on a board
board-find-targets = Targets: designators, values, footprints, nets
board-find-kept = Replace writes designators and values; { $count } net or footprint result(s) left as they are
board-find-replaced = Replaced the text of { $count } footprint(s)
board-find-nothing-replaced = Nothing to replace: no designator or value result holds the text

## Board nets: Select Net, Highlight Net and the Net Inspector
board-net-nothing = Select or point at something on a net first
board-net-unknown = The board has no net named { $net }
board-net-highlighted = Net { $net } is highlighted
board-net-unhighlighted = Net { $net } is no longer highlighted
board-net-selected = Selected net { $net }: { $count } object(s)
board-nets-column-length = Length
board-nets-column-vias = Vias
board-nets-column-pads = Pads
board-nets-column-state = State
board-nets-routed = Routed
# { $count } connections of the net are still to route.
board-nets-open = { $count } open
board-nets-select = Select
board-nets-highlight = Highlight
board-nets-summary = { $shown } of { $total } nets
board-nets-empty = No net of the board matches the filter

## Board items an agent or the CLI names by uuid (#2326)
# { $id } is the uuid; the whole command is refused, nothing changes.
board-target-unknown = { $id } names nothing on the board this command acts on; nothing was changed
# A keepout is made from the named zones or from the named lines, never both.
board-keepout-mixed = A keepout is made from zones or from lines, not both; nothing was changed
