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

#let _kind-labels = (
  note: "Note",
  calculation: "Calculation",
  measurement: "Measurement",
  issue: "Issue",
  decision: "Decision",
)

// The common frame of an annotation block: a left rule, the kind label,
// the typed id, the title and the status, then the body. A block never
// splits across a page break.
#let _block(kind, id, title, status, body) = block(
  breakable: false,
  width: 100%,
  above: 1.2em,
  below: 1.2em,
  stroke: (left: 1.5pt + luma(35%)),
  inset: (left: 8pt, rest: 6pt),
  {
    text(weight: "bold", size: 0.85em, upper(_kind-labels.at(kind, default: kind)))
    if id != none { h(0.6em); raw(str(id)) }
    if title != none { h(0.6em); strong(title) }
    if status != none { h(1fr); text(size: 0.85em, upper(str(status))) }
    if body != none { parbreak(); body }
  },
)

// The function of one block kind. Named fields other than id, title and
// status (owner, target, chosen, value, ...) are accepted for the kinds
// that print them; the body is optional.
// A kind the export leaves out prints nothing.
#let _entry(kind) = (..args) => if kind in _hidden-kinds { none } else {
  let named = args.named()
  let body = args.pos().at(0, default: none)
  if body == none and "value" in named {
    body = [#str(named.at("value")) #named.at("unit", default: "")]
  }
  _block(
    kind,
    named.at("id", default: none),
    named.at("title", default: none),
    named.at("status", default: none),
    body,
  )
}

// One function per kind, each with a typed id: NOTE-, CALC-, MEAS-, ISS-,
// DEC-.
#let note = _entry("note")
#let calculation = _entry("calculation")
#let measurement = _entry("measurement")
#let issue = _entry("issue")
#let decision = _entry("decision")

// The earlier one-function form, #annotation(kind: "issue", ...)[...],
// still prints.
#let annotation(kind: "note", ..args) = _entry(kind)(..args)

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
