# The Fabrication Outputs dialog (#2388): File ▸ Fabrication Outputs, the
# PCB Active Bar button and the palette row.

fab-outputs-title = Fabrication Outputs
# Subtitle: the board the files are made from.
fab-outputs-subtitle = { $board }
# Tooltip of the PCB Active Bar button.
fab-outputs-tooltip = Fabrication Outputs: Gerber, drill, pick and place, test netlist, IPC-2581, ODB++, assembly drawings and STEP
fab-outputs-intro = Writes the files a board house and an assembler need. Nothing is written until you press Generate.
# The four groups of kinds (mockup row 115), each with how many of its kinds are ticked.
fab-outputs-group-fabrication = Fabrication
fab-outputs-group-single-file = Single-file job
fab-outputs-group-assembly = Assembly
fab-outputs-group-model = 3D
fab-outputs-group-count = { $ticked } of { $total }
# Each kind: a short title, and under some a muted description.
fab-outputs-gerber = Gerber RS-274X
fab-outputs-gerber-detail = X2 attributes: copper, legend, mask, paste, profile
fab-outputs-drill = Excellon drill
fab-outputs-drill-detail = Plated and non-plated holes, slots, drill table
fab-outputs-centroid = Pick and place (CSV)
fab-outputs-test-netlist = IPC-D-356 test netlist
fab-outputs-ipc2581 = IPC-2581 revision C
fab-outputs-ipc2581-detail = The whole job in one file
fab-outputs-odb = ODB++ job
# Under the ODB++ row; greyed until ODB++ is ticked.
fab-outputs-odb-folder = As a folder instead of one archive
fab-outputs-assembly-pdf = Assembly drawing (PDF)
fab-outputs-assembly-pdf-detail = A page for each side
fab-outputs-assembly-svg = Assembly drawings (SVG)
fab-outputs-assembly-svg-detail = A file for each side
fab-outputs-step = STEP model of the board and its parts
fab-outputs-destination = Destination
fab-outputs-name = File name
fab-outputs-folder = Output folder
fab-outputs-browse = Browse…
# Under the folder: how the files are named, from the file name.
fab-outputs-name-example = Files are named { $name }-…, for example { $name }-assembly.pdf.
# Footer, before Generate: how many kinds are ticked.
fab-outputs-selected = { $count ->
    [one] 1 kind of file selected
   *[other] { $count } kinds of file selected
}
# Footer, after Generate: the time the outcome arrived (HH:MM).
fab-outputs-generated = Generated { $time }
fab-outputs-generate-again = Generate again
# The success banner's button.
fab-outputs-open-folder = Open folder
fab-outputs-folder-not-opened = The folder { $folder } could not be opened: { $why }
fab-outputs-notes = Notes
fab-outputs-notes-count = { $count }
# Title of the folder picker.
fab-outputs-pick-folder = Choose the output folder
fab-outputs-generate = Generate
fab-outputs-cancel = Cancel
fab-outputs-close = Close
fab-outputs-writing = Writing the files…
fab-outputs-written = { $count ->
    [one] One file written to { $folder }.
   *[other] { $count } files written to { $folder }.
}
# Messages line when the command runs with no board in view.
fab-outputs-no-pcb = Fabrication Outputs: open a PCB first.
fab-outputs-board-gone = The board was closed. Nothing was written.
fab-outputs-nothing-selected = Choose at least one kind of file.
fab-outputs-bad-name = The file name is empty or holds a character a file name cannot hold.
fab-outputs-no-outline = The board has no outline. Draw one with Board Sketch first.
fab-outputs-write-failed = The files were not written: { $why }
# The whole-panel checkbox (#2410) in the info banner at the top, shown when
# the board has a panel.
fab-outputs-panel = Whole panel: { $columns } × { $rows } copies with rails, marks and breakaway edges
fab-outputs-panel-refused = The panel cannot be laid out: { $why }
fab-outputs-assembly-refused = The assembly drawing cannot be drawn: { $why }
fab-outputs-step-refused = The STEP model cannot be made: { $why }
# What the files leave out, said under the result.
fab-outputs-note-zones = { $count ->
    [one] One copper zone is not filled and is left out of the copper.
   *[other] { $count } copper zones are not filled and are left out of the copper.
}
fab-outputs-note-skipped = { $count ->
    [one] One item with a size or position out of range was left out.
   *[other] { $count } items with a size or position out of range were left out.
}
fab-outputs-note-unnamed-parts = { $count ->
    [one] One part has no designator and has no pick-and-place row.
   *[other] { $count } parts have no designator and have no pick-and-place row.
}
# What the STEP model leaves out (#2489).
fab-outputs-note-holes = { $count ->
    [one] One hole meets a hole cut before it and is left out of the STEP board body.
   *[other] { $count } holes meet a hole cut before them and are left out of the STEP board body.
}
fab-outputs-note-model-unread = The 3D model of { $source } could not be read ({ $why }); the STEP model holds its plain body.
# Characters of the assembly PDF no bundled font covers.
fab-outputs-note-chars = The assembly drawing holds characters no bundled font covers: { $chars }
# Files of the same name stem already in the output folder that this
# Generate did not write; they are left in place.
fab-outputs-stale = { $count ->
    [one] One older file in the folder is not part of this set: { $files }. Remove it before sending the folder.
   *[other] { $count } older files in the folder are not part of this set: { $files }. Remove them before sending the folder.
}
