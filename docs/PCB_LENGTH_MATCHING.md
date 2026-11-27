# Length Matching

A board's length-matching groups (#2797, mockup row 107): named sets of
nets that should route to one target length within a tolerance. They show
as a document tab and as a compact dock panel; one model, two pure views
(DECISIONS 2026-11-18, "Tab + compact panel pair").

## Opening it

- Length Matching... (`length_matching`) on the board in view opens the
  tab after the board's tab, or focuses it. It has no default key and no
  menu row yet (Design ▸ Length Matching... waits on the menu golden).
- The dock panel (Panels ▸ Length Matching, beside the board) has Open as
  tab in its header; the tab's toolbar has Show as panel.

The tab is a page of its board, like the Polygon Manager: it shows the
board's dirty dot, and Save and Undo act on the board.

## The groups

Each group has a name, a target and a tolerance (mm), a topology (Star,
Chain, Fly-by or T) and its members, nets by name. A net sits in at most
one group: adding a net that is in another group moves it.

A member's length is its net's routed length (`pcb_geom::net_routed_length_mm`:
every track and copper arc of the net, added up), the length the Track
panel and the Net Length report show. It is not yet measured pad to pad
along the topology's path, and via barrels do not count. Each member reads:

| Status | When |
|---|---|
| in band | within target ± tolerance, compared in whole nanometres |
| over | longer than the band: a reroute must shorten it |
| short | shorter than the band: tuning can lengthen it |
| unrouted | the net has no routed copper |
| missing | the board has no net of that name (kept, so a renamed net does not drop out silently) |

Over and short count as out of band. A member that is one half of a
differential pair carries the pair's name.

## The tab

- **Toolbar.** Show (All, Out of band), Show as panel, and the status
  "N groups · M members" with "K out of band" or "all in band".
- **New group.** Name, Target and Tolerance, then Add Group. The new group
  is picked for editing.
- **Groups.** Each group's header names it, its member count, the band,
  the topology and the rule it stands for (`Rule: MatchedLengths_<name>`),
  with a pill for the members out of band. Every member has its length,
  its Δ from the target and a bar against the band (`signex_widgets::compact_group`).
- **Editing.** A click on a group's header picks it: Name, Target and
  Tolerance apply on Enter, Topology on pick, Add net adds a net of the
  board, a picked member shows Remove, and Delete group removes the group
  (its nets stay). Each is one board undo step labelled Length Matching.

## The dock panel

Open as tab, the Show filter, then the same groups; a header click folds a
group. The panel reads the board each time the panel context is rebuilt,
so it follows the board while routing.

## Commands

| Command | Operands | Answer |
|---|---|---|
| `length_matching` | `document` | `{document, groups: [group]}` |
| `set_length_group` | `document`, `name`, `rename?`, `target?`, `tolerance?`, `topology?` (`star`, `chain`, `fly_by`, `t`), `members?` (comma separated) | `{document, groups: [the group]}` |
| `remove_length_group` | `document`, `name` | `{document, groups: [the rest], removed}` |

A group in an answer is `{name, rule, target_mm, tolerance_mm, topology,
out_of_band, unrouted, members: [{net, length_mm, delta_mm, status}]}`.
`set_length_group` makes the group when the board has none of that name;
then `target` and `tolerance` are both needed. Each edit is one undo step
on the named board. Without operands (the palette) the two edits say to
use the tab.

## File format

The groups are board options: `[[board_options.length_groups]]` in the
`.snxpcb`, with `uuid`, `name`, `target` and `tolerance` (integer nm),
`topology` (the word above) and `members` (net names). A board with a group
writes `snxpcb/1.12` ([FORMAT_VERSIONS.md](FORMAT_VERSIONS.md)); a board
without one keeps its token, byte for byte. A group whose name is empty,
whose target is not above zero, whose tolerance is negative or whose
topology is unknown refuses the file.

## Not done yet

- The maintainer's 2026-11-18 decision makes target, tolerance and
  topology the values of a High Speed rule in the Rules manager. The rule
  language has no length kind yet (the Matched Length columns read "Not
  checked yet"), so the group holds the values and names its rule; moving
  them into the rule waits on that kind. Delete group therefore has no
  "also delete the rule" choice.
- xSignals (a member through a series part), dragging a member between
  groups, the topology diagram, a row click that highlights the net on the
  board, and the bar of the net being routed moving live are not built.
