//! Touchstone (.sNp) S-parameter file parser and writer.
//!
//! Supports .s1p through .s4p files.
//! Format: `# <freq_unit> <param> <format> R <z_ref>`
//! Data lines: `freq re(S11) im(S11) re(S21) im(S21) ...`

use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SParameterData {
    pub num_ports: usize,
    pub frequencies: Vec<f64>,          // Hz
    pub params: HashMap<String, Vec<(f64, f64)>>, // "S11" → [(re, im), ...]
    pub z_ref: f64,                     // Reference impedance (ohms)
    pub freq_unit: String,              // "Hz", "kHz", "MHz", "GHz"
    pub format: String,                 // "RI", "MA", "DB"
}

impl SParameterData {
    /// Get S-parameter as complex values.
    pub fn get_complex(&self, name: &str) -> Option<Vec<Complex64>> {
        self.params.get(name).map(|vals| {
            vals.iter()
                .map(|&(re, im)| match self.format.as_str() {
                    "MA" => Complex64::from_polar(re, im.to_radians()),
                    "DB" => {
                        let mag = 10.0_f64.powf(re / 20.0);
                        Complex64::from_polar(mag, im.to_radians())
                    }
                    _ => Complex64::new(re, im), // RI
                })
                .collect()
        })
    }

    /// Get magnitude in dB.
    pub fn get_db(&self, name: &str) -> Option<Vec<f64>> {
        self.get_complex(name).map(|vals| {
            vals.iter().map(|c| 20.0 * c.norm().log10()).collect()
        })
    }
}

/// Parse a Touchstone file content string.
pub fn parse_touchstone(content: &str) -> Result<SParameterData, String> {
    let mut freq_unit = "GHz".to_string();
    let mut param_type = "S".to_string();
    let mut format = "MA".to_string();
    let mut z_ref = 50.0_f64;
    let mut num_ports = 0_usize;

    let mut frequencies: Vec<f64> = Vec::new();
    let mut raw_data: Vec<Vec<f64>> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('!') {
            continue;
        }

        // Option line: # Hz S RI R 50
        if trimmed.starts_with('#') {
            let tokens: Vec<&str> = trimmed[1..].split_whitespace().collect();
            for (i, tok) in tokens.iter().enumerate() {
                match tok.to_uppercase().as_str() {
                    "HZ" | "KHZ" | "MHZ" | "GHZ" | "THZ" => {
                        freq_unit = tok.to_uppercase();
                    }
                    "S" | "Y" | "Z" | "H" | "G" => {
                        param_type = tok.to_uppercase();
                    }
                    "RI" | "MA" | "DB" => {
                        format = tok.to_uppercase();
                    }
                    "R" => {
                        if i + 1 < tokens.len() {
                            if let Ok(r) = tokens[i + 1].parse::<f64>() {
                                z_ref = r;
                            }
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }

        // Data line — parse numbers
        let values: Vec<f64> = trimmed
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect();

        if values.is_empty() {
            continue;
        }

        raw_data.push(values);
    }

    if raw_data.is_empty() {
        return Err("No data found in Touchstone file".to_string());
    }

    // Determine number of ports from data width
    // For N ports: each frequency line has 1 + 2*N*N values (freq + re/im pairs)
    let first_len = raw_data[0].len();
    if first_len >= 3 {
        // .s1p: freq + 2 values = 3
        // .s2p: freq + 8 values = 9
        // .s3p: freq + 18 values = 19
        // .s4p: freq + 32 values = 33
        let data_vals = first_len - 1;
        for n in 1..=4 {
            if data_vals == 2 * n * n {
                num_ports = n;
                break;
            }
        }
        if num_ports == 0 {
            // Fallback: assume 2-port if 8 data values
            num_ports = if data_vals >= 8 { 2 } else { 1 };
        }
    }

    // Frequency multiplier
    let freq_mult = match freq_unit.as_str() {
        "HZ" => 1.0,
        "KHZ" => 1e3,
        "MHZ" => 1e6,
        "GHZ" => 1e9,
        "THZ" => 1e12,
        _ => 1e9, // Default GHz
    };

    // Build S-parameter map
    let mut params: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    let expected_pairs = num_ports * num_ports;

    // Initialize parameter vectors
    for i in 1..=num_ports {
        for j in 1..=num_ports {
            params.insert(format!("S{}{}", i, j), Vec::new());
        }
    }

    for row in &raw_data {
        if row.is_empty() {
            continue;
        }

        let freq = row[0] * freq_mult;
        frequencies.push(freq);

        // Extract S-parameter pairs
        let mut pair_idx = 0;
        for i in 1..=num_ports {
            for j in 1..=num_ports {
                let base = 1 + pair_idx * 2;
                let (v1, v2) = if base + 1 < row.len() {
                    (row[base], row[base + 1])
                } else {
                    (0.0, 0.0)
                };
                let key = format!("S{}{}", i, j);
                if let Some(vec) = params.get_mut(&key) {
                    vec.push((v1, v2));
                }
                pair_idx += 1;
            }
        }
    }

    Ok(SParameterData {
        num_ports,
        frequencies,
        params,
        z_ref,
        freq_unit,
        format,
    })
}

/// Write S-parameter data as Touchstone format string.
pub fn write_touchstone(data: &SParameterData) -> String {
    let mut lines = Vec::new();

    lines.push("! Touchstone file generated by Signex".to_string());
    lines.push(format!(
        "# {} {} {} R {:.1}",
        data.freq_unit, "S", data.format, data.z_ref
    ));

    let n = data.num_ports;
    for (fi, &freq) in data.frequencies.iter().enumerate() {
        let freq_mult = match data.freq_unit.as_str() {
            "HZ" => 1.0,
            "KHZ" => 1e3,
            "MHZ" => 1e6,
            "GHZ" => 1e9,
            _ => 1e9,
        };
        let mut vals = vec![format!("{:.6e}", freq / freq_mult)];

        for i in 1..=n {
            for j in 1..=n {
                let key = format!("S{}{}", i, j);
                if let Some(pairs) = data.params.get(&key) {
                    if fi < pairs.len() {
                        vals.push(format!("{:.8e}", pairs[fi].0));
                        vals.push(format!("{:.8e}", pairs[fi].1));
                    }
                }
            }
        }

        lines.push(vals.join(" "));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_s2p() {
        let content = r#"! 2-port S-parameters
# GHz S RI R 50
1.0  0.5 0.1  0.3 -0.2  0.3 -0.2  0.5 0.1
2.0  0.4 0.15 0.35 -0.25 0.35 -0.25 0.4 0.15
"#;
        let data = parse_touchstone(content).unwrap();
        assert_eq!(data.num_ports, 2);
        assert_eq!(data.frequencies.len(), 2);
        assert_eq!(data.frequencies[0], 1.0e9);
        assert_eq!(data.frequencies[1], 2.0e9);
        assert!(data.params.contains_key("S11"));
        assert!(data.params.contains_key("S21"));
        assert_eq!(data.z_ref, 50.0);
    }

    #[test]
    fn test_roundtrip() {
        let content = "# GHz S RI R 50\n1.0 0.5 0.1 0.3 -0.2 0.3 -0.2 0.5 0.1\n";
        let data = parse_touchstone(content).unwrap();
        let written = write_touchstone(&data);
        let reparsed = parse_touchstone(&written).unwrap();
        assert_eq!(data.frequencies.len(), reparsed.frequencies.len());
        assert_eq!(data.num_ports, reparsed.num_ports);
    }
}
