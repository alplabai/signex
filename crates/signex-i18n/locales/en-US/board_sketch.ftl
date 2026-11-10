# Board sketch mode of the PCB editor (#2234).

# Name of the mode: the bar over the canvas and the notice card heading.
board-sketch-title = Board Sketch
# Tooltip of the PCB Active Bar button that starts the mode.
board-sketch-tooltip = Board Sketch: draw the board outline, keepouts, cutouts and mounting holes
board-sketch-apply = Apply
board-sketch-leave = Leave
board-sketch-discard = Discard
# Messages panel line when the mode starts.
board-sketch-entered = Board sketch mode: draw with the sketch tools. Leave or Esc applies the changes to the board, Discard drops them.
# Notice and Messages line when the board does not take the sketch; the
# board is unchanged and the mode stays on.
board-sketch-refused = The sketch was not applied to the board: { $reason }
# Messages line when a board closes, or is read again from disk, while its
# sketch has changes that were never applied.
board-sketch-dropped = The board sketch changes of { $board } were not applied and are discarded: the board was closed or read again from disk.
# Messages line and notice when a save, close, quit or undock waits because
# the sketch could not be applied; the copy and the mode stay.
board-sketch-held = { $board } was not saved, closed or undocked: its sketch could not be applied to the board. Fix the sketch or Discard it, then try again.
# Messages line when the mode is asked for on a board shown in its own window.
board-sketch-undocked = Board Sketch runs in the main window: dock the board tab first.
# The Board section of the Properties panel.
board-sketch-section = Board
board-sketch-outline = Board outline
board-sketch-hole = Mounting hole
board-sketch-hole-drill = Drill (empty: circle diameter)
board-sketch-hole-plated = Plated
board-sketch-hole-net = Net
board-sketch-roles-hint = Keepouts and cutouts are set with Role above.
