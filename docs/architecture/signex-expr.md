# signex-expr: the shared expression engine

Issue #1984. One engine evaluates every Signex value that may be a formula:
footprint sketch parameters today; part fields (#1986), sheet and project
variables (#1987), BOM columns (#1988) and board rules (#1989) next.

## Layer placement

`signex-expr` is a leaf domain crate: its only dependencies are `serde` and
`thiserror`, no workspace crate, no UI, no IO (it is in the `DOMAIN` set of
`scripts/check_arch_guard.sh`). Any crate may depend on it, `signex-types`
included, without pulling in the sketch solver. Today `signex-sketch`,
`signex-bake` and `signex-app` do (`scripts/arch-edges.txt`).

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
`LiteralContext::Length`, milli everywhere else. `m` before another unit
symbol (`10mV`) is always milli. See `docs/DECISIONS.md` (2026-11-18).

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
chain deeper than `MAX_EXPR_DEPTH` is `ExprError::TooDeep`.

The markup lookups (`${refdes:key}`, `@{name}`, `NET_NAME()`) and the
output-side refdes tables are to read the same resolvers (maintainer
decision 2026-11-18); `Binding::Text` gives them the stored text unchanged.

## What stays in signex-sketch

- The array index `i` / `j`: parsed as ordinary references and bound by
  `signex_sketch::expr::eval::EvalContext`, which implements `Resolver`.
- "A bare number is mm" (`EvalContext::with_unitless_as_mm`).
- The boundary check `signex_sketch::expr::eval::sketch_value`: a sketch
  value is a length, an angle or a count. An intermediate area is fine; a
  final `2mm * 3mm` is refused with `WrongFamily`.
- The parameter table reads every value as an expression with an optional
  `=` (`signex_expr::expression_body`); `.snxfpt` does not change.
