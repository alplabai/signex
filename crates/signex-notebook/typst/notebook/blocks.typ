// The annotation blocks of the Signex notebook package, printed as design
// review records (mockup row 198): a kind label and id, a title, a status
// badge, a field grid, the body and a footer line. The kind colour is in
// the left rule and the label only, so a page of blocks reads in print.
// The fields each kind takes are listed in docs/NOTEBOOK_ANNOTATIONS.md,
// "Review records"; signex-notebook reads the same fields.

#let _kind-labels = (
  note: "Note",
  calculation: "Calculation",
  measurement: "Measurement",
  issue: "Issue",
  decision: "Decision",
)

// The print colour of each kind: the Graphite Light annotation tones, as a
// page is always black on white.
#let _kind-colours = (
  note: rgb("#5B5E66"),
  calculation: rgb("#1F45AE"),
  measurement: rgb("#24613A"),
  issue: rgb("#A8301F"),
  decision: rgb("#7A3EA8"),
)

#let _status-labels = (
  open: "Open",
  resolved: "Resolved",
  proposed: "Proposed",
  accepted: "Accepted",
  superseded: "Superseded",
  planned: "Planned",
  done: "Done",
  current: "Up to date",
  stale: "Out of date",
  pass: "Pass",
  warn: "Warn",
  fail: "Fail",
  draft: "Draft",
  reviewed: "Reviewed",
)

#let _muted = luma(40%)

// The verdict of a value against (min: .., max: ..) limits, the rule of
// signex_notebook::verdict_of: "fail" outside, "warn" inside but closer to
// a limit than 1 % of the reference value (the band's middle, else its one
// limit), else "pass".
#let verdict(value, limits) = {
  let lo = limits.at("min", default: none)
  let hi = limits.at("max", default: none)
  if (lo != none and value < lo) or (hi != none and value > hi) {
    return "fail"
  }
  let ref = if lo != none and hi != none { (lo + hi) / 2 } else if lo != none { lo } else if hi != none { hi } else { return "pass" }
  let margins = ()
  if lo != none { margins.push(value - lo) }
  if hi != none { margins.push(hi - value) }
  let scale = calc.abs(ref)
  if scale > 0 and calc.min(..margins) < scale * 0.01 { "warn" } else { "pass" }
}

// The limits of a (nominal, tolerance) pair: 2% of the nominal, or an
// absolute tolerance.
#let _band(nominal, tolerance) = {
  let half = if type(tolerance) == ratio {
    calc.abs(nominal) * (tolerance / 100%)
  } else {
    tolerance
  }
  (min: nominal - half, max: nominal + half)
}

// The worst of several verdicts.
#let _worst(verdicts) = {
  if "fail" in verdicts { "fail" } else if "warn" in verdicts { "warn" } else if verdicts.len() > 0 { "pass" } else { none }
}

// A field value as content: text as written, an array joined by commas.
#let _show(value) = {
  if value == none { none } else if type(value) == array { value.map(_show).join(", ") } else { [#value] }
}

#let _number(value) = if value == none { [] } else { [#calc.round(value, digits: 3)] }

// The status (or verdict) badge, in ink, outlined.
#let _badge(status) = box(
  stroke: 0.5pt + luma(30%),
  inset: (x: 4pt, y: 2pt),
  radius: 2pt,
  text(size: 0.75em, weight: "bold", upper(_status-labels.at(str(status), default: str(status)))),
)

// The field grid: label and value pairs, two to a row; an empty value is
// left out.
#let _fields(..pairs) = {
  let cells = ()
  for (label, value) in pairs.pos() {
    if value != none and value != () and value != "" and value != [] {
      cells.push(text(size: 0.8em, fill: _muted, label))
      cells.push(_show(value))
    }
  }
  if cells.len() > 0 {
    grid(columns: (auto, 1fr, auto, 1fr), column-gutter: 8pt, row-gutter: 4pt, ..cells)
  }
}

// The dashed footer line under a block, holding `items` joined by a dot.
#let _footer(items) = {
  let items = items.filter(item => item != none and item != [])
  if items.len() > 0 {
    v(2pt)
    line(length: 100%, stroke: (paint: luma(65%), thickness: 0.4pt, dash: "dashed"))
    text(size: 0.8em, fill: _muted, items.join[ · ])
  }
}

// The ids of the issues raised from block `id`, read from every block's
// metadata.
#let _opened-by(id) = {
  let raised = query(<signex-block>).map(found => found.value)
  let issues = raised.filter(value => value.kind == "issue" and value.from == id)
  issues.map(value => value.id)
}

