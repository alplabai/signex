//! QA harness — exercise every v0.8 exporter against a real Signex
//! project and report sizes / sheet counts / validation issues.
//!
//! Usage: `cargo run --example qa_harness -p signex-output -- <project.snxprj> [out_dir]`
//!
//! Reads the project, walks every sheet via the same logic the app uses,
//! drives PdfExporter / NetlistExporter / BomExporter (CSV / HTML / XLSX),
//! and prints a one-line summary per artefact plus any BOM validation
//! issues. Exits non-zero if any export panics or returns Err.

use std::path::{Path, PathBuf};

use signex_output::{
    BomColumn, BomExporter, BomFormat, BomGrouping, BomOptions, ExportContext, Exporter,
    NetlistExporter, NetlistOptions, PdfExporter, PdfOptions, ProjectMetadata, SheetSnapshot,
    rollup,
};
use signex_types::format::SnxSchematic;
use signex_types::project::parse_project;

fn main() {
    let mut args = std::env::args().skip(1);
    let project_path = match args.next() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: qa_harness <project.snxprj> [out_dir]");
            std::process::exit(2);
        }
    };
    let out_dir =
        PathBuf::from(args.next().unwrap_or_else(|| std::env::temp_dir().join("signex_qa").to_string_lossy().into_owned()));
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    println!("== Signex v0.8 QA harness ==");
    println!("project: {}", project_path.display());
    println!("out_dir: {}", out_dir.display());

    let project = parse_project(&project_path).expect("parse project");
    let project_dir = project_path.parent().expect("project parent");

    println!(
        "project name: {} | sheets: {} | active_variant: {:?}",
        project.name,
        project.sheets.len(),
        project.active_variant
    );

    let mut snapshots: Vec<SheetSnapshot> = Vec::new();
    for (i, entry) in project.sheets.iter().enumerate() {
        let abs = project_dir.join(&entry.filename);
        let text = match std::fs::read_to_string(&abs) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  ! read {}: {e}", abs.display());
                continue;
            }
        };
        let parsed = match SnxSchematic::parse(&text) {
            Ok(snx) => snx.sheet,
            Err(e) => {
                eprintln!("  ! parse {}: {e}", abs.display());
                continue;
            }
        };
        println!(
            "  sheet [{:>2}] {:<32} symbols={:>3} wires={:>3} labels={:>3}",
            i + 1,
            entry.filename,
            parsed.symbols.len(),
            parsed.wires.len(),
            parsed.labels.len(),
        );
        snapshots.push(SheetSnapshot {
            path: abs,
            schematic: parsed,
            sheet_name: entry.name.clone(),
            sheet_number: i + 1,
            sheet_count: project.sheets.len().max(1),
        });
    }

    if snapshots.is_empty() {
        eprintln!("no sheets parsed — cannot continue");
        std::process::exit(1);
    }

    let active = &snapshots[0];
    let tb = &active.schematic.title_block;
    let comment = |n: usize| tb.get(&format!("comment{n}")).cloned().unwrap_or_default();
    let metadata = ProjectMetadata {
        title: tb.get("title").cloned().unwrap_or_default(),
        revision: tb.get("rev").cloned().unwrap_or_default(),
        date: tb.get("date").cloned().unwrap_or_default(),
        company: tb.get("company").cloned().unwrap_or_default(),
        comments: [comment(1), comment(2), comment(3), comment(4)],
        custom_fields: std::collections::BTreeMap::new(),
    };
    let ctx = ExportContext {
        sheets: snapshots,
        metadata,
    };

    println!();
    println!("== PDF ==");
    qa_pdf(&ctx, &out_dir);

    println!();
    println!("== Netlist ==");
    qa_netlist(&ctx, &out_dir);

    println!();
    println!("== BOM (Grouped) ==");
    qa_bom(
        &ctx,
        &out_dir,
        BomGrouping::Grouped,
        "grouped",
    );
    println!();
    println!("== BOM (Flat) ==");
    qa_bom(&ctx, &out_dir, BomGrouping::Flat, "flat");

    println!();
    println!("== Done. Artefacts in: {}", out_dir.display());
}

fn qa_pdf(ctx: &ExportContext, out_dir: &Path) {
    let opts = PdfOptions::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        PdfExporter.export(ctx, &opts)
    }));
    match result {
        Ok(Ok(out)) => {
            let path = out_dir.join("qa.pdf");
            std::fs::write(&path, &out.bytes).expect("write pdf");
            println!(
                "  PDF: pages={} bytes={} → {}",
                out.page_count,
                out.bytes.len(),
                path.display()
            );
        }
        Ok(Err(e)) => eprintln!("  PDF: ERROR {e}"),
        Err(_) => eprintln!("  PDF: PANIC"),
    }
}

