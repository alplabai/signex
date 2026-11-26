//! Name lookup through a `Resolver`, cycles, the `=` / `==` rule and
//! dependency order.

use std::collections::BTreeMap;

use signex_expr::{
    Binding, ExprError, Layered, Quantity, Resolver, dependency_order, eval, parse, references,
};

fn texts(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn a_reference_reads_a_stored_text_under_the_eq_rule() {
    let fields = texts(&[
        ("R_load", "4.7k"),
        ("R1.Value", "= R_load * 2"),
        ("Note", "==not an expression"),
    ]);
    let q = eval(&parse("R1.Value / 2").unwrap(), &fields).unwrap();
    assert_eq!(q, Quantity::count(4700.0));
    // An escaped literal is text, not a number.
    assert!(matches!(
        eval(&parse("Note").unwrap(), &fields),
        Err(ExprError::BadQuantity(_))
    ));
}

#[test]
fn a_reference_cycle_is_an_error() {
    let fields = texts(&[("a", "= b + 1"), ("b", "= c"), ("c", "= a")]);
    assert!(matches!(
        eval(&parse("a").unwrap(), &fields),
        Err(ExprError::Cycle(_))
    ));
    let own = texts(&[("a", "= a")]);
    assert!(matches!(
        eval(&parse("a").unwrap(), &own),
        Err(ExprError::Cycle(_))
    ));
}

#[test]
fn an_unknown_name_is_reported() {
    let fields = texts(&[]);
    assert_eq!(
        eval(&parse("missing").unwrap(), &fields),
        Err(ExprError::Unknown("missing".into()))
    );
}

#[test]
fn an_inner_scope_shadows_an_outer_one() {
    let project = texts(&[("VCC", "3.3V"), ("I_max", "10mA")]);
    let sheet = texts(&[("VCC", "5V")]);
    let scope = Layered::new(vec![&sheet as &dyn Resolver, &project]);
    let r = eval(&parse("VCC / I_max").unwrap(), &scope).unwrap();
    assert!((r.value - 500.0).abs() < 1e-9);
}

#[test]
fn a_host_resolver_can_refuse_a_name() {
    struct Indexed;
    impl Resolver for Indexed {
        fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
            match name {
                "i" => Err(ExprError::ArrayIndexOutsideArray),
                "n" => Ok(Some(Binding::Value(Quantity::count(3.0)))),
                _ => Ok(None),
            }
        }
    }
    assert_eq!(
        eval(&parse("n * 2").unwrap(), &Indexed),
        Ok(Quantity::count(6.0))
    );
    assert_eq!(
        eval(&parse("i").unwrap(), &Indexed),
        Err(ExprError::ArrayIndexOutsideArray)
    );
}

#[test]
fn dependency_order_puts_dependencies_first_and_rejects_cycles() {
    let mut table = BTreeMap::new();
    table.insert("d".to_string(), parse("b + c").unwrap());
    table.insert("b".to_string(), parse("a + 1mm").unwrap());
    table.insert("c".to_string(), parse("a * 2").unwrap());
    table.insert("a".to_string(), parse("1mm").unwrap());
    let order = dependency_order(&table).unwrap();
    let pos = |n: &str| order.iter().position(|x| x == n).unwrap();
    assert!(pos("a") < pos("b") && pos("a") < pos("c"));
    assert!(pos("b") < pos("d") && pos("c") < pos("d"));

    table.insert("a".to_string(), parse("d").unwrap());
    assert!(matches!(dependency_order(&table), Err(ExprError::Cycle(_))));
}

#[test]
fn references_lists_each_name_once() {
    let node = parse("max(a, b) + a * lookup(k, [1, 2], [b, c])").unwrap();
    assert_eq!(references(&node), ["a", "b", "k", "c"]);
}
