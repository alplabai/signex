//! The numbers a power tree reads from text: a part's rail parameters, its
//! efficiency, a rail voltage spelled in a net name, and which net names are
//! ground. The rules are in `docs/POWER_TREE.md`.

/// The parameter a part names the current it draws from a rail by: `Rail
/// Load <net>`. The same name the requirement checks read (#2943).
pub const LOAD_PARAMETER: &str = "Rail Load ";
/// The parameter a part names the most current it can supply to a rail by:
/// `Rail Supply <net>`. The same name the requirement checks read (#2943).
pub const SUPPLY_PARAMETER: &str = "Rail Supply ";
/// The parameter a converter names its efficiency by (`88 %`, `0.88`).
pub const EFFICIENCY_PARAMETER: &str = "Efficiency";

/// Whether a parameter supplies a rail or draws from one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailRole {
    Supply,
    Load,
}

/// The rail a parameter names and its role; `None` for any other parameter
/// or a blank net name.
pub fn rail_parameter(parameter: &str) -> Option<(&str, RailRole)> {
    let (net, role) = if let Some(net) = parameter.strip_prefix(SUPPLY_PARAMETER) {
        (net, RailRole::Supply)
    } else {
        (parameter.strip_prefix(LOAD_PARAMETER)?, RailRole::Load)
    };
    let net = net.trim();
    (!net.is_empty()).then_some((net, role))
}

/// A current in A: a number, optionally followed by `A`, `mA`, `uA` or
/// `µA`; finite and not negative.
pub fn parse_current_a(text: &str) -> Option<f64> {
    let text = text.trim();
    let (number, per_amp) = [("mA", 1e3), ("uA", 1e6), ("µA", 1e6), ("A", 1.0)]
        .iter()
        .find_map(|&(unit, per_amp)| text.strip_suffix(unit).map(|number| (number, per_amp)))
        .unwrap_or((text, 1.0));
    let value: f64 = number.trim().parse().ok()?;
    let current = value / per_amp;
    (current.is_finite() && current >= 0.0).then_some(current)
}

/// An efficiency as a fraction above 0 and at most 1: `88 %` or `88%` as a
/// percentage, a bare number as the fraction (`0.88`). A bare number above 1
/// is refused rather than read as a percentage.
pub fn parse_efficiency(text: &str) -> Option<f64> {
    let text = text.trim();
    let fraction = match text.strip_suffix('%') {
        Some(percent) => percent.trim().parse::<f64>().ok()? / 100.0,
        None => text.parse::<f64>().ok()?,
    };
    (fraction.is_finite() && fraction > 0.0 && fraction <= 1.0).then_some(fraction)
}

/// The voltage a net name spells, read from its last word that is one:
/// `3V3`, `+3V3`, `1V8`, `5V0`, `12V`, `3.3V`, `-12V` (`VOUT_3V3` is 3.3,
/// `VIN_12V` is 12). Words are split at `_`, `/` and spaces. `None` when no
/// word spells a voltage, or it spells 0.
pub fn voltage_from_net_name(name: &str) -> Option<f64> {
    name.split(['_', '/', ' '])
        .filter_map(word_voltage)
        .next_back()
}

fn word_voltage(word: &str) -> Option<f64> {
    let (sign, rest) = match word.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, word.strip_prefix('+').unwrap_or(word)),
    };
    let at = rest.find(['V', 'v'])?;
    let (whole, fraction) = (&rest[..at], &rest[at + 1..]);
    if whole.is_empty() || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole_ok = whole.chars().all(|c| c.is_ascii_digit() || c == '.');
    if !whole_ok || (whole.contains('.') && !fraction.is_empty()) {
        return None;
    }
    let text = if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    };
    let volts = sign * text.parse::<f64>().ok()?;
    (volts.is_finite() && volts != 0.0).then_some(volts)
}

/// Whether a power net is a ground and no rail: one of its words (split at
/// any character that is not a letter or digit) is `GND` or starts or ends
/// with it (`AGND`, `GND_ISO`, `PGND`), or is `VSS`, `EARTH` or `0V`.
pub fn is_ground_net(name: &str) -> bool {
    name.split(|c: char| !c.is_alphanumeric())
        .map(str::to_ascii_uppercase)
        .any(|word| {
            word.starts_with("GND")
                || word.ends_with("GND")
                || matches!(word.as_str(), "VSS" | "EARTH" | "0V")
        })
}