fn qa_netlist(ctx: &ExportContext, out_dir: &Path) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        NetlistExporter.export(ctx, &NetlistOptions::default())
    }));
    match result {
        Ok(Ok(out)) => {
            let path = out_dir.join("qa.net");
            std::fs::write(&path, &out.bytes).expect("write netlist");
            println!(
                "  Netlist: bytes={} → {}",
                out.bytes.len(),
                path.display()
            );
        }
        Ok(Err(e)) => eprintln!("  Netlist: ERROR {e}"),
        Err(_) => eprintln!("  Netlist: PANIC"),
    }
}

fn qa_bom(ctx: &ExportContext, out_dir: &Path, grouping: BomGrouping, label: &str) {
    let columns = vec![
        BomColumn::Name,
        BomColumn::Description,
        BomColumn::Designator,
        BomColumn::Value,
        BomColumn::Footprint,
        BomColumn::LibRef,
        BomColumn::Qty,
    ];
    // Show the rolled-up table once via signex-output::rollup, then
    // emit each format. The rollup is independent of format so a
    // single rollup feeds all 3 emitters.
    let table = rollup(
        ctx,
        &BomOptions {
            grouping,
            columns: columns.clone(),
            ..BomOptions::default()
        },
    );
    let fitted: u32 = table.rows.iter().map(|r| r.fitted_qty).sum();
    println!(
        "  rollup: {} row(s), {} fitted",
        table.rows.len(),
        fitted
    );

    for (format, ext) in [
        (BomFormat::Csv, "csv"),
        (BomFormat::Html, "html"),
        (BomFormat::Xlsx, "xlsx"),
    ] {
        let opts = BomOptions {
            grouping,
            format,
            columns: columns.clone(),
            ..BomOptions::default()
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            BomExporter.export(ctx, &opts)
        }));
        match result {
            Ok(Ok(out)) => {
                let path = out_dir.join(format!("qa_{label}.{ext}"));
                std::fs::write(&path, &out.bytes).expect("write bom");
                let issue_summary = if out.validation_report.has_errors() {
                    format!(
                        "{} error(s) {} warning(s)",
                        out.validation_report.error_count(),
                        out.validation_report.warning_count(),
                    )
                } else {
                    "clean".to_string()
                };
                println!(
                    "  BOM[{:?}]: bytes={:>7} validation={} → {}",
                    format,
                    out.bytes.len(),
                    issue_summary,
                    path.display()
                );
                // Print errors only (warnings are usually 1×missing MPN
                // per component which is verbose and historical
                // projects without MPN fields populated).
                if format == BomFormat::Csv {
                    use signex_output::BomIssueSeverity;
                    let mut shown = 0;
                    for issue in &out.validation_report.issues {
                        if issue.severity == BomIssueSeverity::Error {
                            println!("    ! ERR {:?}: {}", issue.rule, issue.message);
                            shown += 1;
                        }
                    }
                    let warn_kinds: std::collections::BTreeMap<String, usize> = out
                        .validation_report
                        .issues
                        .iter()
                        .filter(|i| i.severity == BomIssueSeverity::Warning)
                        .fold(std::collections::BTreeMap::new(), |mut acc, i| {
                            *acc.entry(format!("{:?}", i.rule)).or_insert(0) += 1;
                            acc
                        });
                    for (rule, count) in warn_kinds {
                        println!("    . WARN {rule}: {count} component(s)");
                    }
                    if shown == 0 && out.validation_report.error_count() > 0 {
                        println!(
                            "    (errors counted but none surfaced — bug?)"
                        );
                    }
                }
            }
            Ok(Err(e)) => eprintln!("  BOM[{:?}]: ERROR {e}", format),
            Err(_) => eprintln!("  BOM[{:?}]: PANIC", format),
        }
    }

    if !table.rows.is_empty() {
        let preview_count = 5.min(table.rows.len());
        println!("  rollup preview (first {preview_count} row(s)):");
        for r in table.rows.iter().take(preview_count) {
            println!(
                "    {:<24} qty={:<3} {:<14} {:<24} fp={}",
                truncate(&r.name, 24),
                r.qty,
                truncate(&r.value, 14),
                truncate(&r.references.join(","), 24),
                truncate(&r.footprint, 30),
            );
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out = s.chars().take(n.saturating_sub(1)).collect::<String>();
        out.push('…');
        out
    }
}
