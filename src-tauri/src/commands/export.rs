use crate::engine::parser::SchematicSheet;
use std::collections::HashMap;

/// Generate BOM as CSV string from schematic data
#[tauri::command]
pub fn generate_bom(data: SchematicSheet) -> Result<String, String> {
    let mut csv = String::from("Designator,Value,Footprint,Library,Quantity\n");

    // Group by value+footprint for quantity counting
    let mut groups: HashMap<(String, String, String), Vec<String>> = HashMap::new();
    for sym in &data.symbols {
        if sym.is_power {
            continue;
        }
        if sym.reference.ends_with("?") {
            continue;
        }
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
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            des_escaped,
            val_escaped,
            fp_escaped,
            lib_escaped,
            refs.len()
        ));
    }

    Ok(csv)
}

fn escape_sexpr(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Export netlist in KiCad format
#[tauri::command]
pub fn export_netlist(data: SchematicSheet) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("(export (version \"E\")\n");
    out.push_str("  (design\n");
    out.push_str(&format!("    (source \"{}\")\n", data.uuid));
    out.push_str("    (tool \"Signex 0.1\")\n");
    out.push_str("  )\n");

    // Components
    out.push_str("  (components\n");
    for sym in &data.symbols {
        if sym.is_power {
            continue;
        }
        out.push_str(&format!(
            "    (comp (ref \"{}\")\n",
            escape_sexpr(&sym.reference)
        ));
        out.push_str(&format!("      (value \"{}\")\n", escape_sexpr(&sym.value)));
        out.push_str(&format!(
            "      (footprint \"{}\")\n",
            escape_sexpr(&sym.footprint)
        ));
        out.push_str(&format!(
            "      (libsource (lib \"{}\") (part \"{}\"))\n",
            escape_sexpr(sym.lib_id.split(':').next().unwrap_or("")),
            escape_sexpr(sym.lib_id.split(':').next_back().unwrap_or(""))
        ));
        out.push_str("    )\n");
    }
    out.push_str("  )\n");

    // Libraries
    out.push_str("  (libraries\n");
    let mut libs: Vec<&str> = data
        .lib_symbols
        .keys()
        .map(|k| k.split(':').next().unwrap_or(""))
        .collect::<Vec<_>>();
    libs.sort();
    libs.dedup();
    for lib in libs {
        out.push_str(&format!(
            "    (library (logical \"{}\")\n      (uri \"\")\n    )\n",
            lib
        ));
    }
    out.push_str("  )\n");

    // Nets — deduplicated by label text (basic extraction, not full connectivity)
    out.push_str("  (nets\n");
    let mut net_id = 1;
    let mut seen_nets = std::collections::HashSet::new();
    for label in &data.labels {
        if seen_nets.insert(label.text.clone()) {
            out.push_str(&format!(
                "    (net (code {}) (name \"{}\"))\n",
                net_id,
                escape_sexpr(&label.text)
            ));
            net_id += 1;
        }
    }
    out.push_str("  )\n");

    out.push_str(")\n");
    Ok(out)
}

