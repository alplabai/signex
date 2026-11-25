// The Signex notebook package, imported by a .snxnbk file with
//   #import "@signex/notebook:0.1.0": *
// It is plain Typst: references, the annotation blocks and the report
// layout. Signex serves it from the signex-notebook crate; the reading
// rules are in docs/NOTEBOOK_ANNOTATIONS.md.

// One quiet style for every reference: the name in mono with a thin
// underline. Pages print black on white.
#let _reference(kind, name) = box(underline(
  stroke: 0.4pt,
  offset: 2pt,
  evade: false,
  text(font: "DejaVu Sans Mono", size: 0.9em, name),
))

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
#import "blocks.typ": note, calculation, issue, decision, annotation, verdict, page-info
#import "blocks.typ": measurement as _record-measurement
// Bench readings and attachments (mockup row 199): measure.typ.
#import "measure.typ": attachment, measurement as _bench-measurement

// One #measurement for both forms: a block of bench readings (a body of
// #measurement calls) or a reading with a `quantity` prints as measure.typ
// draws it; the review record of mockup row 198 (`rows:`, or one
// `target:`/`value:`/`limit:` row) as blocks.typ draws it.
#let measurement(..args) = {
  let bench = args.pos().len() > 0 or "quantity" in args.named()
  if bench { _bench-measurement(..args) } else { _record-measurement(..args) }
}

// The report layout: #show: report.with(doc: "DN-0042", rev: "B").
// No creation date is written, so two exports give the same file.
#let report(doc: none, rev: none, title: none, body) = {
  set document(title: title, date: none)
  set page(
    paper: "a4",
    margin: (x: 22mm, y: 24mm),
    footer: context text(size: 8pt)[
      #if doc != none [#doc]
      #if rev != none [· Rev #rev]
      #h(1fr)
      Page #counter(page).display() of #counter(page).final().first()
    ],
  )
  set text(font: "Libertinus Serif", size: 10.5pt)
  set heading(numbering: "1.1")
  set math.equation(numbering: "(1)")
  body
}
