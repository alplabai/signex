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
# hover text from above; the note after the dash is menu-planned-tooltip.
chrome-active-bar-planned-tooltip = { $name } — { menu-planned-tooltip }

## The corner-mode pill shown while drawing a wire or a bus

chrome-active-bar-corner-90 = 90°
chrome-active-bar-corner-45 = 45°
# The wire may leave at any angle.
chrome-active-bar-corner-any = Any

## Page tabs: document tabs that show one of a project's pages (#2523)

# A page tab's title: the project's name, then the page's own name, e.g.
# "sensor-node — Layer Stack". $project is the project name as the user
# wrote it; $page is the page name below or another catalog page name.
chrome-page-tab-title = { $project } — { $page }
# The page name of the Project tab (Project ▸ Project Options...).
chrome-page-tab-project = Project
