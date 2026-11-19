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

# Messages lines. { $name } is a layout's name, { $path } a file.
workspace-layouts-applied = Workspace layout “{ $name }” applied
workspace-layouts-layout-saved = Workspace layout “{ $name }” saved
workspace-layouts-layout-replaced = Workspace layout “{ $name }” replaced
workspace-layouts-gone = Workspace layouts: that layout no longer exists; nothing was arranged
workspace-layouts-nothing-to-reset = Workspace layouts: the layout on screen is not an edited built-in; nothing was reset
# { $why } is the system's reason.
workspace-layouts-not-saved = Workspace layouts were not saved; the change is lost at the next launch: { $why }
workspace-layouts-exported = Workspace layouts exported to { $path }
workspace-layouts-not-exported = Workspace layouts were not exported: { $why }
# { $why } is one of the workspace-layouts-why-* reasons.
workspace-layouts-not-imported = Workspace layouts: { $path } was not imported: { $why }
workspace-layouts-imported = Workspace layouts: imported { $count } from { $path }

# A stored or imported entry that was skipped. { $key } is its stored id,
# { $entry } its place in the imported file, { $why } a reason below.
workspace-layouts-list-unreadable = Workspace layouts: the stored list is not readable; only the built-in layouts are available
workspace-layouts-skipped-unknown-builtin = Workspace layouts: skipped the edit of an unknown built-in layout “{ $key }”
workspace-layouts-skipped-builtin-edit = Workspace layouts: skipped your edit of the built-in layout “{ $key }”: { $why }
workspace-layouts-skipped-builtin-id = Workspace layouts: skipped a layout stored under the built-in id “{ $key }”
workspace-layouts-skipped-stored = Workspace layouts: skipped the stored layout “{ $key }”: { $why }
workspace-layouts-skipped-import-entry = Workspace layouts: skipped entry { $entry } of the imported file: { $why }

# Why an entry was skipped or a file refused.
workspace-layouts-why-not-object = it is not an object
workspace-layouts-why-no-name = it has no name
workspace-layouts-why-panels-unreadable = its panels are not readable
workspace-layouts-why-split-unknown = its document split is unknown
workspace-layouts-why-nothing-stored = it stores neither panels nor a split
workspace-layouts-why-not-layouts-file = it is not a Signex workspace layouts file
# { $format } is the file's export format, { $reads } the newest this build reads.
workspace-layouts-why-newer = it was written by a newer Signex (format { $format }, this one reads { $reads })
workspace-layouts-why-no-list = it has no layouts list
# { $error } is the system's or the parser's reason.
workspace-layouts-why-read-failed = reading it failed: { $error }
workspace-layouts-why-not-json = it is not JSON: { $error }
