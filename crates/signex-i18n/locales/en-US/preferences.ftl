# Preferences dialog.

# Label of the Preferences checkbox that lets a local agent connect.
preferences-agent-connections-label = Allow agent connections

# Help text under that checkbox.
preferences-agent-connections-help = When on, Signex listens for agents running as you on this computer, through a socket only you can open. An agent names its document and edits it without changing your tab, selection or tool; every request it makes is listed in Messages. Turning this off closes the socket and drops its connections. Off by default.

# Label of the Mouse wheel setting (Zoom or Scroll).
preferences-wheel-mode-label = Mouse wheel

# Help text of the Mouse wheel setting.
preferences-wheel-mode-help = Zoom: the mouse wheel zooms the canvas. Scroll: the wheel and a two-finger scroll pan vertically, Shift + wheel pans horizontally, and Ctrl/Cmd + wheel or a pinch zooms. Default: Zoom.

## Theme page.

# Page heading.
preferences-theme-heading = Theme

# Line under the heading.
preferences-theme-intro = Colours, density and canvas text. Changes preview live; Apply keeps them.

# Section label over the theme cards, shown in capitals.
preferences-theme-application = APPLICATION THEME

# Theme names are product names: Graphite is the name of the look.
preferences-theme-light-name = Graphite Light
# Tag on the light theme's card: it is the default theme.
preferences-theme-light-tag = Default
preferences-theme-light-description = Warm paper canvas, graphite ink, copper accent.

preferences-theme-dark-name = Graphite Dark
# Tag on the dark theme's card.
preferences-theme-dark-tag = Dark
preferences-theme-dark-description = Low-glare charcoal for long routing sessions.

## Dialog shell.

# Placeholder of the search field above the page list.
preferences-search = Search preferences

# Dialog subtitle: the open page's group, then the page.
preferences-subtitle = { $group } / { $page }

# Footer pill while there are changes not applied yet.
preferences-unsaved = Unsaved changes

# Footer hint on the Appearance page.
preferences-theme-preview-hint = Theme changes preview live

# Page names in the page list.
preferences-page-theme = Appearance
preferences-page-keyboard = Keyboard shortcuts
# The page of graphical editing options (selection, dragging, paste).
preferences-page-editing = Editing
preferences-page-annotation = Annotation
# The library of named electrical rule check (ERC) rule sets.
preferences-page-rule-profiles = Rule Profiles
# API keys of part distributors.
preferences-page-distributors = Distributor APIs
# The component classes a new library starts with.
preferences-page-component-classes = Component Classes

# Page groups: the name in the subtitle, and the heading (in capitals) over
# the group in the page list.
preferences-group-system = System
preferences-group-system-heading = SYSTEM
preferences-group-schematic = Schematic
preferences-group-schematic-heading = SCHEMATIC
# Rule checking.
preferences-group-validation = Validation
preferences-group-validation-heading = VALIDATION
preferences-group-library = Library
preferences-group-library-heading = LIBRARY

