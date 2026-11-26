# PDF export: watermark, layers and bookmarks

This page describes three groups of schematic PDF options in
`signex_output::PdfOptions`: the watermark, the PDF layers and the bookmarks
(issue #1298). The Print Preview / Export PDF dialog sets them; the exporter
itself chooses nothing. All three keep today's output when left at their
defaults.

PDF text is locale-invariant (docs/DECISIONS.md, 2026-11-07). The watermark
presets, the layer names and the bookmark group names are written in English
whatever the UI language.

## Defaults

| Option | Default | Effect of the default |
|---|---|---|
| `watermark` | `None` | No watermark. |
| `layers` | `None` | A flat PDF, byte-identical to the export before these options existed. |
| `bookmark_sheets` | `true` | One bookmark per exported sheet. |
| `bookmark_parts` | `true` | One bookmark per part, under its sheet. |
| `part_bookmark_order` | `BySheet` | Parts are listed under the sheet they are on. |
| `generate_nets_info` | `true` | The "one bookmark per net" option, unchanged. |
| `global_bookmarks` | `false` | The Components and Nets groups sit under each sheet. |

The domain defaults keep the watermark and the layers off, and so does the
Export PDF dialog ("Write PDF layers" is off by default, Caner 2026-11-20).

## Variant

`variant` names the variant the PDF prints. The Export PDF dialog always
names one, Base included: it opens on the project's active variant, and Base
picked in the dialog prints Base even while the project's active variant is
another one (#2430). The preview follows the same rule (one helper,
`apply_printed_variant`, serves the preview and the export) and is redrawn
when the variant changes, so it shows what the PDF prints. With Use Physical
Structure on, `${VARIANT}` and the sheet bookmarks show a named variant
("Sheet 1: Top [Lite]"). Base prints no variant name: `${VARIANT}` is empty
and the sheet bookmarks carry no tag (Caner, 2026-11-20). The helper hands
the renderer `variant: None` for Base, after it has taken the project's
active variant out of the title-block fields.

## Watermark

`Watermark` holds the text (`Draft` prints "DRAFT", `Confidential` prints
"CONFIDENTIAL", `Custom` prints the typed text), the opacity in percent, the
angle, the colour, the pages and an optional revision line. The default is
"DRAFT" at 18 %, 45°, colour `#C0392B`, on every page, with no revision line.

- **Pages.** `All` puts the watermark on every exported page. `FirstOnly`
  puts it on the first exported page only (index 0 among the exported pages,
  whatever the sheet's number).
- **Opacity.** `opacity_percent` above 100 prints as 100. The text is filled
  through an `/ExtGState` with `/ca` set to the opacity. 100 % needs no
  graphics state and writes none.
- **Colour.** The colour goes through the page's colour mode like any other
  text: Greyscale prints its luma, Black and White prints it black (still at
  the chosen opacity, so it reads as grey).
- **Font.** The bold sans face embedded for the rest of the page. A custom
  text that face cannot show falls back to the embedded face that covers
  most of it. Its glyphs are added to the font subset.
- **Order.** The watermark is drawn last on the page, over the drawing,
  inside its own `q ... Q`.
- **Revision line.** The caller builds it with
  `Watermark::revision_line_for(revision, date)`, which gives fixed English
  ("Rev B · 2026-11-18", the date alone without a revision); the exporter
  reads no clock. It is drawn only when set.

### Placement rule

`Watermark::placement(page_w_mm, page_h_mm)` is the one rule for size and
position. The export and the dialog's live preview both use it. All values
are in millimetres, measured from the page's top-left corner with y down.

1. The centre of the main text is the centre of the page.
2. The angle is the watermark's angle, counter-clockwise. A non-finite angle
   is treated as 0°.
3. The main text's cap height is the smaller of:
   - 15 % of the page's short edge, and
   - the height at which the text spans 80 % of the line through the page
     centre at that angle, between the two page edges it meets. The text
     width comes from the embedded face's advances.
4. The revision line's cap height is 16 % of the main cap height, at least
   3 mm, and never more than the main cap height.
5. The main text's cap is centred on the centre point. The revision line's
   baseline sits below the main baseline by 30 % of the main cap height plus
   the revision cap height.

On A4 landscape (297 × 210 mm) "DRAFT" is held by rule 3's first limit: a
31.5 mm cap height, centred at (148.5, 105). A long custom text is held by
the second limit and gets smaller.

## PDF layers

With `layers: Some(PdfLayerVisibility)` the export writes optional content:

- the catalog's `/OCProperties`, with `/OCGs` and a default configuration
  `/D` whose `/Order` lists the groups in `PdfLayer::ALL` order and whose
  `/OFF` lists exactly the groups in `hidden_on_open`;
- one `/OCG` object per group, named as in the table below;
- a `/Properties` resource on every page naming the groups;
- marked content `/OC /<name> BDC ... EMC` around each run of drawing in one
  group. The runs never nest.

| `PdfLayer` | Group name | What it holds |
|---|---|---|
| `WiresAndBuses` | Wires and buses | Wires, buses, bus entries, signal harnesses, junctions |
| `Parts` | Parts | Part bodies, pins and their designator and value text |
| `NetLabelsAndPorts` | Net labels and ports | Net, global and hierarchical labels (ports) |
| `PowerPorts` | Power ports | Power port symbols and power labels |
| `TitleBlockAndBorder` | Title block and border | The sheet border, the template frame and the title block |
| `NotFittedParts` | Not-fitted parts | Parts not fitted in the printed variant or marked DNP: their bodies, pins, body fills and text (#2610) |
| `Watermark` | Watermark | The watermark |

A part goes in Not-fitted parts when `variant_fit::is_fitted` says it is not
fitted in `PdfOptions::variant` (Base for `None`), read from its own sheet.
Power ports never do. The group is written whenever layers are, like the
other drawing groups, and it is visible on open unless the dialog's
"Not-fitted parts (variant)" row is unticked; switched off in the reader it
hides those parts and leaves every fitted part as it was.

Not in any group, so always visible: the page's paper fill, free drawings
(Bezier curves, dimensions and tables included), notes, no-connect and No-ERC
markers, and sheet symbols (a repeated sheet's stack and its ×N pill
included).

The Watermark group is written only when the export has a watermark. It
carries `/Usage << /Print << /PrintState /ON >> >>`, and the default
configuration has an `/AS` print event for it, so the watermark always
prints, even when the reader opened it hidden.

With `layers: None` none of this is written: no `/OCProperties`, no
`/Properties`, no marked content.

## Pages

The Pages field takes All, Current or Specific. Specific reads sheet numbers
and ranges such as `1-2, 4` (mockup row 111); text it cannot read stays in
the box, marked, and Export refuses it. The preview shows the pages the file
prints: both resolve the range by the export's rule, whose sheet numbers
`printed_sheet_numbers` names (#2787). Switching
away from Specific and back keeps the list in the box and prints it.

## From the command line

`export_schematic_pdf document=<sheet> path=<file> [pages=..] [layers=..]
[variant=..]` writes the PDF the dialog would, starting from the options the
dialog opens with (the sheet's paper, the theme's colours, the project's
active variant). `pages` is `all`, `current` or the Pages list; `layers` is
`all` or the ids of the groups visible on open (`wires`, `parts`,
`net_labels`, `power_ports`, `title_block`, `not_fitted`, `watermark`) and
writes a flat PDF when left out; `variant` is Base or a variant the project
defines. The answer names the sheet numbers printed, the variant and the
groups written. In a window the command opens the dialog, as `print` does.

## Bookmarks

- `bookmark_sheets` writes one item per exported sheet, pointing at the
  page's top-left corner.
- `bookmark_parts` writes one item per part (each symbol with a designator)
  in a Components group.
- `generate_nets_info` writes the Nets group, with net labels, ports and
  pins as chosen by `bookmark_net_labels`, `bookmark_ports` and
  `bookmark_pins`.
- With every one of `bookmark_sheets`, `bookmark_parts` and
  `generate_nets_info` off, the PDF has no `/Outlines`.

### Where the groups sit

| Setting | Components group | Nets group |
|---|---|---|
| Default (sheets on, `BySheet`, not global) | Under each sheet | Under each sheet |
| `global_bookmarks` | Top level, parts in sheet order | Top level |
| `part_bookmark_order: Alphabetical` | Top level, parts A to Z | Under each sheet unless `global_bookmarks` |
| `bookmark_sheets` off | Top level | Top level |

The rule for A to Z: `Alphabetical` always means one top-level Components
group in natural designator order (R2 before R10, the order the BOM and the
board report use, `signex_types::designator::compare_references`). With
`Alphabetical`, `global_bookmarks` decides only where the Nets group goes.
So the two options never both decide the Components group. A designator that
appears on several pages keeps its pages in page order. Each part item still
jumps to its own page and position.

With `bookmark_sheets` off there is no sheet item to nest under, so every
group sits at the top level, as in global mode.
