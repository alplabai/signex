// Bench readings and attachments of the Signex notebook package
// (mockup row 199). #measurement prints a verdict table; #attachment a
// numbered figure with its source line and, once a part it was taken
// with has changed, a stale mark. The rules are in
// docs/NOTEBOOK_MEASUREMENTS.md; the verdict is the one
// signex_notebook::measurement::check gives.

// Print colours: the page is always white.
#let _ink = (
  pass: rgb("#2b7a3d"),
  warn: rgb("#a86a00"),
  fail: rgb("#b3261e"),
  muted: luma(40%),
  rule: luma(35%),
  stale: rgb("#a86a00"),
  cursor: rgb("#6b4fa0"),
)
#let _verdicts = (pass: "Pass", warn: "Warn", fail: "Fail")
#let _thin = "\u{2009}"
// Inside the limit, closer than this share of the reference value to it.
#let _warn-margin = 0.01

// A number as printed, with a minus sign for a negative one.
#let _num(x) = str(x).replace("-", "−")

// A number and its unit, a thin space between.
#let _qty(number, unit) = if unit == none or unit == "" { number } else {
  number + _thin + unit
}

// The bounds a limit sets and the value the error is taken against.
#let _bounds(a) = {
  let nominal = a.at("nominal", default: none)
  let tolerance = a.at("tolerance", default: none)
  let limit = a.at("limit", default: none)
  if nominal != none and tolerance != none {
    let band = if type(tolerance) == ratio { nominal * (tolerance / 100%) } else { tolerance }
    (low: nominal - calc.abs(band), high: nominal + calc.abs(band), ref: nominal)
  } else if type(limit) == dictionary {
    let low = limit.at("min", default: none)
    let high = limit.at("max", default: none)
    if low != none and high != none {
      (low: low, high: high, ref: (low + high) / 2)
    } else if high != none {
      (low: none, high: high, ref: high)
    } else if low != none {
      (low: low, high: none, ref: low)
    } else { none }
  } else { none }
}

// The error (a share) and the result of a reading; none without a value
// or a limit.
#let _check(value, b) = {
  if value == none or b == none or type(value) not in (int, float) { return none }
  let scale = calc.abs(b.ref)
  let error = if scale > 0 { (value - b.ref) / scale } else { none }
  let fails = (b.low != none and value < b.low) or (b.high != none and value > b.high)
  let margins = ()
  if b.low != none { margins.push(value - b.low) }
  if b.high != none { margins.push(b.high - value) }
  let warns = not fails and scale > 0 and calc.min(..margins) < _warn-margin * scale
  (error: error, result: if fails { "fail" } else if warns { "warn" } else { "pass" })
}

// `x` with `digits` decimals, trailing zeros kept.
#let _fixed(x, digits) = {
  let s = str(calc.round(x, digits: digits))
  if digits == 0 { return s }
  let parts = s.split(".")
  let frac = parts.at(1, default: "")
  parts.at(0) + "." + frac + "0" * calc.max(0, digits - frac.len())
}

// The Error column: two significant digits, a sign, a thin space and %.
#let _error-text(e) = {
  let p = e * 100
  let m = calc.abs(p)
  if m == 0 { return "0" + _thin + "%" }
  let digits = calc.max(0, 2 - (calc.floor(calc.log(m, base: 10)) + 1))
  let sign = if p < 0 { "−" } else { "+" }
  sign + _fixed(m, digits) + _thin + "%"
}

#let _target-text(a, unit) = {
  let nominal = a.at("nominal", default: none)
  let limit = a.at("limit", default: none)
  if nominal != none { return _qty(_num(nominal), unit) }
  if type(limit) != dictionary { return "" }
  let low = limit.at("min", default: none)
  let high = limit.at("max", default: none)
  if low != none and high != none { return _num(low) + "–" + _qty(_num(high), unit) }
  if high != none { return "< " + _qty(_num(high), unit) }
  if low != none { return "≥ " + _qty(_num(low), unit) }
  ""
}

