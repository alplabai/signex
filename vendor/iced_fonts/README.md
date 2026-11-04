# iced_fonts (Signex stub)

Upstream `iced_fonts` 0.3.0 embeds many icon fonts behind feature flags. This
vendored copy has all of them removed: no font files, no `include_bytes!`, no
icon-generating macros. Only the `advanced_text` feature remains (enabled by
`iced_aw`), and the crate exports nothing. See `../README.md` for why.
