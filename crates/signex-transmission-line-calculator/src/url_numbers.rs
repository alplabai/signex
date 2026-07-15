pub(crate) fn parse_url_number(value: &str) -> Result<f64, std::num::ParseFloatError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(0.0)
    } else {
        trimmed.parse::<f64>()
    }
}
