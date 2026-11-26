// The Signex notebook package, imported by a .snxnbk file with
//   #import "@signex/notebook:0.1.0": *
// It is plain Typst: references, the annotation blocks and the report
// layout. Signex serves it from the signex-notebook crate; the reading
// rules are in docs/NOTEBOOK_ANNOTATIONS.md.

// The settings the PDF export passes as sys.inputs, each "signex-<name>"
// (docs/NOTEBOOK_PDF.md). The preview passes none, so every page reads as
// it does on screen.
#let _input(name, default) = sys.inputs.at("signex-" + name, default: default)
#let _on(name, default) = _input(name, if default { "true" } else { "false" }) == "true"
// The annotation kinds the export leaves out, and the names of the stale
// references, one per line.
#let _hidden-kinds = _input("hidden-kinds", "").split(",").filter(k => k != "")
#let _stale-names = _input("stale", "").split("\n").filter(n => n != "")

// A stale reference's mark, in the left margin level with the line.
#let _stale-mark = context {
  let x = here().position().x
  box(place(dx: 10mm - x, text(font: "DejaVu Sans Mono", size: 8pt, "\u{26A0}")))
}

// One quiet style for every reference: the name in mono with a thin
// underline. Pages print black on white. A stale one prints the same, as
// plain text, with a mark in the margin.
#let _reference(kind, name) = {
  box(underline(
    stroke: 0.4pt,
    offset: 2pt,
    evade: false,
    text(font: "DejaVu Sans Mono", size: 0.9em, name),
  ))
  if name in _stale-names { _stale-mark }
}

// A part, by designator: #component("R7").
#let component(name) = _reference("component", name)

// A net, by name: #net("+3V3").
#let net(name) = _reference("net", name)

// A pin of a part: #pin("U3", "3"), or one dotted name, #pin("U3.3").
#let pin(part, ..number) = {
  let rest = number.pos()
  let name = if rest.len() == 0 { part } else { part + "." + rest.first() }
  _reference("pin", name)
}

// One function per kind, each with a typed id (NOTE-, CALC-, MEAS-, ISS-,
// DEC-), printed as a review record; the earlier #annotation(kind: ...)
// form; the verdict rule; the page block. See blocks.typ.
#import "blocks.typ": verdict, page-info
#import "blocks.typ": note as _note, calculation as _calculation, issue as _issue
#import "blocks.typ": decision as _decision, annotation as _annotation
#import "blocks.typ": measurement as _record-measurement
// Bench readings and attachments (mockup row 199): measure.typ.
#import "measure.typ": attachment, measurement as _bench-measurement

// A kind the export leaves out prints nothing.
#let _shown(kind, printed) = (..args) => if kind in _hidden-kinds { none } else { printed(..args) }
#let note = _shown("note", _note)
#let calculation = _shown("calculation", _calculation)
#let issue = _shown("issue", _issue)
#let decision = _shown("decision", _decision)
#let annotation(kind: "note", ..args) = if kind in _hidden-kinds { none } else {
  _annotation(kind: kind, ..args)
}

// One #measurement for both forms: a block of bench readings (a body of
// #measurement calls) or a reading with a `quantity` prints as measure.typ
// draws it; the review record of mockup row 198 (`rows:`, or one
// `target:`/`value:`/`limit:` row) as blocks.typ draws it.
#let measurement(..args) = if "measurement" in _hidden-kinds { none } else {
  let bench = args.pos().len() > 0 or "quantity" in args.named()
  if bench { _bench-measurement(..args) } else { _record-measurement(..args) }
}

// The Python cell results of this compile, written by Signex (the cell
// module of the signex-notebook crate). A compile never runs a cell.
#let _cells = json("cells.json")

// A Python cell: #python(label: "ripple-vs-vin")[```python ... ```].
// The page shows the cell's state, the values it read with their
// schematic source, what it printed and its figures; the source is hidden
// in print unless the export asks for it.
#let python(label: none, ..args) = {
  let body = args.pos().at(0, default: none)
  let cell = if label == none { none } else { _cells.cells.at(label, default: none) }
  let ink = if cell != none and cell.greyed { luma(55%) } else { black }
  block(
    breakable: true,
    width: 100%,
    above: 1.2em,
    below: 1.2em,
    stroke: (left: 1.5pt + luma(35%)),
    inset: (left: 8pt, rest: 6pt),
    {
      set text(fill: ink)
      text(weight: "bold", size: 0.85em, "PYTHON")
      if label != none { h(0.6em); raw(label) }
      if cell != none { h(1fr); text(size: 0.85em, cell.caption) }
      // The PDF export's "print Python source" option (docs/NOTEBOOK_PDF.md).
      let source = _cells.print_source or _on("python-source", false)
      if source and body != none { parbreak(); body }
      if label == none {
        parbreak()
        text(size: 0.9em, [Not run: give the cell a label to run it.])
      } else if cell == none {
        parbreak()
        text(size: 0.9em, [Not run yet: Run the cell, or Run all, to compute it.])
      } else {
        if cell.values.len() > 0 {
          parbreak()
          grid(
            columns: (auto, auto, auto),
            column-gutter: 1em,
            row-gutter: 0.4em,
            ..cell.values.map(v => (raw(v.symbol), v.value, [from #v.source])).flatten(),
          )
        }
        if cell.stdout != "" { parbreak(); raw(cell.stdout, block: true) }
        for f in cell.figures {
          figure(
            image(bytes(f.svg), format: "svg", width: 100%),
            caption: if f.caption == "" { none } else { f.caption },
          )
        }
        if cell.note != "" { parbreak(); text(size: 0.9em, cell.note) }
        if cell.reads != "" { parbreak(); text(size: 0.8em, cell.reads) }
      }
    },
  )
}

// A bookmark for a captioned figure or table, under the section it is in.
// It is a heading that takes no room, is not numbered and stays out of the
// contents.
#let _figure-bookmark(it) = {
  it
  if it.caption != none {
    place(hide(heading(
      level: 3,
      numbering: none,
      outlined: false,
      bookmarked: true,
      it.caption.body,
    )))
  }
}

// The title block an export prints at the top of the first page.
#let _title-block(doc, rev, title, date) = block(
  width: 100%,
  below: 1.6em,
  inset: (bottom: 6pt),
  stroke: (bottom: 0.6pt),
  {
    if title != none { text(size: 16pt, weight: "bold", title); linebreak() }
    text(size: 9pt, {
      if doc != none [#doc]
      if rev != none [ · Rev #rev]
      if date != none [ · #date]
    })
  },
)

// The report layout: #show: report.with(doc: "DN-0042", rev: "B").
// No creation date is written, so two exports give the same file. The
// export may set the paper, the title block, the contents and the
// bookmarks (_input above); `date` is the revision date.
#let report(doc: none, rev: none, title: none, date: none, paper: "a4", body) = {
  let bookmarks = _on("bookmarks", true)
  set document(title: title, date: none)
  set page(
    paper: _input("paper", paper),
    margin: (x: 22mm, y: 24mm),
    footer: context text(size: 8pt)[
      #if doc != none [#doc]
      #if rev != none [· Rev #rev]
      #h(1fr)
      Page #counter(page).display() of #counter(page).final().first()
    ],
  )
  set text(font: "Libertinus Serif", size: 10.5pt)
  set heading(numbering: "1.1", bookmarked: if bookmarks { auto } else { false })
  set math.equation(numbering: "(1)")
  show figure: it => if bookmarks and _on("export", false) { _figure-bookmark(it) } else { it }
  if _on("title-block", false) { _title-block(doc, rev, title, date) }
  if _on("contents", false) { outline() }
  body
}
