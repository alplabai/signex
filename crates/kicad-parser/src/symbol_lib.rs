use std::collections::HashMap;

use signex_types::schematic::LibSymbol;

use crate::error::ParseError;
use crate::schematic::parse_lib_symbol;
use crate::sexpr;

/// Parse a `.kicad_sym` symbol library file and return a map of symbol ID to `LibSymbol`.
pub fn parse_symbol_lib(input: &str) -> Result<HashMap<String, LibSymbol>, ParseError> {
    let root = sexpr::parse(input)?;

    if root.keyword() != Some("kicad_symbol_lib") {
        return Err(ParseError::InvalidSExpr(
            "Not a KiCad symbol library file".to_string(),
        ));
    }

    // Build a set of all top-level symbol IDs for O(1) subsymbol checks
    let all_sym_ids: std::collections::HashSet<String> = root
        .find_all("symbol")
        .iter()
        .filter_map(|s| s.first_arg().map(|a| a.to_string()))
        .collect();

    // child_id -> parent_id for (extends ...) symbols
    let mut extends_map: HashMap<String, String> = HashMap::new();

    let mut results: Vec<(String, LibSymbol)> = Vec::new();

    for sym_node in root.find_all("symbol") {
        let id = sym_node.first_arg().unwrap_or("").to_string();

        // Skip sub-symbols: only skip if the prefix (before _N_M) matches
        // a top-level symbol that already exists in the parent kicad_symbol_lib.
        if id.contains('_') {
            let parts: Vec<&str> = id.rsplitn(3, '_').collect();
            if parts.len() >= 3
                && parts[0].parse::<u32>().is_ok()
                && parts[1].parse::<u32>().is_ok()
            {
                let prefix = parts[2];
                if all_sym_ids.contains(prefix) {
                    continue;
                }
            }
        }

        // Record (extends "ParentId") relationship
        if let Some(parent_id) = sym_node.find("extends").and_then(|e| e.first_arg()) {
            extends_map.insert(id.clone(), parent_id.to_string());
        }

        let lib = parse_lib_symbol(sym_node);
        results.push((id, lib));
    }

    // Second pass: resolve (extends) -- derived symbols inherit graphics/pins from parent.
    // Handles transitive chains (A extends B extends C) by iterating until stable.
    if !extends_map.is_empty() {
        // Build id -> index lookup
        let id_to_idx: HashMap<String, usize> = results
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (id.clone(), i))
            .collect();

        // Resolve potentially chained extends (up to 8 levels deep)
        for _ in 0..8 {
            let mut changed = false;
            for (child_id, parent_id) in &extends_map {
                let child_idx = match id_to_idx.get(child_id) {
                    Some(&i) => i,
                    None => continue,
                };
                let parent_idx = match id_to_idx.get(parent_id) {
                    Some(&i) => i,
                    None => continue,
                };
                if results[child_idx].1.pins.is_empty()
                    && results[child_idx].1.graphics.is_empty()
                    && (!results[parent_idx].1.pins.is_empty()
                        || !results[parent_idx].1.graphics.is_empty())
                {
                    let parent_lib = results[parent_idx].1.clone();
                    let child = &mut results[child_idx].1;
                    child.graphics = parent_lib.graphics;
                    child.pins = parent_lib.pins;
                    child.show_pin_numbers = parent_lib.show_pin_numbers;
                    child.show_pin_names = parent_lib.show_pin_names;
                    child.pin_name_offset = parent_lib.pin_name_offset;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    let mut map = HashMap::with_capacity(results.len());
    for (id, lib) in results {
        map.insert(id, lib);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_symbol_lib() {
        let input = r#"(kicad_symbol_lib
  (version 20231120)
  (generator "test")
  (symbol "Device:R"
    (pin_names (offset 0))
    (symbol "Device:R_0_1"
      (rectangle (start -1.016 -2.54) (end 1.016 2.54)
        (stroke (width 0.254) (type default))
        (fill (type none))
      )
    )
    (symbol "Device:R_1_1"
      (pin passive line (at 0 3.81 270) (length 1.27)
        (name "~" (effects (font (size 1.27 1.27))))
        (number "1" (effects (font (size 1.27 1.27))))
      )
      (pin passive line (at 0 -3.81 90) (length 1.27)
        (name "~" (effects (font (size 1.27 1.27))))
        (number "2" (effects (font (size 1.27 1.27))))
      )
    )
  )
)"#;
        let map = parse_symbol_lib(input).unwrap();
        assert_eq!(map.len(), 1);
        let r = map.get("Device:R").unwrap();
        assert!(!r.graphics.is_empty());
        assert_eq!(r.pins.len(), 2);
    }
}
