//! The text forms of a block's list properties, shared by the `block_*`
//! operands and the Properties fields so both read and write the same
//! spelling: rails as `VIN_12V, VOUT_3V3`, key specs as
//! `Vout=3.30 V; Fsw=570 kHz`.

use std::collections::BTreeMap;

use super::refusal::Refusal;

/// The rails a comma-separated list names; blanks are dropped by the
/// model's own cleaning.
pub(crate) fn parse_rails(text: &str) -> Vec<String> {
    text.split(',')
        .map(|rail| rail.trim().to_string())
        .collect()
}

/// The rails as Properties shows them.
pub(crate) fn rails_text(rails: &[String]) -> String {
    rails.join(", ")
}

/// The key specs `name=value` pairs separated by `;` name. A pair with no
/// `=` is refused with its text.
pub(crate) fn parse_specs(text: &str) -> Result<BTreeMap<String, String>, Refusal> {
    let mut specs = BTreeMap::new();
    for pair in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let Some((name, value)) = pair.split_once('=') else {
            return Err(Refusal::BadSpec {
                text: pair.to_string(),
            });
        };
        specs.insert(name.trim().to_string(), value.trim().to_string());
    }
    Ok(specs)
}

/// The key specs as Properties shows them.
pub(crate) fn specs_text(specs: &BTreeMap<String, String>) -> String {
    specs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// A length in mm typed as text: a finite number.
pub(crate) fn parse_mm(text: &str) -> Result<f64, Refusal> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| Refusal::BadLength {
            text: text.trim().to_string(),
        })
}

/// A length as Properties shows it: up to three decimals, no trailing
/// zeros.
pub(crate) fn mm_text(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rails_and_specs_read_back_as_they_are_shown() {
        let rails = vec!["VIN_12V".to_string(), "VOUT_3V3".to_string()];
        assert_eq!(parse_rails(&rails_text(&rails)), rails);
        let specs = BTreeMap::from([
            ("Fsw".to_string(), "570 kHz".to_string()),
            ("Vout".to_string(), "3.30 V".to_string()),
        ]);
        assert_eq!(specs_text(&specs), "Fsw=570 kHz; Vout=3.30 V");
        assert_eq!(parse_specs(&specs_text(&specs)).unwrap(), specs);
    }

    #[test]
    fn a_spec_without_a_value_and_a_length_that_is_not_a_number_are_refused() {
        assert_eq!(
            parse_specs("Vout").unwrap_err(),
            Refusal::BadSpec {
                text: "Vout".into()
            }
        );
        assert_eq!(parse_specs(" ; ").unwrap(), BTreeMap::new());
        assert!(parse_mm("ten").is_err());
        assert!(parse_mm("inf").is_err());
        assert_eq!(parse_mm(" 12.5 ").unwrap(), 12.5);
        assert_eq!(mm_text(12.5), "12.5");
        assert_eq!(mm_text(40.0), "40");
        assert_eq!(mm_text(-0.0001), "0");
    }
}