## Graphite views (#2225)
preferences-appearance-ui-font = UI Font
preferences-appearance-ui-font-help = Applies to all panels and menus. Requires restart.
preferences-appearance-grid-style = Grid Style
preferences-appearance-schematic-grid-style-help = Appearance of snap points on the schematic canvas.
preferences-appearance-fast-grid-1 = Fast grid 1
preferences-appearance-fast-grid-1-help = Snap grid set by the Fast grid 1 command, in mm. Cycle fast grid steps between the two fast grids. Default 1.27 mm (50 mil).
preferences-appearance-fast-grid-2 = Fast grid 2
preferences-appearance-fast-grid-2-help = Snap grid set by the Fast grid 2 command, in mm. Default 2.54 mm (100 mil).
preferences-appearance-fast-grid-unit = mm
preferences-appearance-fast-grid-refused = Preferences were not saved. Fast grid { $number }: { $reason }.
preferences-appearance-fast-grid-below-minimum = { $value } mm is finer than the 1 nm (0.000001 mm) minimum
preferences-appearance-gpu-render = GPU Render (experimental)
preferences-appearance-gpu-render-help = Draw the PCB canvas on the GPU (shader) instead of the CPU. Applies immediately.
preferences-appearance-antialiasing = Anti-aliasing (MSAA 4x)
preferences-appearance-antialiasing-help = Smooth the edges of canvas shapes. Applies after restart.
preferences-appearance-symbol-grid-size = Default Grid Size
preferences-appearance-symbol-grid-size-help = Applied when a symbol library is first opened. Can be changed per-library from the canvas status bar.
preferences-appearance-symbol-grid-style-help = Appearance of snap points on the symbol editor canvas.
preferences-appearance-pin-selection = Pin Selection
preferences-appearance-pin-selection-help = How a click selects a pin on the symbol editor canvas — the pin body only, or its name and number labels too.
preferences-appearance-ui-scale = Interface scale
preferences-appearance-ui-scale-help = Scales text, panels, menus and dialogs on top of the display scale. Applied to every window when you save.
preferences-appearance-zoom-speed = Zoom speed
preferences-appearance-zoom-speed-help = How far one mouse-wheel notch zooms on the schematic, PCB, symbol and footprint canvases.
preferences-appearance-trackpad-pan = Two-finger scroll pans
preferences-appearance-trackpad-pan-help = When on, a two-finger trackpad scroll pans the schematic, PCB, symbol and footprint canvases instead of zooming them. Pinching and Ctrl/Cmd + scroll still zoom, and a mouse wheel notch still zooms. Off by default.
preferences-appearance-remember-geometry = Remember window size and position
preferences-appearance-remember-geometry-help = When on, Signex reopens the main window at the size and position it had when you last closed it. Off by default.
preferences-appearance-window-buttons = Window buttons
preferences-appearance-window-buttons-help = Where minimize, maximize and close sit. Automatic follows your system.
preferences-appearance-restart-note = Takes effect after restart.

