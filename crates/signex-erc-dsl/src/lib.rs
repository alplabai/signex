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
    use signex_types::schematic::{PinDirection, Point};

    use super::*;

    fn sample_context() -> ErcContext {
        ErcContext {
            paper_size: PaperSize::A4,
            symbols: vec![ErcSymbol {
                uuid: "00000000-0000-0000-0000-000000000001"
                    .parse()
                    .expect("valid uuid"),
                reference: "U1".to_string(),
                lib_id: String::new(),
                unit: 1,
                value: "MCU".to_string(),
                position: Point::new(10.0, 10.0),
                is_power: false,
                pins: vec![ErcPin {
                    world_pos: Point::new(10.0, 10.0),
                    number: "1".into(),
                    electrical_type: PinDirection::Output,
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
            no_erc_directives: vec![],
            bus_entries: vec![],
            child_sheets: vec![],
            nets: vec![ErcNet {
                name: "I2C_SDA".to_string(),
                class: "i2c".to_string(),
                pin_types: vec![PinDirection::Output],
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
    fn origin_no_erc_directive_does_not_suppress_a_net_scoped_diagnostic() {
        let src = r#"
            rule net_rule
            on net
            when has_driver()
            then warning "Driver net"
        "#;
        let eval_fns = parse_validate_compile_to_eval_fns(src).expect("dsl should compile");
        let mut ctx = sample_context();
        ctx.no_erc_directives = vec![signex_erc::context::ErcNoErcDirective {
            position: Point::new(0.0, 0.0),
        }];
        let diagnostics = signex_erc::engine::run_all_with_dsl(&ctx, &eval_fns);
        assert!(diagnostics.iter().any(|d| d.message == "Driver net"));
    }

    #[test]
    fn dsl_rule_keeps_its_declared_severity_when_the_placeholder_builtin_is_off() {
        use signex_erc::{RuleKind, Severity, Violation, apply_overrides, engine};

        let src = r#"
            rule net_rule
            on net
            when has_driver() and net.name matches "^I2C_"
            then warning "Driver net"
        "#;
        let eval_fns = parse_validate_compile_to_eval_fns(src).expect("dsl should compile");
        let violations: Vec<Violation> = engine::run_all_with_dsl(&sample_context(), &eval_fns)
            .into_iter()
            .map(Violation::from)
            .collect();

        // The compiler files a Net rule under NetLabelConflict; switching
        // that built-in off or to Error must not touch the user's rule.
        for level in [Severity::Off, Severity::Error, Severity::Info] {
            let overrides = HashMap::from([(RuleKind::NetLabelConflict, level)]);
            let out = apply_overrides(violations.clone(), &overrides);
            let user: Vec<_> = out
                .iter()
                .filter(|v| v.rule_id.as_str() == "user::net_rule")
                .collect();
            assert_eq!(user.len(), 1, "override {level:?} removed the DSL rule");
            assert_eq!(user[0].severity, Severity::Warning);
            assert_eq!(user[0].display_name(), "user::net_rule");
            assert_eq!(user[0].builtin_kind(), None);
        }
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

    fn pin_context(kinds: &[PinDirection]) -> ErcContext {
        let mut ctx = sample_context();
        ctx.symbols[0].pins = kinds
            .iter()
            .map(|&electrical_type| ErcPin {
                world_pos: Point::new(10.0, 10.0),
                number: "1".into(),
                electrical_type,
                required: true,
                connected: false,
            })
            .collect();
        ctx
    }

    fn hits(src: &str, ctx: &ErcContext) -> usize {
        let eval_fns =
            parse_validate_compile_to_eval_fns(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
        eval_fns[0](ctx).len()
    }

    /// #676: `pin.kind` accepts every spelling `has_pin_kind` accepts.
    #[test]
    fn pin_kind_comparison_accepts_the_has_pin_kind_aliases() {
        let ctx = pin_context(&[
            PinDirection::PowerInput,
            PinDirection::ThreeStatable,
            PinDirection::OpenDrainLow,
        ]);
        for (literal, matching) in [
            (r#""power_in""#, 1),
            ("tristate", 1),
            (r#""open_collector""#, 1),
            ("PowerInput", 1),
            (r#""ThreeStatable""#, 1),
            ("clock", 0),
        ] {
            for (op, expected) in [("==", matching), ("!=", 3 - matching)] {
                let src = format!(r#"rule r on pin when pin.kind {op} {literal} then error "x""#);
                assert_eq!(hits(&src, &ctx), expected, "{src}");
            }
        }
    }

    #[test]
    fn pin_kind_matches_tests_the_canonical_name() {
        let ctx = pin_context(&[PinDirection::PowerInput, PinDirection::Input]);
        let src = r#"rule r on pin when pin.kind matches "^Power" then error "x""#;
        assert_eq!(hits(src, &ctx), 1);
    }

    /// #676: a pin kind no rule could ever match is a validation error, not a
    /// rule that silently never fires.
    #[test]
    fn unknown_pin_kind_literal_is_rejected() {
        for (target, when) in [
            ("net", r#"has_pin_kind("pwr_in")"#),
            ("pin", r#"pin.kind == "pwr_in""#),
            ("pin", r#"pin.kind != "pwr_in""#),
        ] {
            let src = format!(r#"rule r on {target} when {when} then error "x""#);
            let Err(errors) = parse_validate_compile(&src) else {
                panic!("{src} should not compile");
            };
            assert!(
                errors
                    .iter()
                    .any(|e| e.to_string().contains("unknown pin kind 'pwr_in'")),
                "{src}: {errors:?}"
            );
        }

        let src = r#"rule r on net when has_pin_kind("output") then error "x""#;
        assert_eq!(
            hits(src, &sample_context()),
            1,
            "a known kind still compiles"
        );
    }

    /// The compiler resolves pin kinds itself, so an unknown one fails even
    /// through `compile()`, which does not run the validator.
    #[test]
    fn compile_rejects_unknown_pin_kind_without_validation() {
        for (target, when) in [
            ("net", r#"has_pin_kind("pwr_in")"#),
            ("pin", r#"pin.kind != "pwr_in""#),
        ] {
            let src = format!(r#"rule r on {target} when {when} then error "x""#);
            let rules = parse(&src).expect("parses");
            let Err(errors) = compile(&rules) else {
                panic!("{src} should not compile");
            };
            assert!(
                errors
                    .iter()
                    .any(|e| e.to_string().contains("unknown pin kind 'pwr_in'")),
                "{src}: {errors:?}"
            );
        }
    }
}
