# Workspace layouts (#166): View ▸ Window ▸ Layouts, Save Layout and Manage
# Layouts. Design: docs/WORKSPACE_LAYOUTS.md.

# The built-in layouts.
workspace-layouts-schematic = Schematic
workspace-layouts-layout = Layout
workspace-layouts-review = Review
workspace-layouts-notebook = Notebook
# The name Save Layout offers; { $n } is the first free number.
workspace-layouts-numbered = Layout { $n }
# The name of a duplicate; { $name } is the layout copied.
workspace-layouts-copy-of = { $name } copy
# A name made unique; { $name } is the name in use, { $n } a number from 2.
workspace-layouts-nth = { $name } ({ $n })

# Save Layout.
workspace-layouts-save-title = Save Layout
workspace-layouts-name = Name
# Under the name field when it names one of your layouts.
workspace-layouts-replaces = Replaces your layout “{ $name }”.
# Under the name field when it names a built-in; Save is off.
workspace-layouts-builtin-name = “{ $name }” is a built-in layout. Pick another name.
workspace-layouts-shortcut = Shortcut
workspace-layouts-shortcut-none = None
workspace-layouts-store = STORE
workspace-layouts-store-panels = Open panels, their docks and sizes
workspace-layouts-store-split = Split of the document area
workspace-layouts-store-windows = Extra windows and their monitors
# { $count } is how many windows are open, the main one included.
workspace-layouts-windows-count = This arrangement has { $count ->
        [one] { $count } window
       *[other] { $count } windows
    }.
workspace-layouts-documents-note = Open documents are not part of a layout. A layout only arranges panels around them.
workspace-layouts-cancel = Cancel
workspace-layouts-save = Save
workspace-layouts-replace = Replace

# Manage Layouts.
workspace-layouts-manage-title = Manage Layouts
workspace-layouts-manage-subtitle = stored in your preferences, not in the project
workspace-layouts-group-builtin = BUILT-IN
workspace-layouts-group-yours = YOUR LAYOUTS
workspace-layouts-none-yet = None yet. View ▸ Window ▸ Layouts ▸ Save Layout… adds one.
workspace-layouts-caption-builtin = built-in
workspace-layouts-caption-edited = edited · Reset available
# A user layout with no shortcut.
workspace-layouts-caption-no-shortcut = —
workspace-layouts-pill-yours = yours
workspace-layouts-pill-builtin = built-in
workspace-layouts-rename = Rename
workspace-layouts-duplicate = Duplicate
workspace-layouts-delete = Delete
workspace-layouts-rename-tooltip = Rename “{ $name }”
workspace-layouts-delete-tooltip = Delete “{ $name }”
workspace-layouts-rename-builtin = Built-in layouts cannot be renamed
workspace-layouts-delete-builtin = Built-in layouts cannot be deleted
# Under the Rename field when another layout has the name.
workspace-layouts-name-taken = Another layout is called “{ $name }”.
workspace-layouts-panels = Panels
workspace-layouts-no-panels = No panels stored
workspace-layouts-windows = Windows
workspace-layouts-windows-main = 1 · main
workspace-layouts-saved = Saved
# { $date } is the local date and time of the save.
workspace-layouts-saved-from-current = { $date } · from the current arrangement
workspace-layouts-saved-at = { $date }
workspace-layouts-saved-builtin = built-in
workspace-layouts-saved-builtin-edited = built-in · edited { $date }
workspace-layouts-update-from-current = Update from current
workspace-layouts-apply = Apply
workspace-layouts-import = Import…
workspace-layouts-export = Export…
workspace-layouts-close = Close
# The toast after Delete; Undo puts the layout back.
workspace-layouts-deleted = Deleted layout “{ $name }”
workspace-layouts-undo = Undo

# File dialogs of Import… and Export….
workspace-layouts-import-title = Import Workspace Layouts
workspace-layouts-export-title = Export Workspace Layouts
workspace-layouts-file-filter = Signex workspace layouts