## Keyboard shortcuts (#2057)
# Column headers shared by the Keyboard Shortcuts sheet and the preferences table.
keymap-col-action = Action
keymap-col-shortcut = Shortcut
# Names of the command groups, over the groups of the preferences table.
keymap-group-general = General
keymap-group-schematic = Schematic
keymap-group-pcb = PCB
keymap-group-3d = 3D
# Where a shortcut applies.
pref-keymap-context-global = Global
pref-keymap-context-schematic = Schematic
pref-keymap-context-footprint = Footprint
pref-keymap-context-pcb = PCB
pref-keymap-context-library = Library
pref-keymap-context-modal = Modal
pref-keymap-context-text-input = Text Input
pref-keymap-context-command-palette = Command Palette
pref-keymap-context-placement = Placement
pref-keymap-intro = Configure keyboard shortcut profiles. Command names and categories come from Signex command metadata.
# Banner when the saved shortcuts file could not be loaded. { $location } is its path, { $error } the reason.
pref-keymap-load-error = Your saved keyboard shortcuts could not be loaded from { $location } — { $error }. The built-in profiles are in use for this session and that file has not been changed. Saving from this pane overwrites it, so the original is kept beside it as a .bak file.
# Stands for the path of the shortcuts file when it is not known.
pref-keymap-usual-location = its usual location
# { $path } is where the backup of the previous shortcuts is kept.
pref-keymap-backup = A backup of your previous keyboard shortcuts is kept at { $path }.
pref-keymap-restore = Restore from backup
pref-keymap-discard-backup = Delete the backup
# Profile picker.
pref-keymap-profile = Profile
# { $count } is the number of bindings of the active profile.
pref-keymap-bindings = { $count } bindings
pref-keymap-no-profile = No active profile
# { $name } is the profile's name.
pref-keymap-profile-built-in = { $name } (built-in)
pref-keymap-profile-custom = { $name } (custom)
pref-keymap-create = Create
pref-keymap-delete = Delete
pref-keymap-import = Import
pref-keymap-export = Export
# Status line under the profile picker.
pref-keymap-conflict-one = 1 conflict in active profile
# { $count } is the number of conflicts, more than one.
pref-keymap-conflicts = { $count } conflicts in active profile
pref-keymap-no-conflicts = No conflicts detected in active keyboard shortcuts.
pref-keymap-search = Search shortcuts…
# Column headers of the table.
pref-keymap-col-category = Category
pref-keymap-col-command = Command
pref-keymap-col-context = Context
pref-keymap-col-state = State
pref-keymap-empty = No shortcuts are defined in the active profile.
# { $query } is what the user typed in the search box.
pref-keymap-no-match = No shortcuts match “{ $query }”.
# What a row's State column says.
pref-keymap-state-invalid = Invalid
pref-keymap-state-unbound = Unbound
pref-keymap-state-reserved = Reserved by macOS
pref-keymap-state-conflict = Conflict
pref-keymap-state-editable = Editable
pref-keymap-state-create-custom = Create custom
pref-keymap-state-gesture = Gesture
# A fixed key of the app claims this stroke first. { $action } names that key's action.
pref-keymap-overridden = Overridden by { $action }
pref-keymap-edit = Edit
# Chord recorder card.
pref-keymap-recorder-title = Edit Shortcut
pref-keymap-recorder-hint = Press Record, then type a shortcut
pref-keymap-recorder-help = Press the exact keystroke to record it. ESC is recorded as a key; use Cancel to close.
pref-keymap-recording = Recording
pref-keymap-recorded = Recorded
pref-keymap-current = Current
pref-keymap-stop = Stop
pref-keymap-record = Record
pref-keymap-cancel = Cancel
pref-keymap-clear = Clear
pref-keymap-ok = OK
# { $display } is the recorded stroke, { $action } the action of the fixed key that takes it.
pref-keymap-claimed = { $display } is claimed by { $action } and will never reach this binding.
# Value beside the zoom speed slider. $percent is a whole number.
preferences-appearance-zoom-step-value = { $percent }% per notch

## Section titles of the pages
preferences-section-font = Font
preferences-section-window-controls = Window controls
preferences-section-schematic-editor = Schematic Editor
preferences-section-pcb-editor = PCB Editor
preferences-section-rendering = Rendering
preferences-section-symbol-editor = Symbol Editor
preferences-section-interface-scale = Interface scale
preferences-section-navigation = Navigation
preferences-section-window = Window
preferences-section-annotation = Schematic — Annotation
preferences-section-editing = Schematic — Editing
preferences-section-distributors = Library — Distributor APIs
preferences-restore-defaults = Restore defaults

