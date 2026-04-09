use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Emitter;

use crate::engine::ngspice_ffi::{self, NgspiceInstance, NgspiceMessage};
use crate::engine::parser::SchematicSheet;
use crate::engine::spice_netlist::{self, AnalysisConfig, AnalysisType};

// --- Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverAvailability {
    pub ngspice: bool,
    pub ngspice_path: Option<String>,
    pub openems: bool,
    pub openems_path: Option<String>,
    pub elmer: bool,
    pub elmer_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformData {
    pub name: String,
    pub unit: String,
    pub real: Vec<f64>,
    pub imag: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub sim_id: String,
    pub analysis_type: String,
    pub vectors: HashMap<String, WaveformData>,
    pub elapsed_ms: u64,
    pub netlist: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimProgress {
    pub sim_id: String,
    pub percent: f64,
    pub message: String,
}

// --- Global ngspice instance ---
// ngspice is single-threaded; we hold one instance behind a Mutex.
static NGSPICE: std::sync::LazyLock<Mutex<Option<NgspiceInstance>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn ensure_ngspice() -> Result<(), String> {
    let mut guard = NGSPICE.lock().map_err(|e| format!("Lock error: {}", e))?;
    if guard.is_some() {
        return Ok(());
    }
    let path = ngspice_ffi::detect_ngspice()
        .ok_or_else(|| "ngspice shared library not found. Install ngspice or add it to PATH.".to_string())?;
    let instance = NgspiceInstance::new(&path)?;
    *guard = Some(instance);
    Ok(())
}

// --- Commands ---

#[tauri::command]
pub fn detect_solvers() -> SolverAvailability {
    let ngspice_path = ngspice_ffi::detect_ngspice();

    // Check for OpenEMS
    let openems_path = which_executable("openEMS");

    // Check for Elmer
    let elmer_path = which_executable("ElmerSolver");

    SolverAvailability {
        ngspice: ngspice_path.is_some(),
        ngspice_path: ngspice_path.map(|p| p.to_string_lossy().into_owned()),
        openems: openems_path.is_some(),
        openems_path: openems_path.map(|p| p.to_string_lossy().into_owned()),
        elmer: elmer_path.is_some(),
        elmer_path: elmer_path.map(|p| p.to_string_lossy().into_owned()),
    }
}

#[tauri::command]
pub fn get_spice_netlist(
    data: SchematicSheet,
    config: AnalysisConfig,
) -> Result<String, String> {
    spice_netlist::generate_spice_netlist(&data, &config)
}

#[tauri::command]
pub async fn run_spice_simulation(
    app: tauri::AppHandle,
    sim_id: String,
    data: SchematicSheet,
    config: AnalysisConfig,
) -> Result<SimulationResult, String> {
    let start = std::time::Instant::now();

    // Generate netlist
    let netlist = spice_netlist::generate_spice_netlist(&data, &config)?;

    // Emit progress
    let _ = app.emit(
        "sim:progress",
        SimProgress {
            sim_id: sim_id.clone(),
            percent: 10.0,
            message: "Netlist generated".to_string(),
        },
    );

    // Initialize ngspice if needed
    ensure_ngspice()?;

    let _ = app.emit(
        "sim:progress",
        SimProgress {
            sim_id: sim_id.clone(),
            percent: 20.0,
            message: "ngspice initialized".to_string(),
        },
    );

    // Run simulation in blocking task (ngspice is not async)
    let netlist_clone = netlist.clone();
    let sim_id_clone = sim_id.clone();
    let app_clone = app.clone();
    let analysis_type_str = format!("{:?}", config.analysis_type);

    let result = tokio::task::spawn_blocking(move || {
        let guard = NGSPICE.lock().map_err(|e| format!("Lock error: {}", e))?;
        let ng = guard
            .as_ref()
            .ok_or_else(|| "ngspice not initialized".to_string())?;

        // Reset previous state
        let _ = ng.reset();

        // Load circuit from netlist lines
        let lines: Vec<&str> = netlist_clone.lines().collect();
        ng.load_circuit(&lines)?;

        let _ = app_clone.emit(
            "sim:progress",
            SimProgress {
                sim_id: sim_id_clone.clone(),
                percent: 40.0,
                message: "Circuit loaded, running simulation...".to_string(),
            },
        );

        // Run the simulation
        ng.command("run")?;

        // Drain callback messages (progress updates)
        while let Ok(msg) = ng.rx.try_recv() {
            match msg {
                NgspiceMessage::Status(s) => {
                    // Parse percent from status string like "--ready"
                    let _ = app_clone.emit(
                        "sim:progress",
                        SimProgress {
                            sim_id: sim_id_clone.clone(),
                            percent: 60.0,
                            message: s,
                        },
                    );
                }
                NgspiceMessage::Output(s) => {
                    // Log output for debugging
                    let _ = app_clone.emit(
                        "sim:progress",
                        SimProgress {
                            sim_id: sim_id_clone.clone(),
                            percent: 60.0,
                            message: s,
                        },
                    );
                }
                _ => {}
            }
        }

        let _ = app_clone.emit(
            "sim:progress",
            SimProgress {
                sim_id: sim_id_clone.clone(),
                percent: 80.0,
                message: "Simulation complete, extracting results...".to_string(),
            },
        );

        // Get current plot and all vectors
        let plot_name = ng
            .current_plot()
            .ok_or_else(|| "No current plot after simulation".to_string())?;
        let vec_names = ng.all_vectors(&plot_name);

        let mut vectors: HashMap<String, WaveformData> = HashMap::new();

        for name in &vec_names {
            let full_name = format!("{}.{}", plot_name, name);
            match ng.get_vector(&full_name) {
                Ok((real, imag)) => {
                    let unit = guess_unit(name);
                    vectors.insert(
                        name.clone(),
                        WaveformData {
                            name: name.clone(),
                            unit,
                            real,
                            imag,
                        },
                    );
                }
                Err(_) => {
                    // Try without plot prefix
                    if let Ok((real, imag)) = ng.get_vector(name) {
                        let unit = guess_unit(name);
                        vectors.insert(
                            name.clone(),
                            WaveformData {
                                name: name.clone(),
                                unit,
                                real,
                                imag,
                            },
                        );
                    }
                }
            }
        }

        Ok::<HashMap<String, WaveformData>, String>(vectors)
    })
    .await
    .map_err(|e| format!("Simulation task failed: {}", e))??;

    let elapsed = start.elapsed().as_millis() as u64;

    let sim_result = SimulationResult {
        sim_id: sim_id.clone(),
        analysis_type: analysis_type_str,
        vectors: result,
        elapsed_ms: elapsed,
        netlist,
    };

    let _ = app.emit(
        "sim:progress",
        SimProgress {
            sim_id,
            percent: 100.0,
            message: format!("Done in {}ms", elapsed),
        },
    );

    Ok(sim_result)
}

// --- Helpers ---

fn guess_unit(vec_name: &str) -> String {
    let name_lower = vec_name.to_lowercase();
    if name_lower.starts_with("v(") || name_lower.starts_with("v_") {
        "V".to_string()
    } else if name_lower.starts_with("i(") || name_lower.starts_with("i_") {
        "A".to_string()
    } else if name_lower == "time" {
        "s".to_string()
    } else if name_lower == "frequency" {
        "Hz".to_string()
    } else {
        String::new()
    }
}

/// Search PATH for an executable.
fn which_executable(name: &str) -> Option<std::path::PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(sep) {
            let candidate = std::path::Path::new(dir).join(name);
            if candidate.exists() {
                return Some(candidate);
            }
            // Windows: also check with .exe extension
            #[cfg(windows)]
            {
                let with_ext = candidate.with_extension("exe");
                if with_ext.exists() {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}
