use crate::sexpr::{Atom, SExpr};

pub trait IntoSExprNode {
    fn into_sexpr_node(self) -> SExpr;
}

pub fn atom<T: IntoSExprNode>(value: T) -> SExpr {
    value.into_sexpr_node()
}

pub fn raw(value: impl Into<String>) -> SExpr {
    SExpr::Atom(Atom::Raw(value.into()))
}

pub fn quoted(value: impl Into<String>) -> SExpr {
    SExpr::Atom(Atom::Quoted(value.into()))
}

pub fn list(items: impl IntoIterator<Item = SExpr>) -> SExpr {
    SExpr::List(items.into_iter().collect())
}

impl IntoSExprNode for SExpr {
    fn into_sexpr_node(self) -> SExpr {
        self
    }
}

impl IntoSExprNode for Atom {
    fn into_sexpr_node(self) -> SExpr {
        SExpr::Atom(self)
    }
}

impl IntoSExprNode for String {
    fn into_sexpr_node(self) -> SExpr {
        quoted(self)
    }
}

impl IntoSExprNode for &str {
    fn into_sexpr_node(self) -> SExpr {
        quoted(self)
    }
}

impl IntoSExprNode for &String {
    fn into_sexpr_node(self) -> SExpr {
        quoted(self.clone())
    }
}

macro_rules! impl_into_sexpr_number {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoSExprNode for $ty {
                fn into_sexpr_node(self) -> SExpr {
                    raw(self.to_string())
                }
            }
        )*
    };
}

impl_into_sexpr_number!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);

#[macro_export]
macro_rules! sexpr {
    (($($items:tt)*)) => {
        $crate::sexpr_builder::list($crate::sexpr!(@items [] $($items)*))
    };

    (@items [$($built:expr,)*]) => {
        vec![$($built,)*]
    };
    (@items [$($built:expr,)*] ($($inner:tt)*) $($rest:tt)*) => {
        $crate::sexpr!(@items [$($built,)* $crate::sexpr!(($($inner)*)),] $($rest)*)
    };
    (@items [$($built:expr,)*] {$expr:expr} $($rest:tt)*) => {
        $crate::sexpr!(@items [$($built,)* $crate::sexpr_builder::atom($expr),] $($rest)*)
    };
    (@items [$($built:expr,)*] $lit:literal $($rest:tt)*) => {
        $crate::sexpr!(@items [$($built,)* $crate::sexpr_builder::atom($lit),] $($rest)*)
    };
    (@items [$($built:expr,)*] $ident:ident $($rest:tt)*) => {
        $crate::sexpr!(@items [$($built,)* $crate::sexpr_builder::raw(stringify!($ident)),] $($rest)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_builds_ast_with_raw_and_quoted_atoms() {
        let net_name = "GND";
        let expr = sexpr!((segment (net 1 {net_name}) (layer {raw("F.Cu")}) (width 0.25)));

        assert_eq!(
            expr.to_string(),
            r#"(segment (net 1 "GND") (layer F.Cu) (width 0.25))"#
        );
    }

    #[test]
    fn pretty_prints_nested_macro_output() {
        let expr = sexpr!((footprint "R_0603" (property "Reference" "R1") (property "Value" "10k")));
        let pretty = expr.pretty(0);

        assert!(pretty.contains("(property \"Reference\" \"R1\")"));
        assert!(pretty.contains("(property \"Value\" \"10k\")"));
    }
}