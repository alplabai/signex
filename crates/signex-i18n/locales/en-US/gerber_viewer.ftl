## The Gerber Viewer (#2539): what a reader could not read, as the Messages
## panel shows it. One message per kind of problem: the file, the line,
## what was not read and what is drawn instead. Kept short, since the panel
## shortens a long row. $file is the file name as given, $line the 1-based
## line in it.

gerber-viewer-read-unknown-aperture-macro = Gerber Viewer: { $file }, line { $line }: aperture macro { $name } is not defined; { $flashes ->
    [one] its flash is drawn as a marker.
   *[other] its { $flashes } flashes are drawn as markers.
}
gerber-viewer-read-bad-macro-parameters = Gerber Viewer: { $file }, line { $line }: aperture macro { $name } builds no shape; its flashes are drawn as markers.
gerber-viewer-read-undefined-aperture = Gerber Viewer: { $file }, line { $line }: aperture D{ $d_code } is not defined; { $uses ->
    [one] the object using it is drawn as a marker.
   *[other] the { $uses } objects using it are drawn as markers.
}
gerber-viewer-read-bad-aperture-definition = Gerber Viewer: { $file }, line { $line }: aperture D{ $d_code } cannot be read; its objects are drawn as markers.
gerber-viewer-read-missing-format = Gerber Viewer: { $file }, line { $line }: no coordinate format is given; positions may be wrong.
gerber-viewer-read-missing-units = Gerber Viewer: { $file }, line { $line }: no units are given; sizes and positions may be wrong.
gerber-viewer-read-bad-coordinate = Gerber Viewer: { $file }, line { $line }: a coordinate cannot be read; the rest of the file is drawn.
gerber-viewer-read-unsupported-command = Gerber Viewer: { $file }, line { $line }: { $command } is not supported and was not read; the rest is drawn.
gerber-viewer-read-unclosed-region = Gerber Viewer: { $file }, line { $line }: a region is not closed; the rest of the file is drawn.
gerber-viewer-read-missing-end-of-file = Gerber Viewer: { $file }, line { $line }: no end-of-file command; the file may be cut short.
gerber-viewer-read-tool-without-size = Gerber Viewer: { $file }, line { $line }: tool T{ $tool } has no size; { $holes ->
    [one] its hole is drawn as a cross.
   *[other] its { $holes } holes are drawn as crosses.
}
gerber-viewer-read-undefined-tool = Gerber Viewer: { $file }, line { $line }: tool T{ $tool } is not defined; { $holes ->
    [one] its hole is drawn as a cross.
   *[other] its { $holes } holes are drawn as crosses.
}
gerber-viewer-read-bad-excellon-header = Gerber Viewer: { $file }, line { $line }: the drill header line { $text } cannot be read.
gerber-viewer-read-no-file-function = Gerber Viewer: { $file }, line { $line }: no X2 file function; the layer is named from the file name.
gerber-viewer-read-not-text = Gerber Viewer: { $file }, line { $line }: the file is not text and is not drawn.
gerber-viewer-read-not-a-fabrication-file = Gerber Viewer: { $file }, line { $line }: not a Gerber, drill or job file; it is not drawn.
gerber-viewer-read-bad-job-file = Gerber Viewer: { $file }, line { $line }: the job file cannot be read ({ $why }); layers are named without it.

