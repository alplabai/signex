# Sketch Modify tools added by mockup row 187 / mockup row 191 and the pad link of mockup row 192, in the
# footprint editor's sketch mode.

# Modify ▾ rows.
sketch-tools-chamfer = Chamfer
sketch-tools-extend = Extend
sketch-tools-linear-pattern = Linear Pattern
# Floating link on the canvas after Edit in Sketch on a pad: goes back to
# Footprint mode with that pad selected. $designator is the pad number.
sketch-tools-back-to-pad = ◂ Back to pad { $designator }
# Floating link on the canvas while a pad's sketch geometry is selected.
sketch-tools-jump-to-pad = Jump to pad ▸

# The Mirror options card (mockup row 191), shown while the Mirror tool is
# armed. Canvas clicks fill its fields; OK mirrors, Cancel leaves the tool.
sketch-tools-mirror-title = Mirror
# Field label: the geometry and patterns to mirror.
sketch-tools-mirror-objects = Objects
# Objects field value before anything is picked.
sketch-tools-mirror-objects-none = Pick objects
# Objects field value. $count is the number of picked objects.
sketch-tools-mirror-objects-count = { $count ->
    [one] 1 object
   *[other] { $count } objects
}
# Field label: the Line the objects are mirrored about.
sketch-tools-mirror-line = Mirror line
# Mirror line field value before a Line is picked.
sketch-tools-mirror-line-none = Pick a line
# Mirror line field value when the picked Line is a centerline.
sketch-tools-mirror-line-centerline = Centerline
# Mirror line field value when the picked Line is an ordinary Line.
sketch-tools-mirror-line-picked = Line
# Checkbox: on, each mirrored Point stays symmetric to its source.
sketch-tools-mirror-keep-linked = Keep linked (Symmetric constraints)
# Caption under the checkbox while Keep linked is on.
sketch-tools-mirror-caption-linked = Copies follow their source. A centerline is taken as the line first.
# Caption under the checkbox while Keep linked is off.
sketch-tools-mirror-caption-free = Copies are free. A mirrored pattern stays live.
sketch-tools-mirror-ok = OK
sketch-tools-mirror-cancel = Cancel
