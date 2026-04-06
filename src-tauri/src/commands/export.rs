use crate::engine::parser::SchematicSheet;
use std::collections::HashMap;

/// Generate BOM as CSV string from schematic data
#[tauri::command]
pub fn generate_bom(data: SchematicSheet) -> Result<String, String> {
    let mut csv = String::from("Designator,Value,Footprint,Library,Quantity\n");

    // Group by value+footprint for quantity counting
    let mut groups: HashMap<(String, String, String), Vec<String>> = HashMap::new();
    for sym in &data.symbols {
        if sym.is_power { continue; }
        if sym.reference.ends_with("?") { continue; }
        let key = (sym.value.clone(), sym.footprint.clone(), sym.lib_id.clone());
        groups.entry(key).or_default().push(sym.reference.clone());
    }

    // Sort by first designator in each group
    let mut entries: Vec<_> = groups.into_iter().collect();
    entries.sort_by(|a, b| {
        let a_ref = a.1.first().unwrap();
        let b_ref = b.1.first().unwrap();
        natural_sort(a_ref, b_ref)
    });

    for ((value, footprint, lib_id), refs) in &entries {
        let designators = refs.join(", ");
        // Escape CSV fields
        let val_escaped = csv_escape(value);
        let fp_escaped = csv_escape(footprint);
        let lib_escaped = csv_escape(lib_id);
        let des_escaped = csv_escape(&designators);
        csv.push_str(&format!("{},{},{},{},{}\n", des_escaped, val_escaped, fp_escaped, lib_escaped, refs.len()));
    }

    Ok(csv)
}

/// Export netlist in KiCad format
#[tauri::command]
pub fn export_netlist(data: SchematicSheet) -> Result<String, String> {
    use crate::engine::parser::Point;

    let mut out = String::new();
    out.push_str("(export (version \"E\")\n");
    out.push_str("  (design\n");
    out.push_str(&format!("    (source \"{}\")\n", data.uuid));
    out.push_str("    (tool \"Alp EDA 0.1\")\n");
    out.push_str("  )\n");

    // Components
    out.push_str("  (components\n");
    for sym in &data.symbols {
        if sym.is_power { continue; }
        out.push_str(&format!("    (comp (ref \"{}\")\n", sym.reference));
        out.push_str(&format!("      (value \"{}\")\n", sym.value));
        out.push_str(&format!("      (footprint \"{}\")\n", sym.footprint));
        out.push_str(&format!("      (libsource (lib \"{}\") (part \"{}\"))\n",
            sym.lib_id.split(':').next().unwrap_or(""),
            sym.lib_id.split(':').last().unwrap_or("")));
        out.push_str("    )\n");
    }
    out.push_str("  )\n");

    // Libraries
    out.push_str("  (libraries\n");
    let mut libs: Vec<&str> = data.lib_symbols.keys().map(|k| k.split(':').next().unwrap_or("")).collect();
    libs.sort();
    libs.dedup();
    for lib in libs {
        out.push_str(&format!("    (library (logical \"{}\")\n      (uri \"\")\n    )\n", lib));
    }
    out.push_str("  )\n");

    // Nets — deduplicated by label text (basic extraction, not full connectivity)
    out.push_str("  (nets\n");
    let mut net_id = 1;
    let mut seen_nets = std::collections::HashSet::new();
    for label in &data.labels {
        if seen_nets.insert(label.text.clone()) {
            out.push_str(&format!("    (net (code {}) (name \"{}\"))\n", net_id, label.text));
            net_id += 1;
        }
    }
    out.push_str("  )\n");

    out.push_str(")\n");
    Ok(out)
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn natural_sort(a: &str, b: &str) -> std::cmp::Ordering {
    let a_prefix: String = a.chars().take_while(|c| c.is_alphabetic()).collect();
    let b_prefix: String = b.chars().take_while(|c| c.is_alphabetic()).collect();
    match a_prefix.cmp(&b_prefix) {
        std::cmp::Ordering::Equal => {
            let a_num: u32 = a.chars().skip_while(|c| c.is_alphabetic()).collect::<String>().parse().unwrap_or(0);
            let b_num: u32 = b.chars().skip_while(|c| c.is_alphabetic()).collect::<String>().parse().unwrap_or(0);
            a_num.cmp(&b_num)
        }
        other => other,
    }
}
