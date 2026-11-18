use super::*;

fn table(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn a_literal_variable_shows_as_written() {
    let project = table(&[("REVISION_TEXT", "Rev B"), ("ESCAPED", "==A")]);
    let resolver = TextTable(&project);
    assert_eq!(
        variable_text(&resolver, "REVISION_TEXT"),
        Some(Ok("Rev B".to_string()))
    );
    assert_eq!(
        variable_text(&resolver, "ESCAPED"),
        Some(Ok("=A".to_string()))
    );
    assert_eq!(variable_text(&resolver, "UNKNOWN"), None);
}

#[test]
fn an_expression_variable_evaluates_through_other_variables() {
    let project = table(&[("VCC", "3.3V"), ("VREF", "= VCC / 2")]);
    assert_eq!(
        variable_text(&TextTable(&project), "VREF"),
        Some(Ok("1.65V".to_string()))
    );
}

#[test]
fn a_sheet_variable_shadows_the_project_variable() {
    let project = table(&[("VCC", "5V"), ("VREF", "= VCC / 2")]);
    let sheet = table(&[("VCC", "3.3V")]);
    let (sheet_table, project_table) = (TextTable(&sheet), TextTable(&project));
    let layered = Layered::new(vec![&sheet_table as &dyn Resolver, &project_table]);
    // The project's expression reads the sheet's VCC on this sheet.
    assert_eq!(
        variable_text(&layered, "VREF"),
        Some(Ok("1.65V".to_string()))
    );
    assert_eq!(
        variable_text(&project_table, "VREF"),
        Some(Ok("2.5V".to_string()))
    );
}

#[test]
fn a_variable_cycle_is_an_error_not_an_overflow() {
    let project = table(&[("A", "= B + 1"), ("B", "= A * 2")]);
    assert!(matches!(
        variable_text(&TextTable(&project), "A"),
        Some(Err(ExprError::Cycle(_)))
    ));
    let issues = project_variable_issues(&project);
    assert_eq!(issues.len(), 2);
    assert!(
        issues
            .iter()
            .all(|issue| matches!(issue.error, ExprError::Cycle(_)))
    );
    assert!(issues[0].to_string().starts_with("Project variable 'A': "));
}

#[test]
fn an_unknown_reference_is_reported_for_the_sheet() {
    let project = table(&[("VCC", "3.3V")]);
    let sheet = table(&[
        ("HALF", "= VDD / 2"),
        ("OK", "= VCC * 2"),
        ("NOTE", "text"),
    ]);
    let issues = sheet_variable_issues(&sheet, &project);
    assert_eq!(
        issues,
        vec![VariableIssue {
            scope: VariableScope::Sheet,
            name: "HALF".to_string(),
            error: ExprError::Unknown("VDD".to_string()),
        }]
    );
}

#[test]
fn display_text_applies_the_eq_rule() {
    let project = table(&[("VCC", "3.3V")]);
    let resolver = TextTable(&project);
    assert_eq!(display_text("= VCC * 2", &resolver), Ok("6.6V".to_string()));
    assert_eq!(display_text("Power", &resolver), Ok("Power".to_string()));
    assert_eq!(display_text("==5", &resolver), Ok("=5".to_string()));
    assert!(display_text("= NOPE", &resolver).is_err());
}

#[test]
fn a_name_matches_exactly_before_ignoring_case() {
    let fields: HashMap<String, String> = HashMap::from([
        ("comment".to_string(), "lower".to_string()),
        ("Comment".to_string(), "upper".to_string()),
        ("Value".to_string(), "10k".to_string()),
    ]);
    let resolver = TextTable(&fields);
    assert_eq!(
        variable_text(&resolver, "Comment"),
        Some(Ok("upper".to_string()))
    );
    assert_eq!(
        variable_text(&resolver, "VALUE"),
        Some(Ok("10k".to_string()))
    );
}

#[test]
fn variable_names_are_identifiers() {
    assert!(is_variable_name("VCC"));
    assert!(is_variable_name("_v2"));
    assert!(!is_variable_name(""));
    assert!(!is_variable_name("2V"));
    assert!(!is_variable_name("a b"));
    assert!(!is_variable_name("R1.Value"));
}
