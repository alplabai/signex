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