#let _tolerance-text(a, unit) = {
  let tolerance = a.at("tolerance", default: none)
  let limit = a.at("limit", default: none)
  if a.at("nominal", default: none) != none and tolerance != none {
    if type(tolerance) == ratio {
      return "±" + _qty(_num(tolerance / 1%), "%")
    }
    return "±" + _qty(_num(tolerance), unit)
  }
  if type(limit) != dictionary { return "" }
  let low = limit.at("min", default: none)
  let high = limit.at("max", default: none)
  if low != none and high != none { return "range" }
  if high != none { return "max" }
  if low != none { return "min" }
  ""
}

// One target, or several joined with commas.
#let _targets(t) = if t == none { none } else if type(t) == array { t.join([, ]) } else { t }

#let _columns = (1.6fr, 1fr, 0.7fr, 1fr, 0.8fr, 0.7fr)

#let _table-row(cells) = grid(
  columns: _columns,
  column-gutter: 6pt,
  inset: (y: 2.5pt),
  ..cells.map(cell => [#cell]),
)

#let _header-row = block(
  stroke: (bottom: 0.4pt + _ink.rule),
  below: 2pt,
  _table-row(
    ("Quantity", "Target", "Tol.", "Measured", "Error", "Result").map(label => text(
      weight: "bold",
      size: 0.85em,
      label,
    )),
  ),
)

// The six cells of one reading.
#let _row-cells(a) = {
  let unit = a.at("unit", default: "")
  let value = a.at("value", default: none)
  let check = _check(value, _bounds(a))
  let quantity = a.at("quantity", default: none)
  let label = if quantity != none { quantity } else { _targets(a.at("target", default: none)) }
  (
    if label == none { [] } else { label },
    _target-text(a, unit),
    _tolerance-text(a, unit),
    if value == none { "" } else { _qty(_num(value), unit) },
    if check == none or check.error == none { "" } else { _error-text(check.error) },
    if check == none { "" } else {
      text(
        fill: _ink.at(check.result),
        weight: if check.result == "fail" { "bold" } else { "regular" },
        _verdicts.at(check.result),
      )
    },
  )
}

// The block's frame: a left rule, the head, then the body. A block never
// splits across a page break.
#let _frame(head, body) = block(
  breakable: false,
  width: 100%,
  above: 1.2em,
  below: 1.2em,
  stroke: (left: 1.5pt + _ink.rule),
  inset: (left: 8pt, rest: 6pt),
  {
    head
    parbreak()
    body
  },
)

#let _head(a, summary) = {
  text(weight: "bold", size: 0.85em, "MEASUREMENT")
  let id = a.at("id", default: none)
  let title = a.at("title", default: none)
  let status = a.at("status", default: none)
  if id != none { h(0.6em); raw(str(id)) }
  if title != none { h(0.6em); strong(title) }
  if summary != none { h(1fr); text(size: 0.85em, summary) } else if status != none {
    h(1fr)
    text(size: 0.85em, upper(str(status)))
  }
}

