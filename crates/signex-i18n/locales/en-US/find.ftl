# Find and Replace, Find Similar Objects and Paste Array (#2058).

## Find and Replace
find-title = Find
find-label = Find
find-replace-label = Replace
find-scope = Scope
find-scope-sheet = Sheet
find-scope-project = Project
find-replacing = Replacing...
find-searching = Searching...
# $count is how many matches (a whole number). Shown in a pill inside the
# Find field.
find-matches = { $count ->
    [one] 1 match
   *[other] { $count } matches
}
find-no-matches = No matches
find-replace-current = Replace Current
# Help text beside the Scope switch: what a sheet search reads.
find-targets = Searches labels, notes, designators, values
# The header switch between Find and Replace (#2525).
find-mode-find = Find
find-mode-replace = Replace
# Tooltips of the three match toggles beside the Find field (#2525).
find-match-case = Match case
find-whole-word = Whole word
find-regex = Regular expression
# The Find text is not a valid regular expression (#2525).
find-invalid-pattern = Invalid pattern
# Head of the results' right column: the sheet a result is on.
find-sheet-column = Sheet
# $count is how many results a group (Net Label, Value, ...) holds.
find-group-count = { $count }
# $current is the position of the selected result, $total how many there are.
find-match-position = Match { $current } of { $total }
# The primary button. $included is how many results have their include box
# set, $total how many there are (#2525).
find-replace-included = Replace { $included } of { $total }
# Footer note under the match position, in Replace mode.
find-one-undo-step = Replace All is one undo step
# Faces of the three match toggles beside the Find field; their names are
# the tooltips (find-match-case, find-whole-word, find-regex).
find-match-case-face = Aa
find-whole-word-face = ab
find-regex-face = .*
# Between a result's text and its text after the replace.
find-replace-arrow = {" "}→{" "}
# Before the note naming the sheets a project search skipped.
find-skipped-glyph = ⚠

## Find Similar Objects
find-similar-title = Find Similar Objects
find-similar-col-property = Property
find-similar-col-seed = Seed value
find-similar-col-match = Match
# A property whose value is empty.
find-similar-empty = (empty)
find-similar-same = Same
find-similar-any = Any
find-similar-scope-sheet = Current sheet
find-similar-scope-project = Whole project
find-similar-footer = Selects every match. Matches on other sheets are only counted.
# $seed names the object the search starts from, such as "Part R1", in
# English.
find-similar-seed = Seed: { $seed }
# The kind name of a placed part, in the Kind cell and the seed line.
find-similar-part = Part

## Paste Array
paste-array-title = Paste Array
paste-array-copies = Copies
# $unit is the length unit, such as "mm".
paste-array-x-spacing = X spacing ({ $unit })
# $unit is the length unit, such as "mm".
paste-array-y-spacing = Y spacing ({ $unit })
paste-array-step = Text increment step
paste-array-designators = Designators
paste-array-hint = Copy k sits k spacings from the original; label numbers count up by k steps.

## Find Similar Objects: counts and the Select button (#2526)
# $count is how many objects match on the dialog's sheet.
find-similar-count-sheet = { $count } on this sheet
# $count is how many objects match on the dialog's board.
find-similar-count-board = { $count } on this board
# $count is how many objects match on the project's other sheets.
find-similar-count-elsewhere = { $count } more on other sheets
# The primary button; $count is how many objects it selects.
find-similar-select = Select { $count }

## Paste Array: groups, preview and footer (#2527)
# Header subtitle; $count is how many items one copy holds.
paste-array-subtitle = { $count ->
    [one] 1 item per copy
   *[other] { $count } items per copy
}
paste-array-group-layout = Layout
paste-array-group-numbering = Numbering
paste-array-spacing = Spacing
# Prefix inside the X spacing field.
paste-array-axis-x = X
# Prefix inside the Y spacing field.
paste-array-axis-y = Y
paste-array-count-up = Count up label numbers
paste-array-preview-caption = Preview · copy 1 is the original
# Tooltips of the preview's expand button.
paste-array-preview-expand = Expand preview
paste-array-preview-collapse = Collapse preview
# Beside the last drawn copy; $count is how many positions are not drawn.
paste-array-preview-more = +{ $count } more
# Spacing callout on the preview; $value is the length with its unit.
paste-array-preview-x = X { $value }
# Spacing callout on the preview; $value is the length with its unit.
paste-array-preview-y = Y { $value }
# Worked example; $from is a copied label and $to its texts in the first
# copies, such as "D1, D2, D3".
paste-array-hint-example = Copy k sits k spacings from the original; label numbers count up by k steps ({ $from } → { $to }).
paste-array-hint-same = Label numbers are not counted up: every copy keeps its label text, so the labels of all copies join one net.
# Footer; $count is how many items the paste places.
paste-array-items = { $count ->
    [one] 1 item will be placed
   *[other] { $count } items will be placed
}
# The primary button; $count is how many new copies it places, one less
# than the Copies field, which counts the original.
paste-array-paste = { $count ->
    [one] Paste 1 copy
   *[other] Paste { $count } copies
}
# The primary button while the Copies field does not parse.
paste-array-paste-none = Paste

## Names the engine gives (#2060)
# Kind of a match in the results list: a net label on the sheet (a noun,
# not the Place command).
find-kind-net-label = Net Label
# Kind of a match in the results list: a text note on the sheet.
find-kind-text-note = Text Note
# Property names in the Find Similar table.
find-similar-prop-kind = Kind
find-similar-prop-library-part = Library part
# The letters a designator starts with, such as "R" for "R12".
find-similar-prop-designator-prefix = Designator prefix
find-similar-prop-net-name = Net name
# Net, global, hierarchical or power label.
find-similar-prop-label-type = Label type