## Schematic ▸ Editing (#2058)
preferences-move-keeps-wires = Keep wires connected when moving
preferences-move-keeps-wires-help = When on, moving, rotating or mirroring symbols, child sheets or sheet pins (drag, Move, Move By, align, rotate, mirror) drags the wires on their pins along, adding a bend where needed to keep them horizontal or vertical. A net check afterwards keeps this from changing connectivity beyond what a plain move, rotate or mirror would do; a pin that cannot be kept connected moves alone and is reported in the Messages panel. When off, only the selected items move or turn and attached wires stay where they were. Hold Alt through one drag to do the opposite for that drag only; the Drag with connections command (G in the Classic profile) drags the picked object with its wires whatever this says.
preferences-net-hover = Show net info on hover
preferences-net-hover-help = When on, resting the pointer on a wire, pin or label for a moment shows a card with the net's name, its net class and the pins on it with the sheet each is on. Turn it off if the card gets in the way while you edit.
preferences-drag-direction = Select by drag direction
preferences-drag-direction-help = When on, dragging a selection box from left to right selects only what the box fully contains, and from right to left selects everything the box touches. When off, the Inside / Touching selection mode (Shift+S cycles it) decides for both directions; while on, that mode is not used for box selection.
preferences-increment-step = Increment step
preferences-increment-step-help = How far the trailing number of a text moves each time it is repeated: placing the label D0 again and again gives D1, D2, ... with a step of 1, and Paste Array offers the same step. A negative step counts down; a number at the start or middle of a text is left alone.
preferences-paste-designators = Designators on paste
preferences-paste-designators-help = What pasted parts are called. Keep leaves the designators of the copy as they were (duplicates stay until Annotate runs). Reset turns them into R?, U?, ... for Annotate to number. Next free number gives each part the next unused number of its prefix in the project. Applies to Paste, Smart Paste, Duplicate, rubber-stamp copy and Paste Array; the units of one part always stay one part.
preferences-autosave = Autosave recovery copies
preferences-autosave-help = When on, { -brand-name } keeps a recovery copy of each schematic, symbol and footprint with unsaved changes about once a minute. The copy is a separate file in the { -brand-name } config folder (recovery); your own file is never touched by it. The copy is removed when you save the document, close it without keeping the changes, or quit with Discard All, even if you turn this off first. Off by default.

## Schematic ▸ Annotation
preferences-annotate-undo = Annotate undo
# $undo is the Undo shortcut, such as "Ctrl+Z". The two choices are named as
# preferences-annotate-undo-project and preferences-annotate-undo-per-sheet read.
preferences-annotate-undo-help = How many undo steps one annotate run is. One step for the whole project reverts every sheet the run touched together, from whichever sheet { $undo } is pressed on. Per sheet undoes each sheet on its own. Applies to the next annotate run.

## Default Component Classes
preferences-classes-title = Default Component Classes
preferences-classes-help = Seeds the class registry of newly-created libraries. Per-library edits live inside each .snxlib's manifest (forthcoming Library Properties pane); this list controls only what new libraries inherit.
preferences-classes-key = Key
preferences-classes-label = Label
preferences-classes-remove = Remove
preferences-classes-empty = No classes defined. Click "+ Add Class" to add one or "Reset to Defaults" to restore the seed list.
preferences-classes-add = + Add Class
preferences-classes-reset = Reset to Defaults

## The banner for a preferences file that could not be loaded
# Bold first line of the banner.
preferences-file-broken = Your preferences file could not be read.
preferences-file-defaults = Appearance, ERC severities, component classes and panel layout show their defaults and are not saved. The file itself has not been altered.
preferences-file-start-fresh = Start a fresh preferences file

## Rule Profiles
preferences-profiles-title = Rule Profiles
preferences-profiles-help = A profile is a named set of ERC rules. A new project starts with a copy of the default profile. Changes here are saved to the profile's file at once.
preferences-profiles-reading = Reading the profile library...
preferences-profiles-automatic = Automatic (User Default if you have one, else { -brand-name } Default)
# Notes after a profile's name and version.
preferences-profiles-note-builtin = shipped, read-only
preferences-profiles-note-user-default = this machine's rules
# $name and $version are the profile's.
preferences-profiles-row = { $name }  v{ $version }
# $name and $version are the profile's; $note is one of the notes above.
preferences-profiles-row-note = { $name }  v{ $version }  ({ $note })
preferences-profiles-is-default = Default for new projects
preferences-profiles-use-as-default = Use as default
preferences-profiles-new = New
preferences-profiles-duplicate = Duplicate
preferences-profiles-rename = Rename
# $name is the profile's name.
preferences-profiles-delete-confirm = Delete "{ $name }"? Projects that recorded it keep their rules but can no longer find it for Update from profile.
preferences-profiles-keep = Keep
preferences-profiles-name = Name
# $name is the profile's name.
preferences-profiles-editing = Editing "{ $name }". Changes are saved to the profile's file immediately.
# $name is the profile's name.
preferences-profiles-read-only = "{ $name }" is read-only. Duplicate it to change the rules.
preferences-profiles-none-selected = No profile is selected.
# $name is the profile's name.
preferences-profiles-reset-confirm = Put every rule of "{ $name }" back to the { -brand-name } Default values?
preferences-profiles-reset = Reset
preferences-profiles-reset-to-default = Reset to { -brand-name } Default

