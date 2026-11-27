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

Place ▸ Graphics ▸ Graphic... (`place_graphic`) opens the Place Image dialog
(mockup row 204) on the project's `assets/images/` folder, read and checked
off the UI thread:

- at the left, a thumbnail of each image with its format and size, a Filter
  field, and Add image file… for a PNG, JPEG, GIF or SVG from anywhere; a file
  from outside the project is marked "new · copied on Save";
- at the right, a large preview of the chosen image and its facts (file,
  format, pixel size and bytes, whether it is in the project), and the size it
  is placed at: a Width field in mm (the height follows), filled with the
  first size for the sheet's free area, or the file's own size at 96 dpi.

Place puts the image on the cursor like a paste, and one click places it as
one undo step; Cancel or Esc closes the dialog. A width that does not read
keeps Place off. A read that lands after Esc, a tool change or a sheet switch
is dropped and said in Messages.

A row of the Projects tree's `assets/images/` puts that file on the cursor
the same way, so a press on the row and a click on the sheet place it.

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

## Editing an image

Properties ▸ Image (mockup row 204) shows, for the selected image:

- **Image:** the file (mono, ellipsised at the start), Replace… and Show in
  folder, and the source facts (format · pixels · bytes, from the last asset
  scan). A missing file shows "not found · last W × H px", swaps Show in
  folder for Relink…, and adds a banner with Relink… and Remove image.
- **Size:** Keep aspect ratio, Scale (percent of the file's own size at
  96 dpi), W and H in mm, Reset to file size (Scale 100 %).
- **Appearance:** Opacity (percent), Placement (Behind / Above circuit) and
  Border (None / Line).

A typed box applies on Enter. Each row is one edit and one undo step, the
same edit `set_image` makes:

```
set_image id=<image> field=<field> value=<value> [document=..]
```

| field | value |
|---|---|
| `width`, `height` | mm, 0.1 to 1000; while the aspect is kept the other side follows the box |
| `scale` | percent of the file's own size; `file` is 100 |
| `keep_aspect`, `border` | `true` or `false` |
| `opacity` | 0 to 100 |
| `placement` | `behind` or `above` |
| `bounds` | `x,y,width,height` in mm, the box's top-left corner and size |

It answers with the image: `{"id", "file", "hash", "position", "size",
"scale", "source_px", "keep_aspect", "opacity", "border", "placement",
"locked"}`. A locked image refuses every edit with the lock's reason.

### Resize handles

A selected, unlocked image has eight handles: four corners and four sides. A
drag moves the handle's edges on the snap grid while the opposite edge or
corner stays; while Keep aspect ratio is on a corner keeps the aspect (the
larger change wins) and a side handle does nothing, as drawn. A pill by the
box says the size in the person's unit and the scale. The drop is one
`bounds` edit; Esc or a right press drops the drag.

### Relink and Replace

Relink… (on a missing file) picks a new file and points every image that
names the old file, on every sheet of the project, at it, as one undo step:
a closed sheet is changed in a tabless engine and left unsaved. Replace…
points the selected image alone at the new file. The box keeps its place and
width; while the aspect is kept the height follows the new file's. A new
file from outside the project is copied into `assets/images/` on save, as a
placed one is.

```
relink_image path=<image file> file=<project-relative file> [document=..]
relink_image path=<image file> id=<image> [document=..]
```

It answers `{"relinked", "sheets", "file", "hash", "format", "width_px",
"height_px", "copied_on_save"}`. An edit list refuses it (it takes a file
path).

### Show in folder

Show in folder opens the file manager on the image's file, off the UI thread.
`show_image_in_folder id=<image> [document=..]` answers `{"id", "path",
"folder", "shown"}`; a headless run launches nothing (`shown` is false).

## Missing files

The project's assets are scanned off the UI thread: every file of
`assets/images/` and every file an image of the project's sheets names (closed
sheets read from disk), each checked and hashed. A use is missing when its
file is gone, refused or holds other content than its hash names; a pick
waiting for its save is not missing. The scan runs when the images of the
open sheets, the picks waiting or the files on disk (a copy landed, a relink)
change; a project with no image is scanned when Place ▸ Graphic… opens or
`list_image_assets` asks.

- **Projects tree:** an `assets/` group with `images/` lists each file, its
  use count, a missing pill or "new" for a pick waiting.
- **Status bar:** "N missing assets" in the warning ink on a sheet whose
  project misses image files.
- **ERC:** `image_file_missing` (Warning, group Parts) is one finding per
  image whose file is missing, at its centre; Run ERC and `check_erc` read the
  files on their run's thread.
- `list_image_assets document=<sheet>` answers `{"root", "folder", "assets":
  [{"file", "format", "bytes", "width_px", "height_px", "uses", "missing",
  "pending", "sheets"}], "missing"}` once the scan lands.

## Export

A PDF export reads the exported sheets' image files on its own thread (a pick
waiting for its save is taken from memory) and draws each image in its box,
under its border line:

- a JPEG is embedded as it is (`DCTDecode`); a PNG, and a GIF's first frame,
  as deflated RGB at the file's own resolution with a soft mask when it has
  transparency; one picture used several times is one object;
- an SVG is drawn as vector paths, so it prints sharp at any size. Solid
  fills and strokes print in their colour (a gradient in its first stop),
  a fill's opacity is laid over white, and text inside the picture is not
  drawn;
- the SVG document of a page embeds a raster picture as a data URI and draws
  an SVG one as paths.

A missing, refused or changed file prints as an empty box with its border
line when it has one. The PDF colour modes leave an embedded picture in
colour.