// The common frame. A block never splits across a page break.
#let _block(kind, id: none, title: none, status: none, from: none, grid: none, body: none, footer: none) = {
  let colour = _kind-colours.at(kind, default: luma(35%))
  [#metadata((kind: kind, id: id, from: from)) <signex-block>]
  block(
    breakable: false,
    width: 100%,
    above: 1.2em,
    below: 1.2em,
    stroke: (left: 2pt + colour),
    inset: (left: 9pt, rest: 6pt),
    {
      text(weight: "bold", size: 0.8em, fill: colour, upper(_kind-labels.at(kind, default: kind)))
      if id != none { h(0.6em); raw(str(id)) }
      if title != none { h(0.6em); strong(title) }
      if status != none { h(1fr); _badge(status) }
      if grid != none { v(4pt); grid }
      if body != none { parbreak(); body }
      footer
    },
  )
}

// A Note: title, author and date; free text with references. No status.
#let note(id: none, title: none, author: none, date: none, target: none, ..rest) = _block(
  "note",
  id: id,
  title: title,
  grid: _fields(("Author", author), ("Date", date), ("Linked", target)),
  body: rest.pos().at(0, default: none),
)

// A Calculation: the equations in the body, then the result line against
// its target band (target: (3.30, 2%)) or limit (limit: (max: 125)).
#let calculation(
  id: none,
  title: none,
  status: none,
  date: none,
  target: none,
  limit: none,
  result: none,
  unit: none,
  requirement: none,
  inputs: none,
  ..rest,
) = {
  let numeric = type(target) == array and target.len() == 2 and type(target.at(0)) in (int, float)
  let limits = if limit != none { limit } else if numeric { _band(target.at(0), target.at(1)) } else { none }
  let outcome = if result != none and limits != none { verdict(result, limits) } else { none }
  let unit = if unit == none { "" } else { unit }
  let result-line = if result != none {
    let parts = ([Result #_number(result) #unit],)
    if numeric {
      parts.push([target #_number(target.at(0)) #unit ±#target.at(1)])
      let margin = (result - target.at(0)) / calc.abs(target.at(0)) * 100
      parts.push([margin #if margin >= 0 [+]#calc.round(margin, digits: 2) %])
    } else if limits != none and limits.at("max", default: none) != none {
      parts.push([limit #_number(limits.max) #unit])
      parts.push([margin #_number(limits.max - result) #unit])
    }
    if outcome != none { parts.push(_badge(outcome)) }
    block(above: 6pt, parts.join(h(1em)))
  }
  _block(
    "calculation",
    id: id,
    title: title,
    status: if outcome != none { outcome } else { status },
    grid: _fields(
      ("Linked", if numeric { none } else { target }),
      ("Requirement", requirement),
      ("Date", date),
    ),
    body: [#rest.pos().at(0, default: []) #result-line],
    footer: _footer((if inputs != none [Inputs: #_show(inputs)], if status == "stale" [out of date])),
  )
}

// A Measurement: Target, Tolerance, Measured, Error and Result per row,
// then instrument, conditions, operator and date. Rows are
// (what, nominal, tolerance, measured) or (what, (max: ..), measured);
// target:, value: and limit: give one row.
#let measurement(
  id: none,
  title: none,
  status: none,
  date: none,
  target: none,
  value: none,
  unit: none,
  limit: none,
  rows: (),
  conditions: none,
  instrument: none,
  operator: none,
  requirement: none,
  ..rest,
) = {
  let unit = if unit == none { "" } else { unit }
  let lines = ()
  if value != none { lines.push((target, none, none, limit, value)) }
  for row in rows {
    if row.len() == 4 {
      lines.push((row.at(0), row.at(1), row.at(2), _band(row.at(1), row.at(2)), row.at(3)))
    } else if row.len() == 3 {
      lines.push((row.at(0), none, none, row.at(1), row.at(2)))
    }
  }
  let verdicts = lines.filter(l => l.at(3) != none).map(l => verdict(l.at(4), l.at(3)))
  let cells = ()
  for (what, nominal, tolerance, limits, measured) in lines {
    let max = if limits == none { none } else { limits.at("max", default: none) }
    let min = if limits == none { none } else { limits.at("min", default: none) }
    let base = if nominal != none { nominal } else if max != none and min == none { max } else if min != none and max == none { min } else { none }
    let wanted = if nominal != none [#_number(nominal) #unit] else if max != none and min == none [\< #_number(max) #unit] else if min != none and max == none [\> #_number(min) #unit] else []
    let tol = if tolerance != none [±#tolerance] else if max != none and min == none [max] else if min != none and max == none [min] else []
    let error = if base != none and base != 0 {
      let e = (measured - base) / calc.abs(base) * 100
      [#if e >= 0 [+]#calc.round(e, digits: 2) %]
    } else []
    let outcome = if limits != none { _badge(verdict(measured, limits)) } else []
    cells += (_show(what), wanted, tol, [#_number(measured) #unit], error, outcome)
  }
  let table-body = if cells.len() > 0 {
    table(
      columns: (1fr, auto, auto, auto, auto, auto),
      stroke: (x, y) => if y == 0 { (bottom: 0.4pt + luma(60%)) } else { none },
      table.header([*Quantity*], [*Target*], [*Tol.*], [*Measured*], [*Error*], [*Result*]),
      ..cells,
    )
  }
  let worst = _worst(verdicts)
  _block(
    "measurement",
    id: id,
    title: title,
    status: if worst != none { worst } else { status },
    body: [#table-body #rest.pos().at(0, default: [])],
    grid: none,
    footer: [
      #_fields(
        ("Instrument", instrument),
        ("Conditions", conditions),
        ("Operator", operator),
        ("Date", date),
        ("Requirement", requirement),
      )
      #context _footer((
        if target != none and value == none [Target #_show(target)],
        if id != none {
          let opened = _opened-by(id)
          if opened.len() > 0 [opens #opened.map(raw).join(", ")]
        },
      ))
    ],
  )
}

// An Issue: Open / Resolved, owner, raised date and source block, linked
// parts and requirements, severity and due revision.
#let issue(
  id: none,
  title: none,
  status: none,
  date: none,
  owner: none,
  severity: none,
  due: none,
  from: none,
  target: none,
  requirement: none,
  closes: none,
  ..rest,
) = _block(
  "issue",
  id: id,
  title: title,
  status: status,
  from: from,
  grid: _fields(
    ("Owner", owner),
    ("Raised", (date, if from != none [from #raw(from)]).filter(x => x != none)),
    ("Linked", target),
    ("Requirement", requirement),
    ("Severity · due", (if severity != none { upper(severity.first()) + severity.slice(1) }, due).filter(x => x != none)),
  ),
  body: rest.pos().at(0, default: none),
  footer: _footer((if closes != none [Closes when #closes],)),
)

// A Decision: Proposed / Accepted / Superseded, owner, date and review, the
// chosen option and the alternatives with why each was dropped, plus the
// decision it supersedes and the approver.
#let decision(
  id: none,
  title: none,
  status: none,
  date: none,
  owner: none,
  review: none,
  target: none,
  requirement: none,
  chosen: none,
  alternatives: (),
  supersedes: none,
  approver: none,
  ..rest,
) = {
  let others = alternatives.map(a => if type(a) == array { (a.at(0), a.at(1, default: [])) } else { (a, []) })
  _block(
    "decision",
    id: id,
    title: title,
    status: status,
    grid: _fields(
      ("Owner", owner),
      ("Decided", (date, if review != none [review #review]).filter(x => x != none)),
      ("Linked", target),
      ("Requirement", requirement),
    ),
    body: [
      #if chosen != none and chosen != [] [Chosen: #chosen]
      #if others.len() > 0 {
        table(
          columns: (auto, 1fr),
          stroke: (x, y) => if y == 0 { (bottom: 0.4pt + luma(60%)) } else { none },
          table.header([*Alternative*], [*Why it was not chosen*]),
          ..others.flatten(),
        )
      }
      #rest.pos().at(0, default: [])
    ],
    footer: _footer((
      if supersedes != none [Supersedes #raw(supersedes)],
      if approver != none [approved by #approver],
    )),
  )
}

#let _functions = (
  note: note,
  calculation: calculation,
  measurement: measurement,
  issue: issue,
  decision: decision,
)

// The earlier one-function form, #annotation(kind: "issue", ...)[...],
// still prints.
#let annotation(kind: "note", ..args) = _functions.at(kind, default: note)(..args)

// The page block under a chapter heading (mockup row 60b): the page file,
// author and date, reviewer and status.
#let page-info(file: none, author: none, date: none, reviewer: none, status: none) = block(
  width: 100%,
  inset: 6pt,
  stroke: 0.4pt + luma(70%),
  radius: 2pt,
  _fields(
    ("Page file", if file != none { raw(file) }),
    ("Author · date", (author, date).filter(x => x != none)),
    ("Reviewer", reviewer),
    ("Status", if status != none { _status-labels.at(status, default: status) }),
  ),
)