## Designators on paste: the choices (also in Paste Array)
# The pasted part keeps its designator unless the project already uses it.
preferences-paste-designators-keep-if-free = Keep unless taken
preferences-paste-designators-keep = Keep
# Keep "R?, U?" as written: designators not numbered yet.
preferences-paste-designators-reset = Reset (R?, U?, ...)
preferences-paste-designators-next-free = Next free number

## Annotate undo: the choices
preferences-annotate-undo-project = One step for the whole project
preferences-annotate-undo-per-sheet = Per sheet

## Preferences shell (#143), Language page (#144a), Annotation page (#146)

# Page name of the interface-language page in the page list.
preferences-page-language = Language

# Footer note on the Language page: when a chosen language takes effect.
preferences-language-footer-note = The language changes when you press Apply or OK; no restart

# Second line of the preferences-file banner. $path is the file; $error says
# what is wrong with it, in English.
preferences-file-where = { $path }: { $error }
# Banner button: shows the folder that holds the preferences file.
preferences-file-open-folder = Open folder

# Language page.
preferences-language-heading = Language
preferences-language-intro = The language of menus, panels, dialogs and messages. Your designs are not changed.
# Section label, shown in capitals.
preferences-language-section-interface = INTERFACE
preferences-language-interface = Interface language
preferences-language-interface-help = Applies to every open window at once. Text on the canvas (designators, values, net names) is your own and is never translated.
# First row of the language list. $language is the language the system
# setting resolves to, named in its own language.
preferences-language-follow-system = Follow the system: { $language }
# The pseudo-locale in the language list: a testing aid, not a language.
# Keep "en-XA" as written.
preferences-language-pseudo = Pseudo-locale (en-XA)
# Label over the sample strings, shown in capitals.
preferences-language-preview = PREVIEW
preferences-language-preview-caption = A few strings from the menus and dialogs in the chosen language.
# Info banner while the pseudo-locale is chosen.
preferences-language-pseudo-note = The pseudo-locale wraps every string in brackets, swaps letters for accented ones and makes it about 30 % longer. A string that shows without brackets is not translatable yet; a label that is cut off needs more room.

# Annotation page.
preferences-annotation-heading = Annotation
preferences-annotation-intro = How annotate runs behave on the schematic.
# Section labels, shown in capitals.
preferences-annotation-section-undo = UNDO
preferences-annotation-section-related = RELATED
# Help of the "Designators on paste" row that points to the Editing page.
preferences-annotation-paste-help = Lives on Editing, because it applies to paste, not annotate.
# Link to the Editing page.
preferences-annotation-open-editing = Open Editing ›

## Appearance page and Editing page (#144, #147)

# Shared by both pages. The link after a shortened help text that shows the
# whole text; the leading space separates it from that text.
preferences-pages-more = { " " }More…
# The link after a whole help text that shortens it again.
preferences-pages-less = { " " }Less
# Tooltip of the button that grows a preview to fill the page.
preferences-pages-expand-preview = Expand preview
# Tooltip of the same button while the preview fills the page.
preferences-pages-close-preview = Close the expanded preview (Esc)
# A length in millimetres. $value is the number.
preferences-pages-millimetres = { $value } mm
# Pill beside a setting that applies at the next start.
preferences-pill-restart = Restart
# Pill beside a setting that is still being tried out.
preferences-pill-experimental = Experimental

