# signex-expr: the shared expression engine

Issue #1984. One engine evaluates every Signex value that may be a formula:
footprint sketch parameters today; part fields (#1986), sheet and project
variables (#1987), BOM columns (#1988) and board rules (#1989) next.

## Layer placement

`signex-expr` is a leaf domain crate: its only dependencies are `serde` and
`thiserror`, no workspace crate, no UI, no IO (it is in the `DOMAIN` set of
`scripts/check_arch_guard.sh`). Any crate may depend on it, `signex-types`
included, without pulling in the sketch solver. Today `signex-types`,
`signex-sketch`, `signex-bake` and `signex-app` do (`scripts/arch-edges.txt`).

| Module | Holds |
|---|---|
| `lex`, `parse` | Lexer and recursive-descent parser; the caps `MAX_EXPR_LEN`, `MAX_EXPR_DEPTH`, `MAX_EXPR_CHAIN` (#678) |
| `ast` | `ExprNode`, `BinOp`, `UnaryOp`, `Func` |
| `unit`, `symbols`, `dimension`, `format` | `Quantity`, the unit table with SI prefixes, dimension vectors, display |
| `eval`, `func` | The evaluator and the functions `min max abs sqrt round floor ceil` |
| `resolver`, `deps` | Name lookup and the dependency order of a table |
| `source` | The `=` / `==` rule |

## Units

A `Quantity` is a value and a `Unit`. Lengths (`mm mil in um`), angles
(`deg rad`) and plain numbers keep the unit they were written in, so a
sketch's numbers are bit-identical to before. Every other unit is
`Unit::Derived(Dimension)`: exponents over length (mm), angle (rad), time
(s), voltage (V) and current (A), with the value in those base units.
`*` and `/` add and subtract exponents, so `3.3V / 10mA` is `330Ω` and
`2mm * 3mm` is `6mm²`. Named units (Ω V A s Hz F H W C) take the prefixes
`p n u µ m k M G`; `4k7` is read as `4.7k`.

A bare `m` after a number is decided in one place,
`symbols::bare_m_meaning`: metres where the caller parses with
`LiteralContext::Length`, milli in `LiteralContext::General`. `m` before
another unit symbol (`10mV`) is always milli. See `docs/DECISIONS.md`
(2026-11-18).

Sketch parameters parse in `LiteralContext::Geometry`. A parameter may be
a length, an angle or a count, and a prefix-only literal (`10m`, `1u`,
`2n`) or an RKM literal (`4k7`) has no single reading there, so it is
refused (`ExprError::BadQuantity`) rather than baked as a count of
millimetres. A parameter named after a reserved constant (`pi`,
`RESERVED_CONSTANTS`) is refused with `ExprError::ReservedName`, because
every expression would read the constant instead.

A footprint field parses in the context its value must have (#2683): an
array pitch in `LiteralContext::Length`, so `2.54m` is metres; an array
count in `LiteralContext::Count` and a pad rotation or polar sweep in
`LiteralContext::Angle`. Both refuse a prefix-only or RKM literal (`4k7`,
`10m`), so a count or an angle is never read as an SI multiplier.

## Resolver

```rust
pub trait Resolver {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError>;
}
```

A name is the identifier as written, dotted paths included (`pad_pitch`,
`R1.Value`). A `Binding` is a value, a parsed expression, or a stored text
under the `=` / `==` rule. `Ok(None)` means unknown here; an `Err` is a
host refusal (the sketch's `i` outside an array). `Layered` stacks
resolvers so an inner scope shadows an outer one (sheet variables over
project variables). The evaluator owns cycle detection: a name met again
while it is still being evaluated is `ExprError::Cycle`, and a reference
chain deeper than `MAX_EXPR_DEPTH` is `ExprError::TooDeep`. One recursion
budget, `MAX_EVAL_DEPTH` tree levels, is shared by every body a reference
chain leads into, so a chain of full-size bodies is also `TooDeep` rather
than a stack overflow that aborts the process.

The markup lookups (`${refdes:key}`, `@{name}`, `NET_NAME()`) and the
output-side refdes tables read the same resolvers (#1987): every table is a
`signex_types::variables::TextTable` (an exact name first, then one in any
ASCII case), and `Binding::Text` gives them the stored text unchanged.

## Variables (#1987)

One variable model, in `signex_types::variables`:

| Scope | Stored in | Edited in |
|---|---|---|
| Project | `.snxprj` `parameters` (title-block keys and user names) | Project tab, Parameters |
| Sheet | `.snxsch` `[extras.sheet.variables]` (`SchematicSheet::variables`) | sheet Properties, Sheet Variables |

A value is a literal text or an `=` expression; the source is saved, the
evaluated text is drawn. Lookups stack `Layered` scopes innermost first: a
part's fields (for a value field's `@{name}` / `${name}`), then the sheet's
variables, then the project's. `${NAME}` special strings
(`signex_output::resolve`) try the built-in tokens first, then the variables.
A value that does not evaluate is drawn as its source and reported in
Messages (`sheet_variable_issues`, `project_variable_issues`); the
evaluator's cycle detection reports `a = b, b = a` as `ExprError::Cycle`.

The sheet table is an extras key, not a TSV row: a build that predates it
keeps it as an unknown `[extras.sheet]` table and writes it back, so the
`.snxsch` token does not change (`docs/FORMAT_VERSIONS.md`, "Changing a
format"), and a sheet without variables re-saves byte-identical.

## What stays in signex-sketch

- The array index `i` / `j`: parsed as ordinary references and bound by
  `signex_sketch::expr::eval::EvalContext`, which implements `Resolver`.
- "A bare number is mm" (`EvalContext::with_unitless_as_mm`).
- The boundary check `signex_sketch::expr::eval::sketch_value`: a sketch
  value is a length, an angle or a count. An intermediate area is fine; a
  final `2mm * 3mm` is refused with `WrongFamily`.
- The parameter table reads every value as an expression with an optional
  `=` (`signex_expr::expression_body`); `.snxfpt` does not change.

## Part fields (#1986)

`signex_types::part_fields` evaluates a part's `=` value or field with the
same evaluator and the same model, one layer further in: the parts of the
sheet (`PartFields`, names `<designator>.<field>`), then the sheet's and the
project's variables. Before a part's expression is evaluated, every bare name
that is one of its own fields is qualified with its key (`Value` in R2
becomes `R2.Value`), so a single evaluation follows references across parts
and a cycle between two parts is an `ExprError::Cycle`. The key is the
designator while it names the part alone, else `_u<uuid hex>`
(`signex_types::part_keys`); annotation and a designator edit call
`follow_part_keys`, which rewrites every reference from each part's old key
to its new one in the same undo step. The markup lookups stay text
substitution over the same resolvers; there is no second evaluator.

What a value shows is decided once, by `signex_output::shown_value`: the
canvas, PDF, SVG, print preview, BOM and netlist call it; ERC and the Net
Inspector read `part_fields::evaluated_value` (no page special strings).
