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
