//! ERC DSL parser and validator.

pub mod ast;
pub mod compiler;
pub mod error;
pub mod parser;
pub mod validator;

pub use ast::*;
pub use compiler::{CompiledExpr, CompiledRule, to_eval_fns};
pub use error::DslError;

/// Parse DSL source text into `RuleAst` items.
///
/// This API converts parser diagnostics into `DslError::Parse` values.
pub fn parse(src: &str) -> Result<Vec<RuleAst>, Vec<DslError>> {
    match parser::parse(src) {
        Ok(rules) => Ok(rules),
        Err(errors) => Err(errors
            .into_iter()
            .map(|(offset, message)| DslError::Parse(offset, offset, message))
            .collect()),
    }
}

/// Validate parsed rules against helper and field compatibility constraints.
pub fn validate(rules: &[RuleAst]) -> Vec<DslError> {
    validator::validate(rules)
}

/// Compile validated AST rules into executable evaluator closures.
pub fn compile(rules: &[RuleAst]) -> Result<Vec<CompiledRule>, Vec<DslError>> {
    compiler::compile(rules)
}

/// Parse, validate, and compile DSL source in a single call.
pub fn parse_validate_compile(src: &str) -> Result<Vec<CompiledRule>, Vec<DslError>> {
    let rules = parse(src)?;
    let validation_errors = validate(&rules);
    if !validation_errors.is_empty() {
        return Err(validation_errors);
    }
    compile(&rules)
}

/// Parse, validate, compile, and convert rules into engine evaluator closures.
pub fn parse_validate_compile_to_eval_fns(
    src: &str,
) -> Result<Vec<signex_erc::engine::EvalFn>, Vec<DslError>> {
    let compiled = parse_validate_compile(src)?;
    Ok(to_eval_fns(&compiled))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use signex_erc::context::{ErcContext, ErcNet, ErcPin, ErcSymbol, PaperSize};
    use signex_types::schematic::{PinElectricalType, Point};

    use super::*;

    fn sample_context() -> ErcContext {
        ErcContext {
            paper_size: PaperSize::A4,
            symbols: vec![ErcSymbol {
                uuid: "00000000-0000-0000-0000-000000000001"
                    .parse()
                    .expect("valid uuid"),
                reference: "U1".to_string(),
                value: "MCU".to_string(),
                position: Point::new(10.0, 10.0),
                is_power: false,
                pins: vec![ErcPin {
                    world_pos: Point::new(10.0, 10.0),
                    electrical_type: PinElectricalType::Output,
                    required: true,
                    connected: true,
                }],
                attrs: HashMap::from([("class".to_string(), "logic".to_string())]),
            }],
            wires: vec![],
            buses: vec![],
            labels: vec![],
            junctions: vec![],
            no_connects: vec![],
            bus_entries: vec![],
            child_sheets: vec![],
            nets: vec![ErcNet {
                name: "I2C_SDA".to_string(),
                class: "i2c".to_string(),
                pin_types: vec![PinElectricalType::Output],
                has_driver: true,
                has_pullup: false,
            }],
            children: HashMap::new(),
        }
    }

    #[test]
    fn parse_validate_compile_to_eval_fns_runs_net_rule() {
        let src = r#"
            rule net_rule
            on net
            when has_driver() and net.name matches "^I2C_"
            then warning "Driver net"
        "#;

        let eval_fns = parse_validate_compile_to_eval_fns(src).expect("dsl should compile");
        assert_eq!(eval_fns.len(), 1);

        let ctx = sample_context();
        let diagnostics = eval_fns[0](&ctx);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id.as_str(), "user::net_rule");
        assert_eq!(diagnostics[0].message, "Driver net");
    }

    #[test]
    fn invalid_regex_returns_compile_error() {
        let src = r#"
            rule bad_rx
            on net
            when net.name matches "["
            then error "bad"
        "#;

        let errors = match parse_validate_compile(src) {
            Ok(_) => panic!("regex should fail"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, DslError::InvalidRegex { .. })),
            "expected InvalidRegex error"
        );
    }
}
