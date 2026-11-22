//! Function audit: drives each mockup row's controls through `Signex::update`
//! and checks what they change. Run by the audit tooling, not the PR gate.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely"
)]

#[path = "regression/settle_support.rs"]
#[allow(dead_code)]
mod settle_support;

#[allow(dead_code)]
#[path = "audit/fixture.rs"]
mod fixture;

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
