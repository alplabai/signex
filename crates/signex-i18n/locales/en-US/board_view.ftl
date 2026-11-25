# The board view configuration in the Layers panel (#2363): the Layers &
# Colors tab and the View Options tab.

# The two tabs of the Layers panel.
pcb-layers-tab-layers = Layers & Colors
pcb-layers-tab-view-options = View Options
# Placeholder of the field that filters the layer rows by name.
pcb-layers-search = Search layers
# A search that lists no row, and the button that empties it. $query is the
# text typed.
pcb-layers-no-match = No layers match “{ $query }”
pcb-layers-clear-search = Clear
# Tooltip of the more button beside the search field.
pcb-layers-more-tip = Manage layer sets, import, export, view from bottom, layer stack
# Rows of the more menu besides Save current as set..., Import..., Export...
# and View From Bottom Side.
pcb-layers-set-delete-current = Delete current set...
pcb-layers-layer-stack = Layer Stack...
# The Active | Set row: its caption (also the pill of the active layer's
# row), what it reads while the active layer is hidden, and the set the view
# shows. $set is the set's short name or a saved set's name.
pcb-layers-active = Active
pcb-layers-no-active = No active layer
pcb-layers-set-current = Set: { $set }
# The row whose eye shows or hides every layer.
pcb-layers-all-layers = All Layers
# Toggle that lists only the layers the board has objects on.
pcb-layers-used-on = Used On
pcb-layers-used-on-tip = List only the layers the board has objects on
# Section headers.
pcb-layers-signal-plane = Signal and Plane Layers
pcb-layers-pairs = Component Layer Pairs
pcb-layers-technical = Other Technical Layers
pcb-layers-other = Other Layers
pcb-layers-system-colors = System Colors
# The count of rows a section lists, in its header.
pcb-layers-section-count = { $count }
# Tooltips of a section header's eye.
pcb-layers-section-show = Show this section
pcb-layers-section-hide = Hide this section
# The type column of a copper row.
pcb-layers-type-signal = Signal
pcb-layers-type-plane = Plane
# The copper number in front of a copper row's name.
pcb-layers-copper-number = { $number }
# The column headers of the Component Layer Pairs table.
pcb-layers-pair-top = Top
pcb-layers-pair-layer = Layer
pcb-layers-pair-bottom = Bottom
# A row of the Component Layer Pairs table. $pair is the row's token.
pcb-layers-pair = { $pair ->
    [body_3d] 3D Body
    [assembly] Assembly
    [coating] Coating
    [component_center] Component Center
    [component_outline] Component Outline
    [courtyard] Courtyard
    [designator] Designator
    [overlay] Silkscreen
    [paste] Paste
    [solder] Solder
    [tenting] Tenting
    [value] Value
    [adhesive] Adhesive
    [fiducial] Fiducial
   *[keepout] Keep-Out
}
# Tooltip of a layer's colour dot.
pcb-layers-colour-tip = Pick this layer's color, saved with the board
# The colour card under a dot: the caption of the palette swatches, the
# caption of the hex field and the field's placeholder.
pcb-layers-palette = Palette
pcb-layers-colour = Color
pcb-layers-hex = #RRGGBB
# The colour card's button that gives the layer the board palette's colour
# again.
pcb-layers-colour-palette = Palette color
# The more menu's layer set header, and the tooltip of the Set dropdown.
pcb-layers-sets = Layer Sets
# Tooltip of the active layer dropdown.
pcb-layers-active-layer = Active Layer
# Mirrors the board view, as if seen from below.
pcb-layers-view-from-bottom = View From Bottom Side
# Import and export of the colour and visibility configuration.
pcb-layers-import = Import...
pcb-layers-export = Export...
# Tooltip of a control the board view cannot honour yet.
pcb-layers-not-yet = The board view cannot do this yet
# A row of System Colors. $row is the row's token.
pcb-layers-system = { $row ->
    [connection_lines] Connection Lines
    [selection] Selection/Highlight
    [pad_holes] Pad Holes
    [via_holes] Via Holes
    [origin_marker] Origin Marker
    [component_reference] Component Reference Point
    [body_reference] 3D Body Reference Point
    [snap_points] Custom Snap Points
    [drc_markers] DRC Error / Waived DRC Error Markers
    [violation_markers] Violation / Waived Violation Markers
    [board] Board Line/Area
    [sheet] Sheet Line/Area
    [workspace_start] Workspace Start Color
   *[workspace_end] Workspace End Color
}
# View Options: section headers.
pcb-view-general = General Settings
pcb-view-objects = Object Visibility
pcb-view-mask-dim = Mask and Dim Settings
pcb-view-additional = Additional Options
# The configuration preset dropdown, and its one entry so far.
pcb-view-preset = Configuration
pcb-view-preset-default = Default
# The 3D switch and the Single Layer Mode switch, and their states.
pcb-view-3d = 3D
pcb-view-single-layer = Single Layer Mode
# The Single Layer Mode caption in View Options; the full name is its tooltip.
pcb-view-single-layer-short = Single Layer
pcb-view-off = Off
pcb-view-on = On
pcb-view-mono = Mono
# The grid checkbox.
pcb-view-show-grid = Show Grid
# The eye that shows or hides the orientation tile over the canvas (mockup row 194).
pcb-view-orientation-tile = Orientation Tile
# A row of Object Visibility. $kind is the kind's token.
pcb-view-object = { $kind ->
    [all] All Objects
    [pads] Pads
    [vias] Vias
    [arcs] Arcs
    [tracks] Tracks
    [polygons] Polygons
    [regions] Regions
    [fills] Fills
    [texts] Texts
    [rooms] Rooms
    [dimensions] Dimensions
    [keepouts] Keepouts
    [coordinates] Coordinates
   *[body_3d] 3D Body
}
# The Object column of Object Visibility, over the names of the kinds.
pcb-view-object-column = Object
# The Draft column (outline only) and the Transparency column.
pcb-view-draft = Draft
pcb-view-transparency = Transparency
# The All Objects row while the rows differ.
pcb-view-mixed = Mixed
# A transparency or dim value. $percent is a whole number.
pcb-view-percent = { $percent } %
# The sliders of Mask and Dim Settings.
pcb-view-dimmed = Dimmed Objects
pcb-view-highlighted = Highlighted Objects
pcb-view-masked = Masked Objects
# The Additional Options header count: $on of the $total options are on.
pcb-view-options-on = { $on }/{ $total }
# A checkbox of Additional Options. $option is the option's token.
pcb-view-option = { $option ->
    [test_points] Test Points
    [status_info] Status Info
    [pad_nets] Pad Nets
    [pad_numbers] Pad Numbers
    [via_nets] Via Nets
    [via_span] Via Span
    [all_connections] All Connections in Single Layer Mode
    [net_color_override] Net Color Override
    [layer_colors_for_connections] Use Layer Colors for Connection Drawing
    [repeated_net_names] Repeated Net Names on Tracks
   *[special_strings] Special Strings
}
