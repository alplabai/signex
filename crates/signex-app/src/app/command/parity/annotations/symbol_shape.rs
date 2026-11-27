//! The symbol editor's curve tools (#2838): the shape lands on the part and
//! body the symbol's editor shows (`SymbolDocument::view`), as a click of
//! the tool does.

use super::super::forms::Form::Operand;
use super::super::inputs::Input::ToolState;
use super::Annotation;

pub(super) const SYMBOL_SHAPE: &[Annotation] = &[(
    &[
        "symbol_draw_bezier",
        "symbol_draw_elliptical_arc",
        "symbol_draw_pie",
        "symbol_draw_round_rectangle",
    ],
    Operand,
    &[ToolState],
)];
