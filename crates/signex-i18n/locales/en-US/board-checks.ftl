# The DRC results panel of a board (#2399) and the board Net Classes panel (#2400).

drc-panel-no-board = Open a board to see its design rule check.
drc-panel-filter-all-rules = All rules
drc-panel-filter-all-layers = All layers
drc-panel-filter-all-severities = All severities
drc-panel-severity-error = Error
drc-panel-severity-warning = Warning
drc-panel-severity-info = Info
drc-panel-severity-off = Off
drc-panel-show-waived = Show waived
drc-panel-group = { $rule } ({ $count })
drc-panel-objects =
    { $count ->
        [one] 1 object
       *[other] { $count } objects
    }
drc-panel-none-shown = No violations match the filters.
drc-panel-waive = Waive
drc-panel-waive-placeholder = Why is this violation accepted?
drc-panel-unwaive = Unwaive
drc-panel-waived-note = Waived: { $note }
drc-panel-waived-no-note = Waived
drc-panel-rule-link = Rule
# Undo labels.
drc-panel-waive-undo = Waive DRC Violation
drc-panel-unwaive-undo = Unwaive DRC Violation
drc-fix-undo = Fix DRC Violations
drc-panel-refused = The waiver was not saved: { $why }
drc-panel-show-refused = The violation could not be shown: { $why }

board-net-classes-no-board = Open a board to edit its net classes.
board-net-classes-no-project = This board is in no open project, so it has no net classes. Open its project.
# A refused net-class edit; $why is the reason, in English.
board-net-classes-not-changed = The net classes were not changed: { $why }
board-net-classes-add = Add Class
board-net-classes-delete = Delete
board-net-classes-undo = Undo net class edit
board-net-classes-redo = Redo net class edit
board-net-classes-empty = No net classes yet. Add one, then pick its nets.
board-net-classes-col-name = Name
board-net-classes-col-color = Color
board-net-classes-col-width = Width
board-net-classes-col-clearance = Clearance
board-net-classes-col-via = Via
board-net-classes-col-drill = Drill
board-net-classes-col-dp-gap = DP Gap
board-net-classes-col-dp-width = DP Width
board-net-classes-col-nets = Nets
board-net-classes-add-net = Add net
board-net-classes-remove-net = Remove { $net } from the class
board-net-classes-mm-hint = mm
board-net-classes-color-hint = #RRGGBB
board-net-classes-patterns =
    { $count ->
        [one] also 1 pattern
       *[other] also { $count } patterns
    }
board-net-classes-invalid-number = { $field } must be a number of millimetres, or empty for the board's value.
board-net-classes-invalid-color = The color must be #RRGGBB or #RRGGBBAA, or empty for none.

## DRC panel (#87, #88): the idle header, notices, count tiles and rows.
drc-panel-rules = Rules
drc-panel-default-clearance = No clearance rule set on this board; checked with { $clearance } mm.
drc-panel-default-clearance-written = No clearance rule set on this board; { $clearance } mm applies where no written rule does.
# A link after the default-clearance notice: opens the Rules tab on Clearance.
drc-panel-set-in-rules = Set it in Rules
drc-panel-see-messages = see Messages
drc-panel-rules-not-applied =
    { $count ->
        [one] 1 written rule was not applied;
       *[other] { $count } written rules were not applied;
    }
drc-panel-rule-text-unparsed = The rule text has a syntax error and was not applied;
drc-panel-keepouts-skipped =
    { $count ->
        [one] 1 keepout could not be checked;
       *[other] { $count } keepouts could not be checked;
    }
# Captions under the count tiles, short to fit four in a row.
drc-panel-tile-clearance = clearance
drc-panel-tile-short = short
drc-panel-tile-unconnected = unconn.
drc-panel-tile-keepout = keepout
drc-panel-tiles-hint = Click a tile to list only that category.
# $category is one of the category names below.
drc-panel-tiles-showing = Showing { $category } only
drc-panel-category-clearance = clearance violations
drc-panel-category-short = shorts
drc-panel-category-unconnected = unconnected links
drc-panel-category-keepout = keepout violations
drc-panel-clear-filter = Clear filter
drc-panel-show-waived-count =
    { $count ->
        [one] (1 violation waived)
       *[other] ({ $count } violations waived)
    }
drc-panel-show = Show
# The second line of a row: layer, position and how many objects.
drc-panel-row-place = { $layer } · { $place } · { $objects }
drc-panel-row-place-no-layer = { $place } · { $objects }
