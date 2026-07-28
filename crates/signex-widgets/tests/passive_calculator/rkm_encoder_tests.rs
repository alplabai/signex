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
