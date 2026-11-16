# Panelisation (#2410): the Panel section of the board Properties panel,
# Tools ▸ Panelize Board and Remove Panel, and what they say in Messages.

board-panel-section = Panel
board-panel-none = This board is made on its own. Panelize it to array copies with rails, fiducials, tooling holes and breakaway tabs or score lines.
board-panel-add = Panelize Board
board-panel-remove = Remove Panel
board-panel-columns = Columns
board-panel-rows = Rows
board-panel-gap-x = Gap X
board-panel-gap-y = Gap Y
board-panel-rails = Rails
board-panel-rail-top = Top
board-panel-rail-bottom = Bottom
board-panel-rail-left = Left
board-panel-rail-right = Right
board-panel-marks = Marks
board-panel-fiducial = Fiducial
board-panel-tooling-hole = Tooling Hole
board-panel-separation = Separation
board-panel-mouse-bites = Mouse Bites
board-panel-v-score = V-Score
board-panel-bite-hole = Bite Hole
board-panel-bite-pitch = Bite Pitch
board-panel-tab-width = Tab Width
board-panel-tabs-per-edge = Tabs per Edge
# The panel's size and how many copies it holds, under the settings.
board-panel-size = { $width } × { $height }, { $copies ->
    [one] one copy
   *[other] { $copies } copies
}
board-panel-tabs = { $tabs ->
    [one] One tab, { $holes } bite holes
   *[other] { $tabs } tabs, { $holes } bite holes
}
board-panel-scores = { $count ->
    [one] One score line
   *[other] { $count } score lines
}
board-panel-layout-error = Cannot be laid out: { $why }
board-panel-not-rectangular = The outline is not a rectangle: tabs and score lines follow its bounding box.
# Why the section's controls are greyed.
board-panel-read-only = The board opened read-only; its panel cannot change.
# Label of the undo step.
board-panel-undo = Panel
board-panel-refused = Panel: { $why }
board-panel-no-pcb = Panelize: open a PCB first.
board-panel-nothing-to-remove = Remove Panel: the board has no panel.
board-panel-already = Panelize: the board already has a panel; change it in the Panel section of Properties.
board-panel-count-invalid = Type a whole number.