# Appearance page.
preferences-appearance-heading = Appearance
# Section labels of the page, shown in capitals.
preferences-appearance-section-look = LOOK
preferences-appearance-section-canvas = CANVAS
preferences-appearance-section-navigation = NAVIGATION
preferences-appearance-section-window = WINDOW
preferences-appearance-ui-font-short-help = Applies to all panels and menus.
# Readout beside the Interface scale slider. $percent is a whole number.
preferences-appearance-ui-scale-value = { $percent }%
preferences-appearance-grid-style-label = Grid style
preferences-appearance-grid-style-help = Appearance of snap points, per editor.
# Captions of the two grid style pickers: the editor each one is for.
preferences-appearance-grid-style-schematic = Schematic
preferences-appearance-grid-style-symbol = Symbol
# The grid styles.
preferences-grid-style-dots = Dots
preferences-grid-style-lines = Lines
preferences-grid-style-small-crosses = Small crosses
# Label over the grid preview, shown in capitals.
preferences-appearance-grid-preview = GRID PREVIEW
# Caption beside that label. $schematic and $symbol are grid styles, $grid a length.
preferences-appearance-grid-preview-caption = Schematic: { $schematic } · Symbol: { $symbol }, { $grid }
# Captions drawn on the two halves of the preview. $style is a grid style, $grid a length.
preferences-appearance-grid-preview-schematic = Schematic · { $style } · { $grid }
preferences-appearance-grid-preview-symbol = Symbol · { $style } · { $grid }
preferences-appearance-symbol-grid-label = Symbol editor default grid
preferences-appearance-symbol-grid-help = Applied when a symbol library is first opened. Can be changed per library from the canvas status bar.
preferences-appearance-pin-selection-label = Pin selection
preferences-appearance-pin-selection-short-help = How a click selects a pin on the symbol editor canvas: the pin body only, or its name and number labels too.
# The two pin selection choices.
preferences-pin-selection-pin-only = Pin only
preferences-pin-selection-text-and-pin = Text and pin
# The GPU render setting; the Experimental pill stands beside it.
preferences-appearance-gpu-render-label = GPU Render
preferences-appearance-antialiasing-short-help = Smooth the edges of canvas shapes.
# The two Mouse wheel choices.
preferences-wheel-mode-zoom = Zoom
preferences-wheel-mode-scroll = Scroll
preferences-wheel-mode-short-help = Zoom: the wheel zooms the canvas. Scroll: the wheel and two-finger scroll pan, Shift + wheel pans sideways, Ctrl/Cmd + wheel or a pinch zooms.
preferences-appearance-trackpad-pan-short-help = A two-finger trackpad scroll pans instead of zooming. Pinch and Ctrl/Cmd + scroll still zoom. Only used when Mouse wheel is Zoom.
preferences-appearance-remember-geometry-short-help = Reopen the main window at the size and position it had when you last closed it.

