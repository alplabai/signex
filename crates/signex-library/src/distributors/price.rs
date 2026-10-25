//! Helpers shared by the clients that read prices.

use crate::distributor::{DistributorError, DistributorPart, PriceBreak, PricingSnapshot};

/// Currencies whose retail price strings use `,` as the decimal mark.
const COMMA_DECIMAL: &[&str] = &[
    "EUR", "SEK", "DKK", "NOK", "PLN", "CZK", "HUF", "RON", "BGN", "RUB", "TRY", "BRL",
];

/// Mouser's `PriceBreaks[].Price` is only ever a display string in the
/// request locale (fixtures: `mouser_partnumber_usd.json` has `"$1,234.50"`,
/// `mouser_keyword_eur.json` has `"0,094 €"` and `"1.234,56 €"`); the payload
/// has no numeric price field to prefer. A lone separator followed by exactly
/// three digits is therefore ambiguous (`1,234` / `1.234`): it is a thousands
/// mark where the currency's convention uses the other character as decimal
/// (`1,234` USD, `1.234` EUR), and a decimal otherwise (`0,125` EUR, whose
/// integer part is `0`). Both separators present: the last one is the decimal.
///
/// Parse a displayed price such as `"$0.10"`, `"0,10 €"`, `"1,234.56"` or
/// `"1.234,56 €"` into a number, using `currency` to read an ambiguous
/// separator. Refuses text without digits and anything not finite.
pub(crate) fn parse_price(text: &str, currency: &str) -> Result<f64, String> {
    let kept: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    let kept = kept.trim_matches(|c| c == '.' || c == ',');
    if !kept.chars().any(|c| c.is_ascii_digit()) {
        return Err(format!("price {text:?} has no digits"));
    }
    let comma_locale = COMMA_DECIMAL.contains(&currency.to_ascii_uppercase().as_str());
    let (dots, commas) = (kept.matches('.').count(), kept.matches(',').count());
    let after_last = |sep: char| kept.rsplit(sep).next().map_or(0, str::len);
    let decimal = match (dots, commas) {
        (0, 0) => None,
        (_, 0) => {
            let integer_part = kept.split('.').next().unwrap_or("");
            let thousands =
                comma_locale && (dots > 1 || (after_last('.') == 3 && integer_part != "0"));
            (!thousands).then_some('.')
        }
        (0, _) => {
            let thousands = !comma_locale && commas == 1 && after_last(',') == 3;
            let decimal = !thousands && commas == 1;
            decimal.then_some(',')
        }
        _ => Some(if kept.rfind('.') > kept.rfind(',') {
            '.'
        } else {
            ','
        }),
    };
    let mut normalised = String::with_capacity(kept.len());
    for c in kept.chars() {
        match c {
            '.' | ',' if Some(c) == decimal => normalised.push('.'),
            '.' | ',' => {}
            d => normalised.push(d),
        }
    }
    let value: f64 = normalised
        .parse()
        .map_err(|_| format!("price {text:?} is not a number"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("price {text:?} is not finite"))
    }
}

/// Percent-encode one URL path segment (keeps `A-Za-z0-9-._~`).
pub(crate) fn encode_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Key of the part's `extra` map under which a response parser records why
/// it could not read the price, so [`pricing_or_error`] can say so.
pub(crate) const PRICING_ERROR_KEY: &str = "pricing_error";

/// Build the snapshot a client attaches to a part: breaks under the client's
/// own name, one currency, optional stock, and the distributor part number
/// the prices belong to.
pub(crate) fn snapshot(
    client_name: &str,
    currency: &str,
    stock: Option<u32>,
    part_number: Option<String>,
    breaks: Vec<PriceBreak>,
) -> PricingSnapshot {
    PricingSnapshot {
        captured_at: chrono::Utc::now(),
        currency: currency.to_ascii_uppercase(),
        stock,
        part_number: part_number.filter(|p| !p.trim().is_empty()),
        by_distributor: std::collections::BTreeMap::from([(client_name.to_string(), breaks)]),
    }
}

/// The pricing of a freshly fetched part, or the reason it has none.
pub(crate) fn pricing_or_error(part: DistributorPart) -> Result<PricingSnapshot, DistributorError> {
    if let Some(pricing) = part.pricing {
        return Ok(pricing);
    }
    match part.extra.get(PRICING_ERROR_KEY) {
        Some(why) => Err(DistributorError::Backend(format!(
            "price unreadable: {why}"
        ))),
        None => Err(DistributorError::NotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_common_price_shapes() {
        for (text, cur, want) in [
            ("$0.10", "USD", 0.10),
            ("0,10 €", "EUR", 0.10),
            ("1,234.56", "USD", 1234.56),
            ("1.234,56 €", "EUR", 1234.56),
            ("kr 1,50", "SEK", 1.5),
            ("1,234", "USD", 1234.0),
            ("1.234", "EUR", 1234.0),
            ("0.125", "EUR", 0.125),
            ("0,125", "EUR", 0.125),
            ("12", "USD", 12.0),
        ] {
            assert_eq!(parse_price(text, cur), Ok(want), "{text} {cur}");
        }
    }

    #[test]
    fn refuses_text_that_is_not_a_price() {
        for text in ["", "N/A", "--", "$", "1.2.3,4,5"] {
            assert!(parse_price(text, "USD").is_err(), "{text:?}");
        }
    }

    #[test]
    fn path_segment_is_encoded() {
        assert_eq!(encode_path_segment("311-10.0KHRCT-ND"), "311-10.0KHRCT-ND");
        assert_eq!(encode_path_segment("a/b c"), "a%2Fb%20c");
    }
}

/// Whether `hit` is the row's part for an MPN-only lookup: the MPN equals
/// exactly (ignoring case) and the manufacturer is the row's. A row without a
/// manufacturer accepts an MPN-exact hit only when it is the sole one, since
/// the MPN alone can name parts of different makers.
pub(crate) fn is_exact_mpn_hit(
    hit: &DistributorPart,
    wanted: &DistributorPart,
    exact_hits: usize,
) -> bool {
    if !hit.mpn.trim().eq_ignore_ascii_case(wanted.mpn.trim()) {
        return false;
    }
    let maker = wanted.manufacturer.trim();
    if maker.is_empty() {
        return exact_hits == 1;
    }
    hit.manufacturer.trim().eq_ignore_ascii_case(maker)
}

/// The one hit that is the row's part for an MPN-only lookup, or `None`.
pub(crate) fn pick_exact_mpn_hit<'a>(
    hits: &'a [DistributorPart],
    wanted: &DistributorPart,
) -> Option<&'a DistributorPart> {
    let exact = hits
        .iter()
        .filter(|h| h.mpn.trim().eq_ignore_ascii_case(wanted.mpn.trim()))
        .count();
    let mut matching = hits.iter().filter(|h| is_exact_mpn_hit(h, wanted, exact));
    let first = matching.next()?;
    // Two distinct parts both claiming the row's MPN and maker is ambiguous.
    matching.next().is_none().then_some(first)
}
