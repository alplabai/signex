pub mod ast;
pub mod parse;
pub mod eval;

#[derive(Debug, thiserror::Error)]
#[error("expr error stub")]
pub struct ExprError;