# Editing page.
preferences-editing-heading = Editing
preferences-editing-intro = How moving, selecting, pasting and snapping behave on the schematic canvas.
# Section labels of the page, shown in capitals.
preferences-editing-section-move = MOVE AND SELECT
preferences-editing-section-grid = GRID
preferences-editing-section-text = TEXT AND PASTE
preferences-editing-section-files = FILES AND AGENTS
# Shortened help texts; More… shows the whole text.
preferences-move-keeps-wires-help-summary = Moving, rotating or mirroring symbols, child sheets or sheet pins drags the wires on their pins along, adding a bend where needed. Hold Alt through one drag to do the opposite.
preferences-drag-direction-help-summary = Left to right selects only what the box fully contains; right to left selects everything the box touches. When off, the Inside / Touching mode (Shift+S) decides.
preferences-paste-designators-help-summary = What pasted parts are called. Applies to Paste, Smart Paste, Duplicate, rubber-stamp copy and Paste Array; the units of one part always stay one part.
preferences-autosave-help-summary = Keeps a recovery copy of each schematic, symbol and footprint with unsaved changes about once a minute, in the { -brand-name } config folder. Your own file is never touched. Off by default.
preferences-agent-connections-help-summary = { -brand-name } listens for agents running as you on this computer, through a socket only you can open. Every request is listed in Messages. Off by default.
preferences-editing-fast-grid-1-help = Snap grid set by the Fast grid 1 command. Cycle fast grid steps between the two fast grids. Default 1.27 mm (50 mil).
preferences-editing-fast-grid-2-help = Snap grid set by the Fast grid 2 command. Default 2.54 mm (100 mil).
# Readout beside a fast grid field: the grid in mil. $value is the number.
preferences-editing-fast-grid-mil = { $value } mil
# The same readout while the field holds no valid grid.
preferences-editing-fast-grid-mil-invalid = —
# Error under a fast grid field. $reason says why the text would be refused.
preferences-editing-fast-grid-problem = { $reason }
# Label over the snap preview, shown in capitals.
preferences-editing-snap-preview = SNAP PREVIEW
# Caption beside that label. $first and $second are a length or the text below.
preferences-editing-snap-caption = Fast grid 1 { $first } (dots) · Fast grid 2 { $second } (rings)
# Stands for a fast grid the field would refuse.
preferences-editing-snap-not-valid = not valid
# Label on the drawing. $number is 1 or 2, $pitch a length.
preferences-editing-snap-label = Fast grid { $number } · { $pitch }
# The same label while its field holds no valid grid.
preferences-editing-snap-label-invalid = Fast grid { $number } · invalid
# Note when a fine grid is drawn with only some of its points. $count is a whole number.
preferences-editing-snap-every = 1 point in { $count } drawn
# Note at the point the pointer snaps to.
preferences-editing-snap-here = snaps here
# Note while Fast grid 2 is not valid.
preferences-editing-snap-no-rings = no rings until it is fixed
preferences-editing-snap-help = Click a pitch on the drawing to type it; Enter commits, Tab moves to the other pitch, Esc cancels. Drag a ● handle to change a pitch. The fields above follow, and the other way round.
# Tooltips of the Increment step buttons.
preferences-editing-increment-down = Step down
preferences-editing-increment-up = Step up

## Keyboard shortcuts page (#145)
# Profile picker entries. $name is the profile's name, $count its number of bindings.
preferences-keymap-profile-option-built-in = { $name } · { $count ->
    [one] { $count } binding
   *[other] { $count } bindings
}
preferences-keymap-profile-option-custom = { $name } (custom) · { $count ->
    [one] { $count } binding
   *[other] { $count } bindings
}
preferences-keymap-import = Import…
preferences-keymap-export = Export…
# Conflict banner, before the clashing keys. $count is the number of conflicts.
preferences-keymap-conflict-lead = { $count ->
    [one] { $count } conflict in active profile:
   *[other] { $count } conflicts in active profile:
}
# Conflict banner, after the clashing keys. $first and $second are the two commands.
preferences-keymap-conflict-pair = is bound to both { $first } and { $second }.
# Conflict banner, after keys macOS keeps for itself. $command is the command bound to them.
preferences-keymap-conflict-reserved = is reserved by macOS but bound to { $command }.
# Banner button: turns on the Conflicts filter.
preferences-keymap-show = Show
# Context filter chips; the context names reuse the table's.
preferences-keymap-filter-all = All
preferences-keymap-filter-conflicts = Conflicts
# Marks the column the table is sorted by, ascending.
preferences-keymap-sorted-ascending = ▲
# Between the strokes of a key sequence: P then W.
preferences-keymap-then = then
# Shortcut cell of a command with no key.
preferences-keymap-no-shortcut = —
preferences-keymap-no-rows-in-filter = No shortcuts in this context.
# Recorder popover, while keys are being captured.
preferences-keymap-recording-live = ● Recording
