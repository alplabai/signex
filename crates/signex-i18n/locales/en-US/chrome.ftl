# The application chrome around the documents: the window title, the tab
# strip and the Active Bars (#2059).

## Window title

# The main window's title. $version is the Signex version number; $count is
# how many documents have unsaved changes. While any are unsaved, the title
# starts with a dot.
chrome-window-title = { $count ->
        [0] { -brand-name } { $version }
       *[other] • { -brand-name } { $version } — { $count } unsaved
    }

## Tab strip

# Hover text of a document tab whose sheet was written by a newer Signex.
# $path is the file's full path, on the first line.
chrome-tab-read-only-tooltip =
    { $path }
    Read-only: written by a newer { -brand-name }. Edit it in that version.

## Schematic Active Bar: the hover text of each button

chrome-active-bar-selection-filter = Selection Filter
chrome-active-bar-move = Move / Transform
chrome-active-bar-select = Select
chrome-active-bar-align = Align
chrome-active-bar-wiring = Wiring
chrome-active-bar-power-port = Power Port
chrome-active-bar-harness = Harness
chrome-active-bar-sheet-symbol = Sheet Symbol
chrome-active-bar-port = Port / Connector
chrome-active-bar-directives = Directives
chrome-active-bar-text = Text
chrome-active-bar-drawing-tools = Drawing Tools
chrome-active-bar-net-color = Net Color
# A button whose tool is planned but not built yet; $name is the button's
# hover text from above.
chrome-active-bar-planned-tooltip = { $name } — Planned — not available yet

## The corner-mode pill shown while drawing a wire or a bus

chrome-active-bar-corner-90 = 90°
chrome-active-bar-corner-45 = 45°
# The wire may leave at any angle.
chrome-active-bar-corner-any = Any
