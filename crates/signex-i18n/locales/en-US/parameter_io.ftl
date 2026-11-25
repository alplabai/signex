# The Parameter Manager's CSV export and import (#1295). Parameter names,
# values, references and file names are not localised.

# Footer buttons of the Parameter Manager.
parameter-io-export = Export…
parameter-io-import = Import…

# Titles of the save and open file dialogs; the import title also names the
# preview window.
parameter-io-export-title = Export Parameters
parameter-io-import-title = Import Parameters

# Messages lines. $sheet is the sheet's file name, $file the CSV file's path.
parameter-io-exported = Exported the parameters of { $sheet } to { $file }
# $why says what went wrong, in the system's words.
parameter-io-export-failed = Could not export the parameters to { $file }: { $why }
# $why is one of the parameter-io-why messages below, or the system's words.
parameter-io-import-failed = Could not import { $file }: { $why }

# Why an import file is refused. $megabytes is the largest size read.
parameter-io-why-too-large = the file is larger than { $megabytes } MB
parameter-io-why-not-utf8 = the file is not UTF-8 text
parameter-io-why-no-header = the file is empty
parameter-io-why-no-key = the file has neither a Unique Id nor a Reference column
# $column is the column title the file holds twice.
parameter-io-why-duplicate = two columns are titled { $column }

# Under the preview window's title. $file is the CSV file's name, $sheet the
# sheet's file name.
parameter-io-preview-subtitle = { $file } into { $sheet }
# $changes is how many parameters change, $parts on how many parts.
parameter-io-preview-summary =
    { $changes ->
        [one] { $changes } parameter changes
       *[other] { $changes } parameters change
    } on { $parts ->
        [one] { $parts } part
       *[other] { $parts } parts
    }.
# Under the summary. $added is how many parameters a part gets that it did
# not have, $changed how many it has get a new value.
parameter-io-preview-counts = { $added } added, { $changed } changed
parameter-io-preview-undo = Apply is one undo step. Nothing changes until you apply.
# $sheet is the sheet's file name.
parameter-io-preview-nothing = The file changes nothing on { $sheet }.
# $count is how many lines of the file match no part; $references names
# them (a reference, or a unique id where the reference is blank), comma
# separated.
parameter-io-preview-unmatched =
    { $count ->
        [one] { $count } line in the file matches no part on the sheet and is skipped: { $references }
       *[other] { $count } lines in the file match no part on the sheet and are skipped: { $references }
    }
# $sheet is the sheet's file name.
parameter-io-preview-sheet-gone = { $sheet } is no longer open.

# Column headings of the preview's change table.
parameter-io-column-part = Part
parameter-io-column-parameter = Parameter
parameter-io-column-before = Before
parameter-io-column-after = After
# A Before cell of a parameter the part does not have yet.
parameter-io-not-set = Not set

# The preview's confirm button. $count is how many parameters change.
parameter-io-apply =
    { $count ->
        [one] Apply { $count } change
       *[other] Apply { $count } changes
    }

# Apply was pressed while another sheet is active. $sheet is the sheet's file
# name.
parameter-io-sheet-not-active = Switch to { $sheet } to apply the import: it applies to the sheet it was read for.
