# Canvas overlays.

# Placement Paused banner: Esc resumes placing without the panel edits.
overlay-placement-paused-esc-hint = Esc back to placing

## Graphite views (#2225)
overlay-placement-paused-title = Placement Paused
overlay-placement-paused-body = Editing properties in the panel.

## Command palette (#2057)
# Key caps of the palette's hint line.
palette-key-esc = Esc
palette-key-enter = Enter
# Row that opens a panel. { $panel } is the panel's name.
palette-open-panel = Open Panel: { $panel }
# An installed extension's command in the palette and the keyboard
# shortcuts: { $extension } is the extension's name, { $command } the
# command's own label.
extension-command-label = { $extension }: { $command }
# A shortcut kept for an extension command no installed extension adds:
# { $command } is the command id. The binding stays and does nothing.
extension-command-unavailable = { $command } (unavailable: extension not installed)

## Command palette. The group headings are shown in capitals, so they are
## written in capitals here.
overlay-palette-group-commands = COMMANDS
overlay-palette-group-components = COMPONENTS
overlay-palette-group-nets = NETS
overlay-palette-group-sheets = SHEETS
overlay-palette-group-files = FILES
# Shown when nothing matches the typed query.
overlay-palette-no-results = No results
# Placeholder of the search field of a palette opened in its own window.
overlay-palette-window-search = Search files, symbols, commands…
# Footer note: matches beyond the rows shown. $count is a whole number.
overlay-palette-more-results = { $count } more results — refine query
# Footer note when every match is shown: how many rows matched. $count is a
# whole number.
overlay-palette-result-count = { $count ->
    [one] 1 result
   *[other] { $count } results
}
# Footer key hints: the arrow keys move the selection, Enter runs it.
overlay-palette-navigate = Navigate
overlay-palette-run = Run

## Hover card of a placed symbol: the field names under its designator
overlay-symbol-card-value = Value
overlay-symbol-card-footprint = Footprint
overlay-symbol-card-library = Library
overlay-placement-paused-resume = Resume Placement

## Hover card of a net
# $class is the name of the net's net class.
overlay-net-card-class = Class: { $class }
# The card lists a few pins; $count is how many more it leaves out, a whole
# number.
overlay-net-card-more = +{ $count } more
# Pill beside the net's name: how many pins the net has; $count is a whole
# number.
overlay-net-card-pin-count =
    { $count ->
        [one] { $count } pin
       *[other] { $count } pins
    }

# Badge over the canvas of a sheet written by a newer Signex, which opens
# read-only.
overlay-read-only-badge = Read-only: written by a newer { -brand-name }

# Over a window that a dialog in its own window has locked.
overlay-locked-window-hint = A dialog is open: click to show it, Esc to cancel

# Tooltip of the read-only badge: why the sheet cannot be edited and what to
# do about it.
overlay-read-only-badge-tooltip = This sheet was saved by a newer { -brand-name }. Edits are off so nothing it wrote is lost. Update { -brand-name } to edit it.
# Key hint above the in-place text editor, shown for its first few uses.
overlay-text-edit-hint = Enter apply · Esc cancel
# The same hint on a part's field that has a next field to Tab to.
overlay-text-edit-hint-next-field = Enter apply · Esc cancel · Tab next field
# The rich text note editor over the sheet (#1114, mockup row 203). Its title
# names the sheet file and the note's position in mm.
overlay-rich-note-title = Typst · { $sheet } note at { $x }, { $y }
# Above the live preview: the width the note draws at, in mm.
overlay-rich-note-preview = As drawn on the sheet · { $width } mm wide
overlay-rich-note-compile-ms = { $ms } ms
overlay-rich-note-compiling = Compiling…
# The first compile error, under the source: its line and the compiler's words.
overlay-rich-note-error = Line { $line }: { $message }
# The key hint under the editor; $apply and $click are this OS's key names.
overlay-rich-note-hint = { $apply } apply · Esc cancel · links open on { $click }
overlay-rich-note-apply = Apply
overlay-rich-note-cancel = Cancel
# The toolbar: each wraps the selection in Typst markup.
overlay-rich-note-wrap-bold = Bold
overlay-rich-note-wrap-italic = Italic
overlay-rich-note-wrap-heading = Heading
overlay-rich-note-wrap-list = List
overlay-rich-note-wrap-math = Math
overlay-rich-note-wrap-link = Link
overlay-rich-note-wrap-table = Table
# A Messages line: a rich note whose source does not compile. The sheet keeps
# its last good drawing with a red corner mark.
overlay-rich-note-compile-error = The rich text note at { $x }, { $y } does not compile: line { $line }: { $message }
# A Messages line: a note's link did not open.
overlay-rich-note-link-failed = The link { $target } did not open: { $why }
