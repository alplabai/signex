//! Function audit: drives each mockup row's controls through `Signex::update`
//! and checks what they change. Run by the audit tooling, not the PR gate.

#[path = "function_audit/g1.rs"]
mod g1;
#[path = "function_audit/g5.rs"]
mod g5;
#[path = "function_audit/g7a.rs"]
mod g7a;
#[path = "function_audit/g7b.rs"]
mod g7b;
#[path = "function_audit/g10.rs"]
mod g10;