/// Configurable BOM export
#[tauri::command]
pub fn generate_bom_configured(
    data: SchematicSheet,
    columns: Vec<String>,
    group_by: Vec<String>,
    format: String,
) -> Result<String, String> {
    // Collect all symbols that are not power and have valid designators
    let mut rows: Vec<HashMap<String, String>> = Vec::new();
    for sym in &data.symbols {
        if sym.is_power || sym.reference.ends_with('?') {
            continue;
        }
        let mut row = HashMap::new();
        row.insert("Designator".to_string(), sym.reference.clone());
        row.insert("Value".to_string(), sym.value.clone());
        row.insert("Footprint".to_string(), sym.footprint.clone());
        row.insert("Library".to_string(), sym.lib_id.clone());
        row.insert("UUID".to_string(), sym.uuid.clone());
        for (k, v) in &sym.fields {
            row.insert(k.clone(), v.clone());
        }
        rows.push(row);
    }

    // Group rows
    let group_keys: Vec<&str> = if group_by.is_empty() {
        vec!["Value", "Footprint"]
    } else {
        group_by.iter().map(|s| s.as_str()).collect()
    };

    let mut groups: HashMap<Vec<String>, Vec<HashMap<String, String>>> = HashMap::new();
    for row in &rows {
        let key: Vec<String> = group_keys
            .iter()
            .map(|k| row.get(*k).cloned().unwrap_or_default())
            .collect();
        groups.entry(key).or_default().push(row.clone());
    }

    let mut sorted_groups: Vec<_> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| {
        let a_ref = a.1.first().and_then(|r| r.get("Designator")).map(|s| s.as_str()).unwrap_or("");
        let b_ref = b.1.first().and_then(|r| r.get("Designator")).map(|s| s.as_str()).unwrap_or("");
        natural_sort(a_ref, b_ref)
    });

    // Determine columns to output
    let cols: Vec<&str> = if columns.is_empty() {
        vec!["Designator", "Value", "Footprint", "Library", "Quantity"]
    } else {
        columns.iter().map(|s| s.as_str()).collect()
    };

    match format.as_str() {
        "csv" | "" => {
            let mut out = String::new();
            out.push_str(&cols.join(","));
            out.push('\n');
            for (_key, group_rows) in &sorted_groups {
                let designators: Vec<&str> = group_rows.iter()
                    .filter_map(|r| r.get("Designator").map(|s| s.as_str()))
                    .collect();
                let mut designators_sorted = designators.clone();
                designators_sorted.sort_by(|a, b| natural_sort(a, b));
                let first = group_rows.first().unwrap();
                let mut fields: Vec<String> = Vec::new();
                for col in &cols {
                    match *col {
                        "Designator" => fields.push(csv_escape(&designators_sorted.join(", "))),
                        "Quantity" => fields.push(group_rows.len().to_string()),
                        other => fields.push(csv_escape(first.get(other).map(|s| s.as_str()).unwrap_or(""))),
                    }
                }
                out.push_str(&fields.join(","));
                out.push('\n');
            }
            Ok(out)
        }
        "tsv" => {
            let mut out = String::new();
            out.push_str(&cols.join("\t"));
            out.push('\n');
            for (_key, group_rows) in &sorted_groups {
                let mut designators: Vec<&str> = group_rows.iter()
                    .filter_map(|r| r.get("Designator").map(|s| s.as_str()))
                    .collect();
                designators.sort_by(|a, b| natural_sort(a, b));
                let first = group_rows.first().unwrap();
                let mut fields: Vec<String> = Vec::new();
                for col in &cols {
                    match *col {
                        "Designator" => fields.push(designators.join(", ")),
                        "Quantity" => fields.push(group_rows.len().to_string()),
                        other => fields.push(first.get(other).map(|s| s.as_str()).unwrap_or("").to_string()),
                    }
                }
                out.push_str(&fields.join("\t"));
                out.push('\n');
            }
            Ok(out)
        }
        other => Err(format!("Unsupported BOM format: {}", other)),
    }
}

/// Export netlist in XML format
#[tauri::command]
pub fn export_netlist_xml(data: SchematicSheet) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<export version=\"E\">\n");
    out.push_str("  <design>\n");
    out.push_str(&format!("    <source>{}</source>\n", xml_escape(&data.uuid)));
    out.push_str("    <tool>Signex 0.1</tool>\n");
    out.push_str("  </design>\n");

    out.push_str("  <components>\n");
    for sym in &data.symbols {
        if sym.is_power {
            continue;
        }
        out.push_str("    <comp>\n");
        out.push_str(&format!("      <ref>{}</ref>\n", xml_escape(&sym.reference)));
        out.push_str(&format!("      <value>{}</value>\n", xml_escape(&sym.value)));
        out.push_str(&format!(
            "      <footprint>{}</footprint>\n",
            xml_escape(&sym.footprint)
        ));
        let lib = sym.lib_id.split(':').next().unwrap_or("");
        let part = sym.lib_id.split(':').next_back().unwrap_or("");
        out.push_str(&format!(
            "      <libsource lib=\"{}\" part=\"{}\"/>\n",
            xml_escape(lib),
            xml_escape(part)
        ));
        if !sym.fields.is_empty() {
            out.push_str("      <fields>\n");
            for (k, v) in &sym.fields {
                out.push_str(&format!(
                    "        <field name=\"{}\">{}</field>\n",
                    xml_escape(k),
                    xml_escape(v)
                ));
            }
            out.push_str("      </fields>\n");
        }
        out.push_str("    </comp>\n");
    }
    out.push_str("  </components>\n");

    out.push_str("  <nets>\n");
    let mut net_id = 1;
    let mut seen_nets = std::collections::HashSet::new();
    for label in &data.labels {
        if seen_nets.insert(label.text.clone()) {
            out.push_str(&format!(
                "    <net code=\"{}\" name=\"{}\"/>\n",
                net_id,
                xml_escape(&label.text)
            ));
            net_id += 1;
        }
    }
    out.push_str("  </nets>\n");
    out.push_str("</export>\n");
    Ok(out)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
            let a_num: u32 = a
                .chars()
                .skip_while(|c| c.is_alphabetic())
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            let b_num: u32 = b
                .chars()
                .skip_while(|c| c.is_alphabetic())
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            a_num.cmp(&b_num)
        }
        other => other,
    }
}
