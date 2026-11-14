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

# Footer hint on the Theme page.
preferences-theme-preview-hint = Theme changes preview live

# Page names in the page list.
preferences-page-theme = Theme
preferences-page-keyboard = Keyboard shortcuts
# The page of graphical editing options (selection, dragging, paste).
preferences-page-editing = Graphical editing
preferences-page-annotation = Annotation
# The library of named electrical rule check (ERC) rule sets.
preferences-page-rule-profiles = Rule Profiles
# API keys of part distributors.
preferences-page-distributors = Distributor APIs
# The component classes a new library starts with.
preferences-page-component-classes = Default Component Classes

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
