# Export PDF dialog (#2058, #2430): one screen with the page list, the page
# and the settings column; the Watermark, PDF Layers and Bookmarks sections
# (#1298).

## Header and footer
# Under the title: $file is the project or document file, $count how many
# sheets it has (a whole number).
pdf-dialog-subtitle = { $file } · { $count ->
    [one] { $count } sheet
   *[other] { $count } sheets
}
# The footer action; it opens a save dialog.
pdf-export-pdf = Export PDF…
# The footer caption: $count pages print (a whole number), on $paper (such
# as "ISO A4 landscape"), at $quality (such as "Medium (300 dpi)").
pdf-footer-status = { $count ->
    [one] { $count } page
   *[other] { $count } pages
} · { $paper } · { $quality }

## Page list and page
# Thumbnail caption: $number is the sheet's place in the list, $name the
# sheet's name.
pdf-thumb-caption = { $number } · { $name }
# Thumbnail caption of a printed page; $page is its sheet number.
pdf-page-n = Page { $page }
# Page toolbar: the page shown ($page) of the pages printed ($pages).
pdf-page-of = Page { $page } of { $pages }
pdf-no-files-selected = No sheet is ticked — tick one under Sheets.
pdf-no-sheets = No sheets to export — open a schematic first.
# Under the page: the $paper (such as "ISO A4 landscape"), its $width ×
# $height in millimetres, and the $scale.
pdf-page-caption = { $paper } · { $width } × { $height } mm · { $scale }
# The paper: $size is the size's name, such as "ISO A4".
pdf-paper-portrait = { $size } portrait
pdf-paper-landscape = { $size } landscape
pdf-scale-fit = Fit to Page
pdf-scale-one-to-one = 1:1
# A scale in percent; $percent is a number written as digits.
pdf-scale-percent = { $percent } %

## Settings sections
pdf-section-sheets = Sheets
pdf-section-pages = Pages
pdf-section-page-setup = Page Setup
pdf-section-output = Output
pdf-section-variant = Variant
pdf-section-structure = Structure
pdf-section-watermark = Watermark
pdf-section-layers = PDF Layers
pdf-section-bookmarks = Bookmarks
pdf-section-bookmarks-nets = Bookmarks and Nets
pdf-section-schematics-include = Schematics include
# A folded section's summary: $on of $all boxes ticked (whole numbers).
pdf-summary-of = { $on } of { $all }
# A folded section's summary: $count boxes ticked (a whole number).
pdf-summary-on = { $count } on
pdf-structure-logical = Logical
pdf-structure-physical = Physical

## Sheets and Pages
pdf-select-all = Select All
pdf-clear = Clear
# Between the Select All and Clear links.
pdf-link-separator = ·
pdf-pages-all = All
pdf-pages-current = Current
pdf-pages-specific = Specific

## Page Setup
pdf-size = Size
# The Custom size's width and height fields.
pdf-custom-size = Custom
pdf-orientation = Orientation
pdf-orientation-portrait = Portrait
pdf-orientation-landscape = Landscape
pdf-scale = Scale
# Custom page size fields: width and height, one letter each.
pdf-width-short = W
pdf-height-short = H
pdf-margins = Margins
pdf-margin-top = Top
pdf-margin-bottom = Bottom
pdf-margin-left = Left
pdf-margin-right = Right
pdf-margins-help = mm, measured from the page edge.
# Units after a field.
pdf-unit-mm = mm
pdf-unit-percent = %
pdf-unit-degree = °

## Output and Variant
pdf-schematics = Schematics
pdf-pcb = PCB
pdf-color-mode-color = Color
pdf-color-mode-greyscale = Greyscale
# Black and white, short.
pdf-color-mode-monochrome = Mono
pdf-quality = Quality
pdf-title-block = Title Block
# The raster resolution of the exported pages; dpi is dots per inch.
pdf-quality-draft = Draft (72 dpi)
pdf-quality-medium = Medium (300 dpi)
pdf-quality-high = High (600 dpi)
pdf-variant = Variant
# The variant choice that prints the design without any variant applied.
pdf-variant-base = Base
pdf-variant-help = One variant per PDF.

## Structure
pdf-use-physical = Use Physical Structure
pdf-use-physical-help = Expand logical sheets into physical sheets.
pdf-expanded-names = Expanded names:
pdf-physical-designators = Designators
pdf-physical-net-labels = Net Labels
pdf-physical-ports = Ports and Sheet Entries
pdf-physical-sheet-number = Sheet Number Parameter
pdf-physical-document-number = Document Number Parameter

## Watermark
pdf-watermark-switch = Print a watermark
# The preset switch; the two presets print the text in capitals.
pdf-watermark-draft = DRAFT
pdf-watermark-confidential = CONFIDENTIAL
pdf-watermark-custom = Custom
pdf-watermark-text = Text
pdf-watermark-opacity = Opacity
pdf-watermark-angle = Angle
pdf-watermark-colour = Colour
pdf-watermark-pages = Pages
pdf-watermark-pages-all = All
pdf-watermark-pages-first = First only
pdf-watermark-revision = Add date and revision under the text
pdf-watermark-revision-help = e.g. "Rev B · 2026-11-18"
# The line printed under the watermark: the project's $revision and the
# export $date (YYYY-MM-DD).
pdf-watermark-stamp = Rev { $revision } · { $date }

## PDF Layers
pdf-layers-switch = Write PDF layers
pdf-layers-help = Readers can switch these groups on and off. Checked = visible when the PDF opens.
pdf-layers-off = Off: the PDF is flat; every group prints as drawn.
pdf-layer-wires = Wires and buses
pdf-layer-parts = Parts
pdf-layer-net-labels = Net labels and ports
pdf-layer-power-ports = Power ports
pdf-layer-title-block = Title block and border
pdf-layer-watermark = Watermark
# Beside the Watermark layer while a watermark prints, and while none does.
pdf-layer-watermark-locked = locked on in print
pdf-layer-watermark-off = off: no watermark

## Bookmarks
pdf-bookmark-sheets = One bookmark per sheet
pdf-bookmark-parts = One bookmark per part
pdf-bookmark-parts-help = Jumps to the part and zooms to it.
pdf-bookmark-group = Group
pdf-bookmark-group-sheet = By sheet
pdf-bookmark-group-az = A–Z
pdf-bookmark-nets = One bookmark per net

## Bookmarks and Nets
pdf-zoom-help = Zoom when jumping to components or nets
pdf-zoom-far = Far
pdf-zoom-close = Close
pdf-net-bookmarks-help = The following bookmarks can be created in the PDF for nets:
pdf-bookmark-pins = Pins
pdf-bookmark-net-labels = Net Labels
pdf-bookmark-ports = Ports
pdf-include-component-parameters = Include Component Parameters
pdf-global-bookmarks = Global Bookmarks for Components and Nets

## Schematics include
pdf-no-erc-markers = No-ERC Markers
pdf-no-connect-markers = No-Connect Markers
pdf-parameter-sets = Parameter Sets
pdf-probes = Probes
pdf-blankets = Blankets
pdf-notes = Notes
pdf-collapsed-notes = Collapsed notes
