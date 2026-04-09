//! OpenEMS bridge — CSXCAD XML generator and subprocess runner.
//!
//! Generates CSXCAD geometry from PCB data, runs OpenEMS as subprocess,
//! parses Touchstone S-parameter results.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

use super::touchstone::{self, SParameterData};

// --- Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmConfig {
    pub freq_start: f64,
    pub freq_stop: f64,
    pub port_pads: Vec<(String, String)>, // (footprint_uuid, pad_number)
    pub mesh_resolution_mm: f64,
    pub boundary: String,    // "PML" | "PEC" | "PMC"
    pub excitation: String,  // "gaussian" | "sinusoidal"
    pub num_timesteps: Option<u64>,
    pub end_criteria: Option<f64>, // Energy convergence criteria (default 1e-5)
}

impl Default for EmConfig {
    fn default() -> Self {
        Self {
            freq_start: 1e6,
            freq_stop: 10e9,
            port_pads: vec![],
            mesh_resolution_mm: 0.2,
            boundary: "PML".to_string(),
            excitation: "gaussian".to_string(),
            num_timesteps: None,
            end_criteria: Some(1e-5),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmResult {
    pub s_params: Option<SParameterData>,
    pub elapsed_ms: u64,
    pub log_output: String,
}

/// Progress message from OpenEMS subprocess.
#[derive(Debug, Clone)]
pub struct EmProgress {
    pub percent: f64,
    pub message: String,
}

// --- Detect OpenEMS ---

pub fn detect_openems() -> Option<PathBuf> {
    let names = if cfg!(windows) {
        vec!["openEMS.exe", "openEMS"]
    } else {
        vec!["openEMS"]
    };

    // Search PATH
    if let Ok(path_var) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(sep) {
            for name in &names {
                let candidate = Path::new(dir).join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    // Common install locations
    #[cfg(target_os = "windows")]
    let search_dirs = &[
        r"C:\openEMS\bin",
        r"C:\Program Files\openEMS\bin",
    ];
    #[cfg(target_os = "linux")]
    let search_dirs = &["/usr/bin", "/usr/local/bin", "/opt/openEMS/bin"];
    #[cfg(target_os = "macos")]
    let search_dirs = &["/usr/local/bin", "/opt/homebrew/bin"];

    for dir in search_dirs {
        for name in &names {
            let path = Path::new(dir).join(name);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

// --- CSXCAD XML generation ---

/// Minimal PCB structure for EM simulation (subset of full PcbBoard).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmPcbGeometry {
    pub board_width_mm: f64,
    pub board_height_mm: f64,
    pub stackup: Vec<EmStackupLayer>,
    pub traces: Vec<EmTrace>,
    pub vias: Vec<EmVia>,
    pub ports: Vec<EmPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmStackupLayer {
    pub name: String,
    pub z_mm: f64,          // Z position (bottom of layer)
    pub thickness_mm: f64,
    pub is_copper: bool,
    pub dielectric_er: f64,
    pub loss_tangent: f64,  // tan(delta) for dielectric loss
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmTrace {
    pub x1_mm: f64,
    pub y1_mm: f64,
    pub x2_mm: f64,
    pub y2_mm: f64,
    pub width_mm: f64,
    pub layer_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmVia {
    pub x_mm: f64,
    pub y_mm: f64,
    pub drill_mm: f64,
    pub annular_ring_mm: f64,
    pub start_layer: usize,
    pub end_layer: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmPort {
    pub x_mm: f64,
    pub y_mm: f64,
    pub layer_index: usize,
    pub impedance: f64,     // Reference impedance (default 50)
    pub port_number: usize,
}

/// Generate CSXCAD XML for OpenEMS from PCB geometry.
pub fn generate_csxcad_xml(geom: &EmPcbGeometry, config: &EmConfig) -> Result<String, String> {
    let mut xml = String::new();

    // Header
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<openEMS>\n");

    // FDTD settings
    let end_crit = config.end_criteria.unwrap_or(1e-5);
    let max_ts = config.num_timesteps.unwrap_or(100_000);
    xml.push_str(&format!(
        "  <FDTD NumberOfTimesteps=\"{}\" endCriteria=\"{:.0e}\" f_max=\"{:.0e}\">\n",
        max_ts, end_crit, config.freq_stop
    ));

    // Excitation
    let f0 = (config.freq_start + config.freq_stop) / 2.0;
    let fc = (config.freq_stop - config.freq_start) / 2.0;
    xml.push_str(&format!(
        "    <Excitation Type=\"0\" f0=\"{:.0e}\" fc=\"{:.0e}\"/>\n",
        f0, fc
    ));

    // Boundary conditions
    let bc = match config.boundary.as_str() {
        "PEC" => "0,0,0,0,0,0",
        "PMC" => "1,1,1,1,1,1",
        _ => "3,3,3,3,3,3", // PML (MUR absorbing)
    };
    xml.push_str(&format!(
        "    <BoundaryCond xmin=\"{}\" xmax=\"{}\" ymin=\"{}\" ymax=\"{}\" zmin=\"{}\" zmax=\"{}\"/>\n",
        bc.split(',').nth(0).unwrap_or("3"),
        bc.split(',').nth(1).unwrap_or("3"),
        bc.split(',').nth(2).unwrap_or("3"),
        bc.split(',').nth(3).unwrap_or("3"),
        bc.split(',').nth(4).unwrap_or("3"),
        bc.split(',').nth(5).unwrap_or("3"),
    ));
    xml.push_str("  </FDTD>\n\n");

    // ContinuousStructure
    xml.push_str("  <ContinuousStructure CoordSystem=\"0\">\n");

    // Properties section
    xml.push_str("    <Properties>\n");

    // Copper material
    xml.push_str("      <Metal Name=\"copper\">\n");

    // Copper ground planes
    for (li, layer) in geom.stackup.iter().enumerate() {
        if !layer.is_copper {
            continue;
        }
        let z0 = layer.z_mm;
        let z1 = z0 + layer.thickness_mm;
        // Full plane (will be cut by trace geometry if needed)
        xml.push_str(&format!(
            "        <Primitives>\n          <Box Priority=\"1\">\n            <P1 X=\"0\" Y=\"0\" Z=\"{:.4}\"/>\n            <P2 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n          </Box>\n        </Primitives>\n",
            z0, geom.board_width_mm, geom.board_height_mm, z1
        ));
    }

    // Signal traces (higher priority overrides ground)
    for trace in &geom.traces {
        if trace.layer_index >= geom.stackup.len() {
            continue;
        }
        let layer = &geom.stackup[trace.layer_index];
        let z0 = layer.z_mm;
        let z1 = z0 + layer.thickness_mm;
        let hw = trace.width_mm / 2.0;

        // Trace as a box with width
        let (dx, dy) = (trace.x2_mm - trace.x1_mm, trace.y2_mm - trace.y1_mm);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            continue;
        }
        let nx = -dy / len;
        let ny = dx / len;

        let x_min = trace.x1_mm.min(trace.x2_mm) - hw * nx.abs();
        let x_max = trace.x1_mm.max(trace.x2_mm) + hw * nx.abs();
        let y_min = trace.y1_mm.min(trace.y2_mm) - hw * ny.abs();
        let y_max = trace.y1_mm.max(trace.y2_mm) + hw * ny.abs();

        xml.push_str(&format!(
            "        <Primitives>\n          <Box Priority=\"10\">\n            <P1 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n            <P2 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n          </Box>\n        </Primitives>\n",
            x_min, y_min, z0, x_max, y_max, z1
        ));
    }

    xml.push_str("      </Metal>\n");

    // Dielectric materials
    for (li, layer) in geom.stackup.iter().enumerate() {
        if layer.is_copper {
            continue;
        }
        let z0 = layer.z_mm;
        let z1 = z0 + layer.thickness_mm;
        xml.push_str(&format!(
            "      <Material Name=\"dielectric_{}\">\n        <Property Epsilon=\"{:.2}\" Mue=\"1\" Kappa=\"0\" Sigma=\"0\"/>\n",
            li, layer.dielectric_er
        ));
        if layer.loss_tangent > 0.0 {
            // Model dielectric loss as conductivity: sigma = 2*pi*f*eps0*er*tan_d
            // Use center frequency for approximation
            let f_center = (config.freq_start + config.freq_stop) / 2.0;
            let eps0 = 8.854e-12;
            let sigma = 2.0 * std::f64::consts::PI * f_center * eps0 * layer.dielectric_er * layer.loss_tangent;
            xml.push_str(&format!(
                "        <Property Kappa=\"{:.4e}\"/>\n",
                sigma
            ));
        }
        xml.push_str(&format!(
            "        <Primitives>\n          <Box Priority=\"0\">\n            <P1 X=\"0\" Y=\"0\" Z=\"{:.4}\"/>\n            <P2 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n          </Box>\n        </Primitives>\n",
            z0, geom.board_width_mm, geom.board_height_mm, z1
        ));
        xml.push_str("      </Material>\n");
    }

    // Vias as metal cylinders
    if !geom.vias.is_empty() {
        xml.push_str("      <Metal Name=\"via_copper\">\n");
        for via in &geom.vias {
            let z_start = if via.start_layer < geom.stackup.len() {
                geom.stackup[via.start_layer].z_mm
            } else {
                0.0
            };
            let z_end = if via.end_layer < geom.stackup.len() {
                geom.stackup[via.end_layer].z_mm + geom.stackup[via.end_layer].thickness_mm
            } else {
                geom.stackup.last().map(|l| l.z_mm + l.thickness_mm).unwrap_or(1.6)
            };
            let radius = via.drill_mm / 2.0;
            xml.push_str(&format!(
                "        <Primitives>\n          <Cylinder Priority=\"20\" Radius=\"{:.4}\">\n            <P1 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n            <P2 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n          </Cylinder>\n        </Primitives>\n",
                radius, via.x_mm, via.y_mm, z_start, via.x_mm, via.y_mm, z_end
            ));
        }
        xml.push_str("      </Metal>\n");
    }

    // Excitation ports
    for port in &geom.ports {
        if port.layer_index >= geom.stackup.len() {
            continue;
        }
        let layer = &geom.stackup[port.layer_index];
        let z0 = layer.z_mm;
        // Find nearest ground layer below
        let z_gnd = geom.stackup.iter()
            .filter(|l| l.is_copper && l.z_mm < z0)
            .map(|l| l.z_mm + l.thickness_mm)
            .last()
            .unwrap_or(0.0);

        xml.push_str(&format!(
            "      <LumpedPort Name=\"port_{}\" R=\"{:.1}\" Direction=\"2\">\n        <Primitives>\n          <Box Priority=\"50\">\n            <P1 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n            <P2 X=\"{:.4}\" Y=\"{:.4}\" Z=\"{:.4}\"/>\n          </Box>\n        </Primitives>\n      </LumpedPort>\n",
            port.port_number, port.impedance,
            port.x_mm - 0.1, port.y_mm - 0.1, z_gnd,
            port.x_mm + 0.1, port.y_mm + 0.1, z0
        ));
    }

    xml.push_str("    </Properties>\n");

    // RectilinearGrid — auto-mesh based on geometry
    let res = config.mesh_resolution_mm;
    let nx = (geom.board_width_mm / res).ceil() as usize + 1;
    let ny = (geom.board_height_mm / res).ceil() as usize + 1;

    let x_lines: Vec<String> = (0..nx)
        .map(|i| format!("{:.4}", i as f64 * res))
        .collect();
    let y_lines: Vec<String> = (0..ny)
        .map(|i| format!("{:.4}", i as f64 * res))
        .collect();

    // Z mesh from stackup boundaries
    let mut z_set: Vec<f64> = Vec::new();
    z_set.push(0.0);
    for layer in &geom.stackup {
        z_set.push(layer.z_mm);
        z_set.push(layer.z_mm + layer.thickness_mm);
    }
    z_set.sort_by(|a, b| a.partial_cmp(b).unwrap());
    z_set.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let z_lines: Vec<String> = z_set.iter().map(|z| format!("{:.4}", z)).collect();

    xml.push_str("    <RectilinearGrid DeltaUnit=\"1e-3\">\n");
    xml.push_str(&format!("      <XLines>{}</XLines>\n", x_lines.join(",")));
    xml.push_str(&format!("      <YLines>{}</YLines>\n", y_lines.join(",")));
    xml.push_str(&format!("      <ZLines>{}</ZLines>\n", z_lines.join(",")));
    xml.push_str("    </RectilinearGrid>\n");

    xml.push_str("  </ContinuousStructure>\n");
    xml.push_str("</openEMS>\n");

    Ok(xml)
}

// --- Run OpenEMS ---

/// Run OpenEMS as a subprocess.
pub fn run_openems(
    openems_path: &Path,
    xml_path: &Path,
    output_dir: &Path,
    progress_tx: Option<mpsc::Sender<EmProgress>>,
) -> Result<EmResult, String> {
    let start = std::time::Instant::now();

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    let output = Command::new(openems_path)
        .arg(xml_path.to_string_lossy().as_ref())
        .arg("--engine=fastest")
        .arg(format!("--dump-path={}", output_dir.to_string_lossy()))
        .output()
        .map_err(|e| format!("Failed to run OpenEMS: {}", e))?;

    let log_output = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "OpenEMS failed (exit {}): {}\n{}",
            output.status.code().unwrap_or(-1),
            log_output,
            stderr
        ));
    }

    if let Some(tx) = &progress_tx {
        let _ = tx.send(EmProgress {
            percent: 90.0,
            message: "Parsing results...".to_string(),
        });
    }

    // Look for Touchstone output
    let s_params = find_and_parse_touchstone(output_dir);

    let elapsed = start.elapsed().as_millis() as u64;

    Ok(EmResult {
        s_params,
        elapsed_ms: elapsed,
        log_output,
    })
}

/// Search output directory for .s*p files and parse the first one found.
fn find_and_parse_touchstone(dir: &Path) -> Option<SParameterData> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ext_str.starts_with('s') && ext_str.ends_with('p') {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(data) = touchstone::parse_touchstone(&content) {
                            return Some(data);
                        }
                    }
                }
            }
        }
    }
    None
}
