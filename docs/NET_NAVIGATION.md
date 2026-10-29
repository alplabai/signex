# Net navigation: partners, highlight and hover

Three features read the same connectivity as the netlist export: jumping to a
port's partner on another sheet (#1272), a net highlight that stays on across
sheet tabs (#1271), and a hover card with the net's name, class and pins
(#1270). None of them derives a net itself.

## Where the connectivity comes from

`signex_net::build_project_netlist_indexed` runs the project stitch and keeps,
for every project net, the elements on it: component pins, labels, sheet
entries, power ports, wires and junctions, each with its sheet and placement
(`NetMember`). `NetIndex` answers the questions over that list:

- `partners(sheet, id)`: the other ports on the element's net.
- `steps(net)`: the members to step through, in walk order.
- `highlight_elements(net)`: every drawn element of the net, per sheet.
- `pin_near(sheet, point, tolerance)`: the pin tip under a point.
- `hover(net, classes)`: name, class and pins with their sheets.

A *port* is a Global or Hierarchical label, a sheet entry or a power port (its
partners are the same-rail power ports on other sheets). A net with no
component pin (only labels and sheet entries) is kept too, so ports that join
across sheets still find each other.

## Partners and cycling

`partners` returns the ports of the net in ring order starting after the
element itself, one per element. Go to net partner takes the first, so running
it again from the partner lands on the next, and the last one leads back to the
first. A local net label is not a port and has no partners. When the net has no
other port, the Messages panel says so.

## The cached index

The index is built on a blocking thread from a snapshot of the open sheets (the
other sheets are read from disk there), and cached per project under the same
`ProjectKey` the net-class styles use: it changes on an edit that can change a
net, a sheet list or root change, or a class-table edit. A request made while
the index is missing or stale starts the build and is kept as pending; the
answer re-runs it. Pointer moves never reach the index: the hover reads it only
once the dwell is up.

Opening a sheet that was not open yet changes the key (an open sheet supplies
live content), so the first step onto such a sheet is followed by one rebuild.

## Highlight

`NetHighlight` lives in `DocumentState::net_nav`. It names the net by the
element it was started from and the net's name, so a rebuilt index finds the
same net after an edit. Every schematic canvas gets the part for the sheet it
shows (`CanvasSlot::set_net_highlight`); the renderer's focus dimming keeps
those elements bright and fades the rest. A sheet the net does not reach shows
everything dimmed. Tab switches and new tabs re-project it.

## Hover

The canvas hit test feeds a dwell candidate (object and since when). A timer
subscription runs only while a candidate is unanswered; when the dwell is up
the card is built from the cached index and stored in the Model, so `view` only
draws it. Turn it off under Preferences, Schematic, Editing, "Show net info on
hover" (default on; key `net_hover_enabled` in `prefs.json`).

## Commands

| Command id | Does |
|---|---|
| `go_to_net_partner` | Jump to the next partner of the selected port. |
| `sheet_navigation_back`, `sheet_navigation_forward` | Step through the sheets visited. |
| `leave_sheet`, `navigate_up_hierarchy` | Up to the parent, landing on the sheet symbol. |
| `highlight_net_under_cursor` | Highlight the pointed-at (else selected) net on every sheet. |
| `highlight_related_net_objects` | Highlight the selected net and select its members here. |
| `next_highlighted_net_item`, `previous_highlighted_net_item` | Step through the net's members across sheets. |
| `clear_net_highlighting` | Remove the highlight. |

A sheet placed more than once is read per placement: hover, partners and
related selection use the placement the tab was entered through (the first one
when the tab was opened directly).

A highlighted net is found again after an edit by its anchor element, then by
its name, never by an auto-assigned `N$k` name; if neither finds it, the
highlight is cleared and Messages says so. At most one index build runs per
project; a newer request waits for it and is built once after.

A project is required: a loose schematic with no `.snxprj` has no hierarchy to
read, and the commands say so in the Messages panel.
