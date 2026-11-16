# Text inside the panels: History, ERC waivers, colours, Projects,
# multi-selection, Components and the Properties panel (#2059).

## History panel
panels-history-no-active-file = Open a schematic, PCB, library, or primitive to see its history.
panels-history-no-repo = This file isn't under version control. Enable from the project's right-click menu.
panels-history-loading = Loading history…
# $reason is why reading the history failed, as the version-control tool
# words it.
panels-history-read-failed = History could not be read: { $reason }
panels-history-restore = Restore this version
# Shown on the restore button when the document has unsaved edits.
panels-history-discard-and-restore = Discard unsaved edits and restore
panels-history-cancel = Cancel
# A pseudo-entry above the commits while the document has unsaved changes.
panels-history-working-tree = Working tree
panels-history-uncommitted = Uncommitted changes
# How long ago a commit was made; $count is a whole number. The unit is
# abbreviated: s seconds, m minutes, h hours, d days, mo months, y years.
panels-history-ago-seconds = { $count }s ago
panels-history-ago-minutes = { $count }m ago
panels-history-ago-hours = { $count }h ago
panels-history-ago-days = { $count }d ago
panels-history-ago-months = { $count }mo ago
panels-history-ago-years = { $count }y ago

## ERC panel: per-finding waivers
panels-erc-waive = Waive
panels-erc-waiver-cancel = Cancel
# $count is how many findings the stored waivers hide, a whole number. The
# English keeps "findings" for every count.
panels-erc-findings-waived = { $count } findings waived
# The status of one stored waiver.
panels-erc-waiver-status-one = waived
# $count is a whole number above one.
panels-erc-waiver-status-many = waives { $count } findings
panels-erc-waiver-status-lapsed = LAPSED: an involved object changed
panels-erc-waiver-status-unmatched = matches no finding
panels-erc-waiver-status-incomplete = ignored: no reason or author
# One stored waiver, on two lines: $rule the rule id, $message the finding,
# $status one of the statuses above; then $reason, $author and $date as the
# user wrote them. Keep the double spaces and the indent of the second line.
panels-erc-violation-waiver-row =
    { $rule }  { $message }  ({ $status })
    {"  "}Reason: { $reason }  by { $author } { $date }
