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
drc-panel-waived-count =
    { $count ->
        [one] 1 violation waived
       *[other] { $count } violations waived
    }
drc-panel-group = { $rule } ({ $count })
drc-panel-objects =
    { $count ->
        [one] 1 object
       *[other] { $count } objects
    }
drc-panel-none-shown = No violations match the filters.
drc-panel-waive = Waive
drc-panel-unwaive = Unwaive
drc-panel-waive-note = Why is this violation accepted?
drc-panel-waive-confirm = Waive
drc-panel-waive-cancel = Cancel
drc-panel-waived-note = Waived: { $note }
drc-panel-waived-no-note = Waived
drc-panel-rule-link = Rule
# The rule link of a group, while the Rules table is not on boards yet.
drc-panel-rule-link-unavailable = Opens this rule in the Rules table, which boards do not have yet.
# Undo labels.
drc-panel-waive-undo = Waive DRC Violation
drc-panel-unwaive-undo = Unwaive DRC Violation
drc-panel-refused = The waiver was not saved: { $why }
drc-panel-show-refused = The violation could not be shown: { $why }

board-net-classes-no-board = Open a board to edit its net classes.
board-net-classes-no-project = This board is in no open project, so it has no net classes. Open its project.
board-net-classes-add = Add Class
board-net-classes-delete = Delete
board-net-classes-undo = Undo net class edit
board-net-classes-redo = Redo net class edit
board-net-classes-empty = No net classes yet. Add one, then pick its nets.
board-net-classes-col-name = Name
board-net-classes-col-color = Colour
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
board-net-classes-invalid-color = The colour must be #RRGGBB or #RRGGBBAA, or empty for none.
