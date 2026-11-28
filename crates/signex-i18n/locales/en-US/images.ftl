# Images placed on a sheet as project assets (#1116, mockup row 204):
# Place ▸ Graphic…, the file checks, the copy into assets/images/ on save
# and a missing file.

## Place ▸ Graphic…
images-dialog-title = Place Image
images-filter = Images (PNG, JPG, GIF, SVG)
images-not-on-sheet = Place Image works on a schematic sheet.
images-cancelled = Place Image cancelled: the tool or the sheet changed before the file was read.
# { $file } is the picked file's name, { $reason } one of the refusals below.
images-refused = { $file } was not placed: { $reason }

## Why a file is refused
images-refusal-empty-path = the image path is empty
images-refusal-absolute = the path is absolute; a sheet names images inside its project
images-refusal-leaves-project = the path leaves the project folder
# { $bytes } and { $limit } are byte counts.
images-refusal-too-large = the file is { $bytes } bytes; an image may be up to { $limit } bytes
images-refusal-unknown-format = the file is not a PNG, JPEG, GIF or SVG image
images-refusal-no-size = the size of the image could not be read
images-refusal-too-many-pixels = the image is { $width } × { $height } pixels, over the limit
# { $reference } is the outside file or address the SVG names.
images-refusal-external = the SVG refers to something outside itself ({ $reference })
# { $error } is the system's reason.
images-refusal-unreadable = the file could not be read ({ $error })
images-refusal-changed = the file changed since it was placed

## Copy on save and a missing file
# { $file } is the project-relative path the image is copied to.
images-copied = Copied { $file } into the project.
images-copy-failed = { $file } could not be copied into the project ({ $error }); it is copied again on the next save.
images-copy-exists = { $file } is already in the project with other content; it was not replaced.
# Said once per sheet; { $sheet } is the sheet's file name.
images-missing = Image file missing on { $sheet }: { $file }. The sheet keeps the image's place and size and draws a placeholder.

## The Place Image dialog (mockup row 204)
# { $sheet } is the sheet's file name.
images-dialog-on-sheet = on { $sheet }
images-dialog-project-images = Project images
images-dialog-filter = Filter images
images-dialog-loading = Reading the project's images…
images-dialog-empty = No images in the project yet. Add one with Add image file…
images-dialog-new = new · copied on Save
# { $format } is PNG, JPEG, GIF or SVG; { $kb } the size in kilobytes.
images-dialog-thumb-meta = { $format } · { $kb } KB
images-dialog-refused = not usable
images-dialog-add-file = Add image file…
images-dialog-add-hint = PNG, JPG, GIF or SVG. A file from outside the project is copied into assets/images/ when you save.
images-dialog-fact-file = File
# { $file } is the project-relative path.
images-dialog-file = { $file }
images-dialog-fact-format = Format
images-dialog-format-vector = { $format }, vector · prints sharp at any size
images-dialog-format-raster = { $format }, pixels
images-dialog-fact-size = Size
images-dialog-size = { $width } × { $height } px · { $kb } KB
images-dialog-fact-used = Used
images-dialog-copied-on-save = new, copied into the project on Save
images-dialog-project-file = in the project
images-dialog-place-at = Place at
images-dialog-width = Width (mm)
images-dialog-height-follows = height follows
images-dialog-file-size = File size at its own resolution
# { $width } and { $height } are in mm.
images-dialog-file-size-mm = { $width } × { $height } mm at 96 dpi
images-dialog-stores-path = The sheet stores the path, not the picture.
images-dialog-place = Place

## Properties ▸ Image
images-panel-title = Image
images-panel-not-found = File not found.
images-panel-section-image = Image
images-panel-section-size = Size
images-panel-section-appearance = Appearance
images-panel-file = File
images-panel-blank = { "" }
images-panel-replace = Replace…
images-panel-show-in-folder = Show in folder
images-panel-relink = Relink…
images-panel-remove = Remove image
images-panel-source-label = Source
# { $format } is PNG, JPEG, GIF or SVG; { $size } is images-panel-size-kb or -mb.
images-panel-source = { $format } · { $width } × { $height } px · { $size }
images-panel-source-missing = not found · last { $width } × { $height } px
images-panel-size-kb = { $value } KB
images-panel-size-mb = { $value } MB
images-panel-keep-aspect = Keep aspect ratio
images-panel-scale = Scale
images-panel-percent = %
images-panel-size = Size (mm)
images-panel-w = W
images-panel-h = H
images-panel-reset = Reset to file size
images-panel-opacity = Opacity
images-panel-placement = Placement
images-panel-border = Border
images-panel-border-none = None
images-panel-border-line = Line
# { $reason } says why.
images-edit-refused = The image was not changed: { $reason }

## Relink, Replace and Show in folder
images-relink-title = Relink Image
# { $count } images now show { $file }.
images-relinked = { $count ->
        [one] 1 image now shows { $file }.
       *[other] { $count } images now show { $file }.
    }
images-relink-failed = The image was not relinked: { $reason }
images-folder-failed = The folder could not be shown: { $reason }

## The Projects tree and the status bar
images-tree-new = new
images-tree-uses = { $count ->
        [one] 1 use
       *[other] { $count } uses
    }
images-status-missing = { $count ->
        [one] 1 missing asset
       *[other] { $count } missing assets
    }
# Why list_image_assets answered nothing: the scan could not read the
# project's images. $why is the system's words.
images-assets-unreadable = the assets could not be read: { $why }
