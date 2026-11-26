# Images on a schematic sheet

An image on a sheet is a reference to a file in the project, never data
inside the `.snxsch` (#1116, mockup row 204). The sheet stores the file's
project-relative path, the content hash of the file as placed, the image's
place, size and look. The picture stays a binary file in the project, so the
sheet stays small, diffs cleanly, and a Git LFS rule on `assets/` covers the
pictures.

The rules live in `signex_types::image_asset` (paths, checks, hash, names) and
`signex_types::schematic::SchImage` (the placed image); the app's slice is
`crates/signex-app/src/app/images/`.

## Placing an image

Place ▸ Graphics ▸ Graphic... (`place_graphic`) asks for a PNG, JPEG, GIF or
SVG file. The file is read and checked off the UI thread; the image then goes
on the cursor like a paste, and one click places it as one undo step. A read
that lands after Esc, a tool change or a sheet switch is dropped and said in
Messages.

From the CLI or an agent:

```
place_graphic path=<image file> at=x,y [width=<mm>] [document=..]
```

`at` is the image's centre and is refused off the grid, as for any placement.
`width` is in mm and the height follows the file's aspect ratio. The answer:

```json
{"placed": 1, "id": "…", "file": "assets/images/logo.png", "hash": "…",
 "format": "png", "width_px": 1200, "height_px": 584,
 "size": [96.52, 46.97], "copied_on_save": true}
```

An edit list (`apply_edits`) refuses `place_graphic`, as it refuses every
command that takes a file path.

### Where the file goes

- A file inside the project folder (its root, with links resolved) is placed
  where it is; nothing is copied.
- A file from anywhere else gets a name under `assets/images/` and is copied
  there when the sheet that places it is saved. Nothing is written before the
  save. The name is the picked file's own name with only letters, digits,
  `.`, `-` and `_` kept, and the extension of its kind.
- A file already in `assets/images/` with the same content (the same hash) is
  reused under its name, so one picture placed on several sheets is one file.
- A name already taken by other content gets the first 8 digits of the hash
  added (`logo-1a2b3c4d.png`). A file already at the target path with other
  content is never replaced: the copy says so in Messages and is not retried.
- A copy that fails for any other reason is said in Messages and runs again on
  the next save of the sheet.

For a sheet in no project, the sheet's own folder stands in for the project
folder.

### First size

Without `width`, an image takes the size that fills the sheet's free area,
keeping its aspect ratio (mockup row 204): the whole page when the sheet is
empty, else the larger of the strip right of the content and the strip below
it, the content reaching as far as the objects' points. Two grid steps
(5.08 mm) are kept clear on each side and the width is rounded down to the
2.54 mm grid. With less than 10.16 mm of room the image takes its own size,
read at 96 dpi.

## The checks

Images come from shared projects, so every file is checked before it is
placed or drawn:

- A sheet names only a relative path inside its project: an absolute path, a
  drive or share, a `~` path or a `..` step is refused, and a path that leaves
  the project through a link is refused.
- A file over 32 MiB is refused before it is read.
- The kind is read from the file's first bytes, not its name.
- The size is read from the header (PNG, GIF, JPEG) or from the SVG's
  `width` / `height` or `viewBox`. A raster image over 16 384 pixels on a side
  or over 64 Mpx is refused before anything decodes it.
- An SVG that refers to anything outside itself is refused: an `href` that is
  not a `#fragment` or a `data:` URI, a CSS `url(…)` or `@import` of the same,
  or an external entity.

## Drawing

The canvas reads each image's file in a `Task`, keyed by its path and hash;
`view` and `update` only look the cache up. A picture that waits to be copied
into the project is drawn from the picked bytes. An image behind the circuit
draws above the paper and under the content; one above the circuit draws over
the content (Properties ▸ Placement, as for every drawing).

A missing file, a refused one, or one whose content no longer matches the
hash keeps the image's place and size: the canvas draws a hatched placeholder
with the path, in the warning ink, and Messages names the first missing image
of each sheet once. Saving never drops the reference.

An image never turns or flips: rotating or mirroring a selection moves the
image's centre and keeps it upright, as for a table.

## File format

Images are rows of `[[extras.sheet.images]]`, one per image, in drawing
order, written only when the sheet has an image:

```toml
[[extras.sheet.images]]
uuid = "…"
width = 96.52
height = 46.97
file = "assets/images/power-path.svg"
hash = "…"            # SHA-256 of the file, hex
source_px = [1200, 584]
above = true          # only when above the circuit
locked = true         # only when locked
opacity = 60          # percent, only when not 100
border = true         # only when on
keep_aspect = false   # only when off

[extras.sheet.images.position]
x = 10.16
y = 20.32
```

The rows are a key of `[extras.sheet]`, not entries of `drawings`, so they
need no new format token: a build that predates them keeps the key as an
unknown key and writes it back. A key a newer build adds to a row is kept
too. A sheet without images writes nothing new and re-saves byte-identical.

## Export

SVG and PDF draw an image's border line when it has one. Embedding the
picture itself (raster at its own resolution, SVG as vectors) is not built
yet.
