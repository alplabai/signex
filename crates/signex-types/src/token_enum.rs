//! One table per enum for the text tokens the TSV codecs write and read.

/// Generate the `enum -> token` and `token -> enum` function pair for a plain
/// enum from a single `Variant => "token"` table.
///
/// Two forms, both `token_enum!(vis fn to_token / from_token for Enum, ..)`:
///
/// - `fallback Variant`: `from_token` is total, unknown text yields the
///   fallback variant (the lenient board and schematic row codecs).
/// - `error ErrorType`: `from_token` returns
///   `Err(ErrorType::UnknownEnumToken { kind, got })` for unknown text
///   (the strict library codecs).
#[macro_export]
macro_rules! token_enum {
    (
        $vis:vis fn $to_token:ident / $from_token:ident for $ty:ident,
        fallback $fallback:ident {
            $($variant:ident => $token:literal),+ $(,)?
        }
    ) => {
        $vis fn $to_token(value: $ty) -> &'static str {
            match value {
                $($ty::$variant => $token,)+
            }
        }

        $vis fn $from_token(text: &str) -> $ty {
            match text {
                $($token => $ty::$variant,)+
                _ => $ty::$fallback,
            }
        }
    };
    (
        $vis:vis fn $to_token:ident / $from_token:ident for $ty:ident,
        error $error:ident {
            $($variant:ident => $token:literal),+ $(,)?
        }
    ) => {
        $vis fn $to_token(value: $ty) -> &'static str {
            match value {
                $($ty::$variant => $token,)+
            }
        }

        $vis fn $from_token(text: &str) -> Result<$ty, $error> {
            match text {
                $($token => Ok($ty::$variant),)+
                other => Err($error::UnknownEnumToken {
                    kind: stringify!($ty),
                    got: other.to_string(),
                }),
            }
        }
    };
}
