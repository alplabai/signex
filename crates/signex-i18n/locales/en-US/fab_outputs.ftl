# The Fabrication Outputs dialog (#2388): File ▸ Fabrication Outputs, the
# PCB Active Bar button and the palette row.

fab-outputs-title = Fabrication Outputs
# Subtitle: the board the files are made from.
fab-outputs-subtitle = { $board }
# Tooltip of the PCB Active Bar button.
fab-outputs-tooltip = Fabrication Outputs: Gerber, drill, pick and place and test netlist files
fab-outputs-intro = Writes the files a board house and an assembler need. Nothing is written until you press Generate.
fab-outputs-files = Files
fab-outputs-gerber = Gerber RS-274X with X2 attributes: copper, legend, solder mask, paste and board profile
fab-outputs-drill = Excellon drill: plated and non-plated holes, slots and a drill table
fab-outputs-centroid = Pick and place (CSV)
fab-outputs-test-netlist = IPC-D-356 test netlist
fab-outputs-name = File name
fab-outputs-folder = Output folder
fab-outputs-browse = Browse…
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
# What the files leave out, said under the result.
fab-outputs-note-zones = { $count ->
    [one] One copper zone is not filled and is left out of the copper.
   *[other] { $count } copper zones are not filled and are left out of the copper.
}
fab-outputs-note-texts = { $count ->
    [one] One text is not drawn yet.
   *[other] { $count } texts are not drawn yet.
}
fab-outputs-note-v-scores = { $count ->
    [one] One V-score line is not written.
   *[other] { $count } V-score lines are not written.
}
fab-outputs-note-skipped = { $count ->
    [one] One item with a size or position out of range was left out.
   *[other] { $count } items with a size or position out of range were left out.
}
fab-outputs-note-unnamed-parts = { $count ->
    [one] One part has no designator and has no pick-and-place row.
   *[other] { $count } parts have no designator and have no pick-and-place row.
}
# Files of the same name stem already in the output folder that this
# Generate did not write; they are left in place.
fab-outputs-stale = { $count ->
    [one] One older file in the folder is not part of this set: { $files }. Remove it before sending the folder.
   *[other] { $count } older files in the folder are not part of this set: { $files }. Remove them before sending the folder.
}
