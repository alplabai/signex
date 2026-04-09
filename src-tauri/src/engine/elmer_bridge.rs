//! Elmer FEM bridge — SIF generator, mesh handling, and VTU result parser.
//!
//! Generates Elmer solver input files from PCB data for:
//! - Thermal analysis (heat equation)
//! - DC IR drop (static current conduction / Poisson equation)

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

// --- Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalConfig {
    pub power_sources: Vec<PowerSource>,
    pub ambient_temp: f64,       // Celsius
    pub convection_coeff: f64,   // W/(m^2*K)
    pub coupled_electric: bool,  // Joule heating
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSource {
    pub name: String,        // Component reference (e.g., "U1")
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub power_watts: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrDropConfig {
    pub power_net: String,
    pub supply_voltage: f64,
    pub source_pads: Vec<PadLocation>,
    pub sink_pads: Vec<SinkPad>,
    pub copper_thickness_mm: f64,
    pub copper_conductivity: f64, // S/m (default: 5.8e7 for copper)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadLocation {
    pub x_mm: f64,
    pub y_mm: f64,
    pub radius_mm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkPad {
    pub x_mm: f64,
    pub y_mm: f64,
    pub radius_mm: f64,
    pub current_a: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldResult {
    pub nodes: Vec<[f64; 2]>,
    pub triangles: Vec<[usize; 3]>,
    pub values: Vec<f64>,
    pub value_unit: String,
    pub value_name: String,
    pub min_value: f64,
    pub max_value: f64,
}

#[derive(Debug, Clone)]
pub struct ElmerProgress {
    pub percent: f64,
    pub message: String,
}

// --- Detect Elmer ---

pub fn detect_elmer_solver() -> Option<PathBuf> {
    which_bin("ElmerSolver")
}

pub fn detect_elmer_grid() -> Option<PathBuf> {
    which_bin("ElmerGrid")
}

fn which_bin(name: &str) -> Option<PathBuf> {
    let names = if cfg!(windows) {
        vec![format!("{}.exe", name), name.to_string()]
    } else {
        vec![name.to_string()]
    };

    if let Ok(path_var) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(sep) {
            for n in &names {
                let candidate = Path::new(dir).join(n);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    let search = &[r"C:\Program Files\Elmer 9.0\bin", r"C:\Elmer\bin"];
    #[cfg(target_os = "linux")]
    let search = &["/usr/bin", "/usr/local/bin"];
    #[cfg(target_os = "macos")]
    let search = &["/usr/local/bin", "/opt/homebrew/bin"];

    for dir in search {
        for n in &names {
            let p = Path::new(dir).join(n);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

// --- SIF generation: Thermal ---

pub fn generate_thermal_sif(
    config: &ThermalConfig,
    board_width_mm: f64,
    board_height_mm: f64,
) -> String {
    let mut sif = String::new();

    sif.push_str("Header\n  Mesh DB \".\" \"mesh\"\nEnd\n\n");

    sif.push_str("Simulation\n");
    sif.push_str("  Coordinate System = Cartesian 2D\n");
    sif.push_str("  Simulation Type = Steady State\n");
    sif.push_str("  Steady State Max Iterations = 1\n");
    sif.push_str("  Output Intervals = 1\n");
    sif.push_str("  Post File = results.vtu\n");
    sif.push_str("End\n\n");

    sif.push_str("Constants\n");
    sif.push_str(&format!("  Stefan Boltzmann = 5.67e-8\n"));
    sif.push_str("End\n\n");

    // Copper body
    sif.push_str("Body 1\n");
    sif.push_str("  Equation = 1\n");
    sif.push_str("  Material = 1\n");
    sif.push_str("End\n\n");

    // Heat equation
    sif.push_str("Equation 1\n");
    sif.push_str("  Active Solvers(1) = 1\n");
    sif.push_str("End\n\n");

    sif.push_str("Solver 1\n");
    sif.push_str("  Equation = Heat Equation\n");
    sif.push_str("  Variable = Temperature\n");
    sif.push_str("  Procedure = \"HeatSolve\" \"HeatSolver\"\n");
    sif.push_str("  Linear System Solver = Iterative\n");
    sif.push_str("  Linear System Iterative Method = BiCGStab\n");
    sif.push_str("  Linear System Max Iterations = 1000\n");
    sif.push_str("  Linear System Convergence Tolerance = 1.0e-8\n");
    sif.push_str("  Linear System Preconditioning = ILU1\n");
    sif.push_str("  Nonlinear System Max Iterations = 1\n");
    sif.push_str("  Steady State Convergence Tolerance = 1.0e-5\n");
    sif.push_str("End\n\n");

    // Copper material
    sif.push_str("Material 1\n");
    sif.push_str("  Name = \"Copper\"\n");
    sif.push_str("  Heat Conductivity = 385.0\n");  // W/(m*K)
    sif.push_str("  Density = 8960.0\n");            // kg/m^3
    sif.push_str("  Heat Capacity = 385.0\n");       // J/(kg*K)
    sif.push_str("End\n\n");

    // Boundary conditions — convection on edges
    sif.push_str("Boundary Condition 1\n");
    sif.push_str("  Target Boundaries(1) = 1\n");
    sif.push_str(&format!(
        "  Heat Transfer Coefficient = {:.2}\n",
        config.convection_coeff
    ));
    sif.push_str(&format!(
        "  External Temperature = {:.2}\n",
        config.ambient_temp
    ));
    sif.push_str("End\n\n");

    // Heat sources as body forces
    if !config.power_sources.is_empty() {
        sif.push_str("Body Force 1\n");
        // Convert power to heat generation rate (W/m^3)
        // Simplified: distribute total power over board area * copper thickness
        let total_power: f64 = config.power_sources.iter().map(|s| s.power_watts).sum();
        let area_m2 = (board_width_mm * board_height_mm) * 1e-6;
        let thickness_m = 0.035e-3; // 35um copper
        let heat_gen = total_power / (area_m2 * thickness_m);
        sif.push_str(&format!("  Heat Source = {:.2e}\n", heat_gen));
        sif.push_str("End\n\n");
    }

    sif
}

// --- SIF generation: IR Drop ---

pub fn generate_ir_drop_sif(config: &IrDropConfig) -> String {
    let mut sif = String::new();

    sif.push_str("Header\n  Mesh DB \".\" \"mesh\"\nEnd\n\n");

    sif.push_str("Simulation\n");
    sif.push_str("  Coordinate System = Cartesian 2D\n");
    sif.push_str("  Simulation Type = Steady State\n");
    sif.push_str("  Steady State Max Iterations = 1\n");
    sif.push_str("  Post File = results.vtu\n");
    sif.push_str("End\n\n");

    // Copper body
    sif.push_str("Body 1\n");
    sif.push_str("  Equation = 1\n");
    sif.push_str("  Material = 1\n");
    sif.push_str("End\n\n");

    // Static current conduction
    sif.push_str("Equation 1\n");
    sif.push_str("  Active Solvers(1) = 1\n");
    sif.push_str("End\n\n");

    sif.push_str("Solver 1\n");
    sif.push_str("  Equation = Static Current Conduction\n");
    sif.push_str("  Variable = Potential\n");
    sif.push_str("  Procedure = \"StatCurrentSolve\" \"StatCurrentSolver\"\n");
    sif.push_str("  Linear System Solver = Iterative\n");
    sif.push_str("  Linear System Iterative Method = BiCGStab\n");
    sif.push_str("  Linear System Max Iterations = 1000\n");
    sif.push_str("  Linear System Convergence Tolerance = 1.0e-8\n");
    sif.push_str("  Linear System Preconditioning = ILU1\n");
    sif.push_str("  Calculate Joule Heating = Logical True\n");
    sif.push_str("End\n\n");

    // Copper material
    sif.push_str("Material 1\n");
    sif.push_str("  Name = \"Copper\"\n");
    sif.push_str(&format!(
        "  Electric Conductivity = {:.2e}\n",
        config.copper_conductivity
    ));
    sif.push_str("End\n\n");

    // Voltage source boundaries
    for (i, pad) in config.source_pads.iter().enumerate() {
        sif.push_str(&format!("Boundary Condition {}\n", i + 1));
        sif.push_str(&format!("  Target Boundaries(1) = {}\n", i + 1));
        sif.push_str(&format!("  Potential = {:.4}\n", config.supply_voltage));
        sif.push_str("End\n\n");
    }

    // Current sink boundaries
    let bc_offset = config.source_pads.len();
    for (i, pad) in config.sink_pads.iter().enumerate() {
        sif.push_str(&format!("Boundary Condition {}\n", bc_offset + i + 1));
        sif.push_str(&format!("  Target Boundaries(1) = {}\n", bc_offset + i + 1));
        // Current density = I / A (pad area)
        let pad_area = std::f64::consts::PI * (pad.radius_mm * 1e-3).powi(2);
        let j = pad.current_a / pad_area;
        sif.push_str(&format!("  Current Density = {:.4e}\n", j));
        sif.push_str("End\n\n");
    }

    sif
}

// --- Run Elmer ---

pub fn run_elmer(
    solver_path: &Path,
    work_dir: &Path,
    progress_tx: Option<mpsc::Sender<ElmerProgress>>,
) -> Result<String, String> {
    if let Some(tx) = &progress_tx {
        let _ = tx.send(ElmerProgress {
            percent: 10.0,
            message: "Starting ElmerSolver...".to_string(),
        });
    }

    let output = Command::new(solver_path)
        .current_dir(work_dir)
        .output()
        .map_err(|e| format!("Failed to run ElmerSolver: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "ElmerSolver failed (exit {}): {}\n{}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        ));
    }

    Ok(stdout)
}

// --- VTU parser (simplified) ---

/// Parse a VTU (VTK Unstructured Grid) XML file for 2D triangular mesh + scalar field.
pub fn parse_vtu(path: &Path) -> Result<FieldResult, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read VTU file: {}", e))?;

    // Simple XML parsing — VTU is XML-based
    let mut nodes: Vec<[f64; 2]> = Vec::new();
    let mut triangles: Vec<[usize; 3]> = Vec::new();
    let mut values: Vec<f64> = Vec::new();
    let mut value_name = "Temperature".to_string();
    let mut value_unit = "C".to_string();

    // Extract Points data
    if let Some(points_data) = extract_data_between(&content, "<Points>", "</Points>") {
        if let Some(coords_str) = extract_data_between(&points_data, "<DataArray", "</DataArray>") {
            // Find the actual data after the ">" closing tag
            if let Some(pos) = coords_str.find('>') {
                let data = &coords_str[pos + 1..];
                let nums: Vec<f64> = data
                    .split_whitespace()
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
                // 3D coordinates, take x,y (skip z)
                for chunk in nums.chunks(3) {
                    if chunk.len() >= 2 {
                        nodes.push([chunk[0], chunk[1]]);
                    }
                }
            }
        }
    }

    // Extract Cells (connectivity)
    if let Some(cells_data) = extract_data_between(&content, "<Cells>", "</Cells>") {
        if let Some(conn_str) = extract_data_between(&cells_data, "<DataArray", "</DataArray>") {
            if let Some(pos) = conn_str.find('>') {
                let data = &conn_str[pos + 1..];
                let indices: Vec<usize> = data
                    .split_whitespace()
                    .filter_map(|s| s.parse::<usize>().ok())
                    .collect();
                for chunk in indices.chunks(3) {
                    if chunk.len() == 3 {
                        triangles.push([chunk[0], chunk[1], chunk[2]]);
                    }
                }
            }
        }
    }

    // Extract PointData (scalar field values)
    if let Some(pd) = extract_data_between(&content, "<PointData", "</PointData>") {
        // Get the first DataArray (the solution field)
        if let Some(da) = extract_data_between(&pd, "<DataArray", "</DataArray>") {
            // Try to get Name attribute
            if let Some(name_start) = da.find("Name=\"") {
                let rest = &da[name_start + 6..];
                if let Some(name_end) = rest.find('"') {
                    value_name = rest[..name_end].to_string();
                    if value_name.to_lowercase().contains("temperature") {
                        value_unit = "\u{00B0}C".to_string();
                    } else if value_name.to_lowercase().contains("potential") {
                        value_unit = "V".to_string();
                    }
                }
            }
            if let Some(pos) = da.find('>') {
                let data = &da[pos + 1..];
                values = data
                    .split_whitespace()
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
            }
        }
    }

    if nodes.is_empty() {
        return Err("No nodes found in VTU file".to_string());
    }

    let min_value = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_value = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    Ok(FieldResult {
        nodes,
        triangles,
        values,
        value_unit,
        value_name,
        min_value,
        max_value,
    })
}

fn extract_data_between<'a>(content: &'a str, start_tag: &str, end_tag: &str) -> Option<&'a str> {
    let start = content.find(start_tag)?;
    let end = content[start..].find(end_tag)? + start;
    Some(&content[start..end + end_tag.len()])
}
