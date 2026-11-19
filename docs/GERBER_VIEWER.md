## Package

`signex_gerber::assemble` turns the files read together (a folder, a zip
or a pick of files) into a `Package`. Each image is one layer:

- **Name.** A layer's function comes from the file's own X2 file function
  (`%TF.FileFunction` or `; #@! TF.FileFunction` in a drill file). A file
  without one takes the function of the job file entry whose path names the
  same file. A file in neither is named from its file name: the layer keeps
  `file_label` ("fab-notes.gbr" is "Fab notes"), and a name that says what
  the layer is (a side and `silk`, `mask` or `paste`; `L3`; the usual
  three-letter extensions; any drill file) gives it a function too.
  Otherwise the layer has no function and is drawn as a drawing. The label is
  the user's own file name, shown as it is. The reader's "no X2 file
  function" warning stays in the problems whichever way the layer was
  named.
- **Copper by position.** Copper is numbered L1..Ln from the top, as in the
  PCB editor; n is the job file's layer number, else the highest copper
  number in the package.
- **Pairs.** Top and bottom legend, solder mask and paste pair with each
  other, and copper Lk pairs with L(n+1-k) (L1 with L4 and L2 with L3 on four
  layers). A layer whose partner is not in the package has none. Pairs are
  always mutual.
- **Default drawing order** (index 0 draws on top). The board profile, the
  drill files (plated, then non-plated) and the drawings come first, so they
  always draw above the copper. Then the stack from the top: top legend,
  paste and mask, copper L1..Ln, bottom mask, paste and legend. This is the
  stack order whether or not there is a job file: the Gerber job file does
  not have to list its files in stack order (Signex's own lists the copper
  first), so its list only breaks ties between files at the same place, then
  the order the files were given. Reset order goes back to it.
- **Nothing dropped.** A file that cannot be read at all (not text, not a
  Gerber, drill or job file, a job file that does not parse) is listed in
  `not_read` with its problem. With several job files the first one given is
  used. `Package::problems` lists every problem: the files not read in the
  order given, then each layer's by file name and line.

## Compare with board

`signex_gerber::compare_with_board` compares a package with the board it
should come from (#193d).

- **The board side is the exporter's.** The board is not drawn from its
  model here. Signex's own Gerber and drill writer
  (`signex_output::fab::generate`, Gerber and drill only) makes the board's
  files, and they are read back through the same reader as the package.
  The frame (`signex_output::fab::FabFrame`: x right, y up from the board
  origin), the pad and track shapes, the zone fills and the layer functions
  are therefore decided once, by the exporter, and a package Signex wrote for
  an unchanged board compares with no difference at all.
- **Layer matching.** Copper matches by its number L1..Ln, solder mask,
  paste and legend by side, the profile with the board shape, and a drill
  file with the holes of the same plating and span. A file whose function
  the board has no layer for (a drawing, L5 on a four-layer board) is
  unmatched: drawn, not compared, and listed once as an unmatched layer. A
  layer can be matched by hand with any board layer, or left as a drawing;
  that choice wins over its function. A board layer the board draws nothing
  on is compared as empty, so everything in the file is extra.
- **Pairing objects.** Two objects are the same when they are the same kind
  (flash, line, arc, region, hole) at the same place: each placing point
  (a line's ends in either order, an arc's centre, a region's box, a hole's
  position and slot end) and the box of their shapes within the tolerance
  on each axis. The default tolerance is 0.010 mm. Only dark objects pair;
  clear ones cut out of a region and are drawn, not compared.
- **Differences.** A board object with no partner is missing copper (on the
  board, not in the file); a package object with no partner is extra copper
  (in the file, not on the board). On mask, paste, legend and drill layers
  the same two kinds name that layer's material; the viewer filters them by
  layer. Extra copper is marked inside a keep-out when its shape meets a
  board keep-out of its layer (drawn keep-outs and the board sketch's);
  a keep-out on an inner layer is checked against every inner layer, since
  the file numbers inner layers by position. Two holes at the same place
  whose sizes differ by more than the tolerance are a drill size mismatch,
  with both sizes. A row carries the X2 net and pin of the object; a hole
  carries the pad flashed where it is drilled.
- **Not readable, never missing.** An object the reader could not build
  (an unknown aperture macro, a tool without a size) is listed as not
  readable, and the board object at its place is not listed as missing
  copper.
- **Order.** Differences are sorted by package layer, kind, then position,
  so the same inputs give the same list.
- **Overlay.** The comparison carries the board outline and cut-outs, its
  copper keep-outs and its copper shapes per copper layer, all in the file
  frame, for the viewer to draw over the package.
- **No outline.** A board with no outline makes no fabrication files, so
  every layer is unmatched and there is no other difference.
- Not decided here: the banner that says the board was saved after the
  package (it needs the files' times, so the viewer tab owns it).

## Read problems in Messages

`signex_app::app::gerber_read::report_read_problems` writes one Messages
row per read problem (#193e), in the order given (`Package::problems`), so
the rows outlive the viewer tab. Each row is a catalog sentence
(`locales/en-US/gerber_viewer.ftl`, one message per kind of problem) that
starts "Gerber Viewer:", names the file and the line, and says what was not
read and what is drawn instead: an aperture macro that is not defined draws
its flashes as markers, a tool with no size draws its holes as crosses. The
level is the problem's severity: errors as errors, the lost X2 file function
and a missing end-of-file command as warnings, a file that is not
fabrication data as information. The rows are logged under the target
`signex::gerber_viewer`, whose Messages code is `SIGNEX-GERBER_VIEWER`.
The Messages panel's Source column has no Gerber Viewer entry yet and
shows App; the sentence itself names the viewer.