# gerber-viewer-base
## The Gerber Viewer tab (mockup rows 193 and 193f): the empty state, the
## toolbar, the layer list and the read-only Properties.
gerber-viewer-tab-title = Gerber Viewer
gerber-viewer-tab-kind = GBR
gerber-viewer-empty-title = No files open
gerber-viewer-empty-body = Drop files, a folder or a .zip anywhere on the tab, or open them here.
gerber-viewer-open-folder = Open folder…
gerber-viewer-open-zip = Open zip package…
gerber-viewer-open-files = Open files…
gerber-viewer-open-zip-tip = The zip is read in place and never unpacked to disk.
gerber-viewer-accepted = Reads Gerber X2 and RS-274X, Excellon drill and the Gerber job file (.gbrjob). Other files are listed as Not read in Messages.
gerber-viewer-recent = Recent packages
gerber-viewer-recent-more = { $path } and { $count } more
gerber-viewer-recent-layers = { $count ->
    [one] 1 layer
   *[other] { $count } layers
}
gerber-viewer-recent-missing = Files not found
gerber-viewer-drop-files = { $count ->
    [one] Drop to open 1 file
   *[other] Drop to open { $count } files
}
gerber-viewer-reading = Reading…
gerber-viewer-read-failed = Gerber Viewer: the package could not be read ({ $why }).
gerber-viewer-open = Open
gerber-viewer-open-tip = Open a folder, a zip package or files ({ $key })
gerber-viewer-flip = Flip
gerber-viewer-flip-tip = Show the board from the bottom
gerber-viewer-drills = Drills
gerber-viewer-drills-tip = Draw the holes over everything
gerber-viewer-highlight-net = Highlight net
gerber-viewer-highlight-net-tip = Dim everything that is not on the selected object's net
gerber-viewer-highlight-net-no-x2 = The files carry no X2 net attributes
gerber-viewer-highlight-net-no-net = Select an object on a net first
gerber-viewer-needs-files = Open files first
gerber-viewer-unit-mm = mm
gerber-viewer-unit-mil = mil
gerber-viewer-layers = Layers
gerber-viewer-badge-x2 = X2
gerber-viewer-badge-job = job
gerber-viewer-badge-name = name
gerber-viewer-badge-x2-tip = Named from the file's X2 file function: { $function }
gerber-viewer-badge-job-tip = Named from the job file
gerber-viewer-badge-name-tip = The file has no X2 file function, so the name was worked out from the file name
gerber-viewer-not-read = Not read
gerber-viewer-show-tip = Show or hide the layer
gerber-viewer-colour-tip = Change the color
gerber-viewer-grip = ⠿
gerber-viewer-grip-tip = Drag to change the drawing order; the top of the list draws on top
gerber-viewer-layer-copper = L{ $number }
gerber-viewer-layer-copper-top = Top ({ $layer })
gerber-viewer-layer-copper-bottom = Bottom ({ $layer })
gerber-viewer-layer-silk-top = Top Silkscreen
gerber-viewer-layer-silk-bottom = Bottom Silkscreen
gerber-viewer-layer-mask-top = Top Solder Mask
gerber-viewer-layer-mask-bottom = Bottom Solder Mask
gerber-viewer-layer-paste-top = Top Paste
gerber-viewer-layer-paste-bottom = Bottom Paste
gerber-viewer-layer-outline = Board outline
gerber-viewer-layer-drill-plated = Drill, plated
gerber-viewer-layer-drill-non-plated = Drill, non-plated
gerber-viewer-layer-drill-span = Drill, { $from } to { $to }
gerber-viewer-properties = Properties
gerber-viewer-prop-type = Type
gerber-viewer-type-flash = Flash
gerber-viewer-type-line = Track
gerber-viewer-type-arc = Arc
gerber-viewer-type-region = Region
gerber-viewer-type-hole = Hole
gerber-viewer-type-slot = Slot
gerber-viewer-prop-layer = Layer
gerber-viewer-prop-file = File
gerber-viewer-prop-file-line = { $file }, line { $line }
gerber-viewer-prop-d-code = D-code
gerber-viewer-prop-aperture = Aperture
gerber-viewer-prop-aperture-function = Aperture function
gerber-viewer-prop-net = Net
gerber-viewer-prop-pin = Pin
gerber-viewer-prop-pin-value = { $component }-{ $pin }
gerber-viewer-prop-component = Component
gerber-viewer-prop-position = Position
gerber-viewer-prop-xy = X { $x } · Y { $y } { $unit }
gerber-viewer-prop-size = Size
gerber-viewer-prop-length = { $value } { $unit }
gerber-viewer-prop-polarity = Polarity
gerber-viewer-polarity-dark = Dark
gerber-viewer-polarity-clear = Clear (cut-out)
gerber-viewer-prop-none = —
gerber-viewer-prop-function = File function
gerber-viewer-prop-units = Units
gerber-viewer-units-mm = Millimetres
gerber-viewer-units-inch = Inches
gerber-viewer-prop-objects = Objects
gerber-viewer-prop-creator = Created by
gerber-viewer-prop-nothing = Click an object on the board, or a layer in the list.
gerber-viewer-footer = { $layers } · { $problems }
gerber-viewer-footer-layers = { $count ->
    [one] 1 layer
   *[other] { $count } layers
}
gerber-viewer-footer-problems = { $count ->
    [one] 1 read problem in Messages
   *[other] { $count } read problems in Messages
}
gerber-viewer-footer-note = Read only: the files are never changed.
gerber-viewer-read-unopenable = Gerber Viewer: { $file }, line { $line }: the file cannot be opened ({ $why }); it is listed as Not read.
gerber-viewer-read-too-large = Gerber Viewer: { $file }, line { $line }: the file is larger than { $limit } MiB; it is listed as Not read.
