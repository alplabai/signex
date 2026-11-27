use super::*;

fn computed(name: &str, expression: &str) -> TemplateColumn {
    TemplateColumn::Computed {
        name: name.into(),
        expression: expression.into(),
    }
}

fn template() -> BomTemplate {
    BomTemplate {
        columns: vec![
            TemplateColumn::Standard("designator".into()),
            TemplateColumn::Standard("qty".into()),
            TemplateColumn::Standard("value".into()),
            TemplateColumn::Field("MPN".into()),
            computed("Power", "= VCC^2 / Value"),
            computed("Cost", "= Quantity * UnitPrice"),
        ],
    }
}

#[test]
fn a_template_with_computed_columns_round_trips_byte_identical() {
    let text = encode_bom_template(&template()).unwrap();
    assert!(text.contains("format = \"snxbomt/1\""), "{text}");
    let read = parse_bom_template(&text).unwrap();
    assert_eq!(read, template());
    assert_eq!(encode_bom_template(&read).unwrap(), text);
}

#[test]
fn a_hand_written_template_reads_each_column_kind() {
    let text = "format = \"snxbomt/1\"\n\n\
        [[columns]]\ncolumn = \"designator\"\n\n\
        [[columns]]\nfield = \"MPN\"\n\n\
        [[columns]]\nname = \"Power\"\nexpression = \"= VCC^2 / Value\"\n";
    let read = parse_bom_template(text).unwrap();
    assert_eq!(
        read.columns,
        vec![
            TemplateColumn::Standard("designator".into()),
            TemplateColumn::Field("MPN".into()),
            computed("Power", "= VCC^2 / Value"),
        ]
    );
}

#[test]
fn a_missing_or_newer_token_is_refused() {
    let none = "[[columns]]\ncolumn = \"qty\"\n";
    assert!(matches!(
        parse_bom_template(none),
        Err(BomTemplateError::Version(_))
    ));
    let newer = "format = \"snxbomt/2\"\n[[columns]]\nwidth = 3\n";
    assert!(matches!(
        parse_bom_template(newer),
        Err(BomTemplateError::Version(_))
    ));
}

#[test]
fn an_unknown_key_or_a_mixed_column_is_refused() {
    let unknown = "format = \"snxbomt/1\"\n[[columns]]\ncolumn = \"qty\"\nwidth = 3\n";
    assert!(matches!(
        parse_bom_template(unknown),
        Err(BomTemplateError::Parse(_))
    ));
    let mixed = "format = \"snxbomt/1\"\n[[columns]]\ncolumn = \"qty\"\nfield = \"MPN\"\n";
    assert!(matches!(
        parse_bom_template(mixed),
        Err(BomTemplateError::Column { index: 1, .. })
    ));
}

#[test]
fn a_bad_computed_column_is_refused_on_save_and_on_read() {
    for bad in [
        computed("", "= 1"),
        computed("Power", "VCC^2"),
        computed("Power", "= VCC^"),
    ] {
        let template = BomTemplate {
            columns: vec![TemplateColumn::Standard("qty".into()), bad],
        };
        assert!(matches!(
            encode_bom_template(&template),
            Err(BomTemplateError::Column { index: 2, .. })
        ));
    }
    let twice = BomTemplate {
        columns: vec![computed("Power", "= 1"), computed("Power", "= 2")],
    };
    assert!(encode_bom_template(&twice).is_err());
    let unparsable = "format = \"snxbomt/1\"\n[[columns]]\nname = \"P\"\nexpression = \"= (\"\n";
    assert!(parse_bom_template(unparsable).is_err());
}

#[test]
fn check_computed_is_the_one_rule() {
    assert!(check_computed("Power", "= VCC^2 / Value").is_ok());
    assert!(check_computed("  ", "= 1").is_err());
    assert!(
        check_computed("Power", "1")
            .unwrap_err()
            .contains("must start with `=`")
    );
}
