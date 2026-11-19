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
# $seed names the object the search starts from, such as "Symbol R1", in
# English.
find-similar-seed = Seed: { $seed }

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
