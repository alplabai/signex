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
preferences-section-keyboard-shortcuts = Keyboard Shortcuts
# Command groups of the Keyboard Shortcuts page.
preferences-keymap-group-general = General
preferences-keymap-group-schematic = Schematic
preferences-keymap-group-pcb = PCB
preferences-keymap-group-three-d = 3D