// Instruments, conditions, operator and time under the table.
#let _footer(a, instruments) = {
  let parts = ()
  if instruments.len() > 0 { parts.push([*Instruments* #instruments.join(" · ")]) }
  let conditions = a.at("conditions", default: none)
  if conditions != none { parts.push([*Conditions* #conditions]) }
  let operator = a.at("operator", default: none)
  if operator != none { parts.push([*Operator* #operator]) }
  let t = a.at("timestamp", default: a.at("date", default: none))
  if t != none {
    let shown = if type(t) == str { t.replace("T", " ") } else { t }
    parts.push([*Recorded* #shown])
  }
  if parts.len() > 0 { block(above: 6pt, text(size: 0.85em, fill: _ink.muted, parts.join(h(1.2em)))) }
}

// The targets and requirements a block names.
#let _targets-line(a) = {
  let t = _targets(a.at("target", default: none))
  let requirements = a.at("requirements", default: none)
  let parts = ()
  if t != none { parts.push([Targets #t]) }
  if requirements != none {
    let listed = if type(requirements) == array { requirements.join(", ") } else { requirements }
    parts.push([requirements #listed])
  }
  if parts.len() > 0 { block(above: 4pt, text(size: 0.85em, fill: _ink.muted, parts.join(" · "))) }
}

// "Pass · 4 of 4" when every reading passes, else "2 pass · 1 warn".
#let _summary(results) = {
  let checked = results.filter(r => r.result != none)
  let n = checked.len()
  if n == 0 { return none }
  let count(v) = checked.filter(r => r.result == v).len()
  if count("pass") == n { return "Pass · " + str(n) + " of " + str(n) }
  let present = ("pass", "warn", "fail").filter(v => count(v) > 0)
  present.map(v => str(count(v)) + " " + v).join(" · ")
}

#let _group = state("signex-measurement-group", none)
#let _groups = counter("signex-measurement-groups")
#let _results(key) = state("signex-measurement-results-" + key, ())

#let _instrument-list(a) = {
  let own = a.at("instrument", default: none)
  if own == none { () } else { (own,) }
}

// A reading on its own: the head with its result, the table of one row.
#let _single(a) = {
  let check = _check(a.at("value", default: none), _bounds(a))
  let summary = if check == none { none } else { _verdicts.at(check.result) }
  _frame(_head(a, summary), {
    _header-row
    _table-row(_row-cells(a))
    _footer(a, _instrument-list(a))
    _targets-line(a)
  })
}

// A block of readings: the readings inside its body are its rows.
#let _group-block(a, body) = {
  _groups.step()
  context {
    let key = str(_groups.get().first())
    let results = _results(key).final()
    let instruments = if a.at("instrument", default: none) != none { _instrument-list(a) } else {
      results.map(r => r.instrument).filter(i => i != none).dedup()
    }
    let framed = _frame(_head(a, _summary(results)), {
      _group.update((key: key, unit: a.at("unit", default: none)))
      _header-row
      body
      _group.update(none)
      _footer(a, instruments)
      _targets-line(a)
    })
    let caption = a.at("caption", default: none)
    if caption == none { framed } else {
      figure(kind: table, caption: figure.caption(position: top, caption), framed)
    }
  }
}

// #measurement(target: net("+3V3"), quantity: "ripple", value: 18.0,
// unit: "mV", limit: (max: 30), ...): one reading. With a body it is a
// block whose readings are the #measurement calls inside it.
#let measurement(..args) = {
  let a = args.named()
  let body = args.pos().at(0, default: none)
  if body != none { return _group-block(a, body) }
  context {
    let group = _group.get()
    if group == none { _single(a) } else {
      let row = if group.unit != none and "unit" not in a { a + (unit: group.unit) } else { a }
      let check = _check(row.at("value", default: none), _bounds(row))
      _results(group.key).update(r => r + (
        (
          result: if check == none { none } else { check.result },
          instrument: row.at("instrument", default: none),
        ),
      ))
      _table-row(_row-cells(row))
    }
  }
}

// What Signex found of every attachment (signex_notebook::derived).
#let _manifest() = json("/derived/attachments.json")

#let _missing(file, why) = block(
  width: 100%,
  inset: 8pt,
  stroke: 0.5pt + luma(60%),
  text(size: 0.85em, fill: _ink.muted, [#why #raw("attachments/" + str(file))]),
)

// A scope capture: the legend, the screen with its axis numbers, cursor
// labels and trigger mark, then the readout.
#let _scope(p) = {
  let s = p.scope
  let legend = s.channels.map(c => text(fill: rgb(c.colour), c.legend)).join(h(1em))
  block(width: 100%, {
    text(size: 0.8em, legend)
    layout(size => {
      let gutter = 2.6em
      let width = size.width - gutter
      let height = width * 0.8
      let screen = box(width: width, height: height, {
        image("/" + p.svg, width: width, height: height)
        for c in s.cursors {
          let offset = if c.label.starts-with("Y") {
            (x: 2pt, y: height * (4 - c.at) / 8 - 0.9em)
          } else {
            (x: width * c.at / 10 + 2pt, y: 2pt)
          }
          place(top + left, dx: offset.x, dy: offset.y, text(size: 0.7em, fill: _ink.cursor, c.label))
        }
        if s.trigger != none {
          place(top + left, dx: width * s.trigger / 10 - 0.25em, dy: -1.1em, text(size: 0.7em, "T"))
        }
      })
      let y-labels = box(width: gutter, height: height, for l in s.y_labels {
        place(top + right, dx: -3pt, dy: height * (4 - l.at) / 8 - 0.45em, text(size: 0.7em, l.text))
      })
      let x-labels = box(width: width, height: 1.2em, for l in s.x_labels {
        place(top + left, dx: width * l.at / 10 - 0.4em, dy: 2pt, text(size: 0.7em, l.text))
      })
      grid(columns: (gutter, width), y-labels, screen, [], x-labels)
    })
    text(size: 0.8em, (s.readout + (s.time_per_div,)).join(h(1.5em)))
  })
}

// The line under a figure: the file, when it was taken and the part
// values it was taken with.
#let _source-line(file, a) = {
  let taken = a.at("taken", default: none)
  let stored = a.at("taken-with", default: none)
  let values = if type(stored) == dictionary and stored.len() > 0 {
    stored.pairs().map(((part, value)) => part + " " + value).join(" · ")
  } else { none }
  let shown = if type(taken) == str { taken.replace("T", " ") } else { "" }
  let when = if shown == "" { "taken" } else { "taken " + shown }
  let note = if values == none { if taken == none { none } else { when } } else { when + " with " + values }
  let parts = (raw("attachments/" + str(file)),)
  if note != none { parts.push(note) }
  block(above: 4pt, text(size: 0.75em, fill: _ink.muted, parts.join(h(0.8em))))
}

// "Stale: taken with L1 = 3.3 µH; L1 is now 4.7 µH."
#let _stale-note(changes) = block(
  width: 100%,
  above: 4pt,
  inset: 5pt,
  stroke: (left: 1.5pt + _ink.stale),
  text(size: 0.8em, {
    let said = changes.map(c => {
      let now = if c.now == none { " is no longer in the design" } else { " is now " + c.now }
      "taken with " + c.part + " = " + c.was + "; " + c.part + now
    })
    strong("Stale: ")
    said.join("; ") + "."
  }),
)

#let _stale-badge = box(
  inset: 2pt,
  stroke: 0.6pt + _ink.stale,
  text(size: 0.7em, fill: _ink.stale, "Stale"),
)

// #attachment("scope/ripple-2A-revB.csv", caption: [...], target: ...,
// taken-with: ("L1": "4.7 µH"), ...): a numbered figure of a file in
// notebook/attachments/. A scope CSV is drawn as a plot, a picture shown
// as is.
// What a figure shows of its file.
#let _shown(file, entry) = {
  if file == none { return _missing("", "An attachment names no file:") }
  if entry == none or not entry.present { return _missing(file, "Missing attachment:") }
  if entry.plot != none { return _scope(entry.plot) }
  if entry.image != none { return image("/" + entry.image, width: 100%) }
  if entry.error != none { return _missing(file, "Cannot draw (" + entry.error + "):") }
  _missing(file, "No preview for")
}

#let attachment(..args) = {
  let a = args.named()
  let file = args.pos().at(0, default: a.at("file", default: none))
  let entry = if file == none { none } else { _manifest().files.at(file, default: none) }
  let shown = _shown(file, entry)
  let stale = entry != none and entry.changes.len() > 0
  figure(
    kind: image,
    caption: a.at("caption", default: none),
    block(width: 100%, {
      if stale { place(top + right, _stale-badge) }
      shown
      if file != none { _source-line(file, a) }
      if stale { _stale-note(entry.changes) }
    }),
  )
}
