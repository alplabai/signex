use signex_widgets::passive_calculator::rkm_encoder::parse_value;
use signex_widgets::passive_calculator::{ESeries, RkmEncoder, RkmEncoderMessage};

#[test]
fn value_dropdown_contains_exactly_the_e192_series() {
    let encoder = RkmEncoder::default();
    let expected = ESeries::E192.preferred_numbers().collect::<Vec<_>>();
    assert_eq!(encoder.value_options(), expected);
    assert_eq!(encoder.value_options().first().unwrap().to_string(), "1.00");
    assert_eq!(encoder.value.to_string(), "4.70");
}

#[test]
fn selected_e192_value_drives_the_rkm_code() {
    let mut encoder = RkmEncoder::default();
    let value = encoder
        .value_options()
        .iter()
        .copied()
        .find(|number| number.significand == 499)
        .unwrap();
    encoder.update(RkmEncoderMessage::ValueChanged(value));
    assert_eq!(encoder.code().value_code(), "4K99");
}

// #2992 (mockup row 17): the Value field is a text input, the same as
// Target; the typed text drives the code.

fn typed(text: &str) -> RkmEncoder {
    let mut encoder = RkmEncoder::default();
    encoder.update(RkmEncoderMessage::ValueInputChanged(text.to_string()));
    encoder
}

#[test]
fn the_value_field_starts_on_the_default_value() {
    let encoder = RkmEncoder::default();
    assert_eq!(encoder.value_input, encoder.value.to_string());
}

#[test]
fn a_typed_value_drives_the_rkm_code() {
    let encoder = typed("4.7");
    assert_eq!(encoder.value_input, "4.7");
    assert_eq!(encoder.value.to_string(), "4.7");
    assert_eq!(encoder.code().value_code(), "4K7");
    assert_eq!(typed("220").code().value_code(), "220K");
    assert_eq!(typed("0.47").code().value_code(), "470R");
}

#[test]
fn a_comma_is_read_as_the_decimal_separator() {
    let encoder = typed("4,7");
    assert_eq!(encoder.value_input, "4,7");
    assert_eq!(encoder.code().value_code(), "4K7");
}

#[test]
fn an_unreadable_value_keeps_the_text_and_the_last_readable_value() {
    for text in ["", "4.", ".", "abc", "4.7.1", "-4.7", "0", "0.00", "99999"] {
        let mut encoder = typed("3.3");
        encoder.update(RkmEncoderMessage::ValueInputChanged(text.to_string()));
        assert_eq!(encoder.value_input, text, "{text:?}");
        assert_eq!(encoder.code().value_code(), "3K3", "{text:?}");
    }
}

#[test]
fn a_picked_value_rewrites_the_field() {
    let mut encoder = typed("abc");
    let value = encoder
        .value_options()
        .iter()
        .copied()
        .find(|number| number.significand == 499)
        .unwrap();
    encoder.update(RkmEncoderMessage::ValueChanged(value));
    assert_eq!(encoder.value_input, "4.99");
}

#[test]
fn the_value_reader_accepts_positive_decimals_only() {
    let read = |text: &str| parse_value(text).map(|number| number.to_string());
    assert_eq!(read("4.7").as_deref(), Some("4.7"));
    assert_eq!(read(" 4.70 ").as_deref(), Some("4.70"));
    assert_eq!(read(".5").as_deref(), Some("0.5"));
    assert_eq!(read("100").as_deref(), Some("100"));
    for text in ["", "4.", "+4", "1e3", "0", "70000", "1.0000000000001"] {
        assert_eq!(read(text), None, "{text:?}");
    }
}
