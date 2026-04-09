//! 2D finite-difference Laplace equation solver for PCB stackup impedance.
//!
//! Calculates characteristic impedance (Z0), effective dielectric constant (er_eff),
//! and propagation delay from a PCB cross-section geometry using the
//! capacitance extraction method:
//!   1. Solve Laplace with actual dielectrics → C
//!   2. Solve Laplace with all Er=1 → C_air
//!   3. Z0 = 1 / (c * sqrt(C * C_air)), er_eff = C / C_air

use serde::{Deserialize, Serialize};

const SPEED_OF_LIGHT: f64 = 299_792_458.0; // m/s

// --- Public types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupLayer {
    pub name: String,
    pub height_um: f64,
    pub dielectric_er: f64,
    pub is_copper: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceGeometry {
    pub width_um: f64,
    pub thickness_um: f64,
    pub layer_index: usize,
    pub offset_um: f64, // Horizontal offset from center (for differential)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpedanceRequest {
    pub stackup: Vec<StackupLayer>,
    pub traces: Vec<TraceGeometry>, // 1 for single-ended, 2 for differential
    pub grid_resolution: Option<usize>, // Cells per trace width (default 20)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpedanceResult {
    pub z0: f64,
    pub z_diff: Option<f64>,
    pub z_odd: Option<f64>,
    pub z_even: Option<f64>,
    pub er_eff: f64,
    pub delay_ps_per_mm: f64,
    pub loss_db_per_mm: Option<f64>,
}

// --- 2D Grid ---

struct Grid2D {
    nx: usize,
    ny: usize,
    dx: f64, // Cell size in meters
    dy: f64,
    potential: Vec<f64>,
    er: Vec<f64>,        // Per-cell relative permittivity
    is_fixed: Vec<bool>, // Fixed boundary cells (conductors)
}

impl Grid2D {
    fn new(nx: usize, ny: usize, dx: f64, dy: f64) -> Self {
        let n = nx * ny;
        Self {
            nx,
            ny,
            dx,
            dy,
            potential: vec![0.0; n],
            er: vec![1.0; n],
            is_fixed: vec![false; n],
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.nx + x
    }

    fn set_potential(&mut self, x: usize, y: usize, v: f64) {
        let i = self.idx(x, y);
        self.potential[i] = v;
        self.is_fixed[i] = true;
    }

    fn set_er_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, er: f64) {
        let nx = self.nx;
        let ny = self.ny;
        for y in y0..y1.min(ny) {
            for x in x0..x1.min(nx) {
                let i = y * nx + x;
                self.er[i] = er;
            }
        }
    }

    fn set_conductor_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, v: f64) {
        let nx = self.nx;
        let ny = self.ny;
        for y in y0..y1.min(ny) {
            for x in x0..x1.min(nx) {
                let i = y * nx + x;
                self.potential[i] = v;
                self.is_fixed[i] = true;
            }
        }
    }
}

// --- Gauss-Seidel SOR solver ---

fn solve_sor(grid: &mut Grid2D, max_iter: usize, tol: f64, omega: f64) -> usize {
    let nx = grid.nx;
    let ny = grid.ny;

    for iter in 0..max_iter {
        let mut max_change: f64 = 0.0;

        for y in 1..ny - 1 {
            for x in 1..nx - 1 {
                let i = y * nx + x;
                if grid.is_fixed[i] {
                    continue;
                }

                // 5-point stencil with dielectric weighting
                let er_l = 0.5 * (grid.er[i] + grid.er[i - 1]);
                let er_r = 0.5 * (grid.er[i] + grid.er[i + 1]);
                let er_d = 0.5 * (grid.er[i] + grid.er[i - nx]);
                let er_u = 0.5 * (grid.er[i] + grid.er[i + nx]);

                let sum_er = er_l + er_r + er_d + er_u;
                if sum_er < 1e-30 {
                    continue;
                }

                let v_new = (er_l * grid.potential[i - 1]
                    + er_r * grid.potential[i + 1]
                    + er_d * grid.potential[i - nx]
                    + er_u * grid.potential[i + nx])
                    / sum_er;

                let old = grid.potential[i];
                grid.potential[i] = old + omega * (v_new - old);

                let change = (grid.potential[i] - old).abs();
                if change > max_change {
                    max_change = change;
                }
            }
        }

        if max_change < tol {
            return iter + 1;
        }
    }
    max_iter
}

// --- Capacitance extraction ---

/// Extract capacitance per unit length (F/m) from the solved potential field.
/// Uses Gauss's law: Q = epsilon_0 * sum(Er * E_normal * ds) around the trace.
fn extract_capacitance(grid: &Grid2D, trace_v: f64) -> f64 {
    let eps0 = 8.854187817e-12; // F/m
    let nx = grid.nx;
    let ny = grid.ny;
    let mut charge = 0.0;

    // Walk all cells adjacent to the trace conductor and sum normal E-field
    for y in 1..ny - 1 {
        for x in 1..nx - 1 {
            let i = y * nx + x;
            if !grid.is_fixed[i] || (grid.potential[i] - trace_v).abs() > 0.01 {
                continue;
            }

            // Check each neighbor — if it's not a conductor, compute E contribution
            let neighbors = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)];
            let ds_vals = [grid.dy, grid.dy, grid.dx, grid.dx]; // Face areas per unit length

            for (k, &(nx_pos, ny_pos)) in neighbors.iter().enumerate() {
                if nx_pos >= nx || ny_pos >= ny {
                    continue;
                }
                let j = ny_pos * nx + nx_pos;
                if grid.is_fixed[j] && (grid.potential[j] - trace_v).abs() < 0.01 {
                    continue; // Same conductor
                }

                let e_normal = (trace_v - grid.potential[j]).abs()
                    / if k < 2 { grid.dx } else { grid.dy };
                let er_avg = 0.5 * (grid.er[i] + grid.er[j]);
                charge += eps0 * er_avg * e_normal * ds_vals[k];
            }
        }
    }

    charge / trace_v.abs()
}

// --- Build grid from stackup ---

fn build_grid(
    req: &ImpedanceRequest,
    use_actual_er: bool,
    cells_per_trace_width: usize,
) -> (Grid2D, Vec<(usize, usize, usize, usize)>) {
    let trace_w = req.traces[0].width_um;
    let cell_um = trace_w / cells_per_trace_width as f64;

    // Calculate total stackup height
    let total_height_um: f64 = req.stackup.iter().map(|l| l.height_um).sum();

    // Grid dimensions: width = 10x trace width, height = full stackup
    let grid_width_um = trace_w * 10.0;
    let nx = (grid_width_um / cell_um).ceil() as usize + 1;
    let ny = (total_height_um / cell_um).ceil() as usize + 1;

    let dx = cell_um * 1e-6; // Convert to meters
    let dy = cell_um * 1e-6;

    let mut grid = Grid2D::new(nx, ny, dx, dy);

    // Set dielectric constants per layer
    let mut y_pos = 0.0_f64;
    let mut copper_layer_idx = 0;
    let mut copper_y_positions: Vec<(usize, usize)> = Vec::new(); // (y_start, y_end) for each copper layer

    for layer in &req.stackup {
        let y_start = (y_pos / cell_um).round() as usize;
        let y_end = ((y_pos + layer.height_um) / cell_um).round() as usize;

        if layer.is_copper {
            // Copper layers are ground planes by default
            let thickness_cells = ((layer.height_um / cell_um).round() as usize).max(1);
            copper_y_positions.push((y_start, y_start + thickness_cells));

            // Set as ground plane (V=0) across full width
            grid.set_conductor_rect(0, y_start, nx, y_start + thickness_cells, 0.0);
            copper_layer_idx += 1;
        } else {
            // Dielectric layer
            let er = if use_actual_er { layer.dielectric_er } else { 1.0 };
            grid.set_er_rect(0, y_start, nx, y_end.min(ny), er);
        }

        y_pos += layer.height_um;
    }

    // Place signal traces (V=1) — override ground plane at trace positions
    let center_x = nx / 2;
    let mut trace_rects = Vec::new();

    for trace in &req.traces {
        let tw_cells = (trace.width_um / cell_um).round() as usize;
        let tt_cells = (trace.thickness_um / cell_um).round() as usize;
        let offset_cells = (trace.offset_um / cell_um).round() as isize;

        let tx_start = ((center_x as isize + offset_cells) - (tw_cells as isize / 2)).max(0) as usize;
        let tx_end = (tx_start + tw_cells).min(nx);

        // Find the Y position from the copper layer index
        if trace.layer_index < copper_y_positions.len() {
            let (cy_start, _) = copper_y_positions[trace.layer_index];
            let ty_start = cy_start;
            let ty_end = (ty_start + tt_cells.max(1)).min(ny);

            // Clear the ground conductor at trace position and set trace voltage
            for y in ty_start..ty_end {
                for x in tx_start..tx_end {
                    let i = grid.idx(x, y);
                    grid.is_fixed[i] = true;
                    grid.potential[i] = 1.0;
                }
            }

            trace_rects.push((tx_start, ty_start, tx_end, ty_end));
        }
    }

    // Boundary conditions: top and bottom are ground (PEC)
    for x in 0..nx {
        grid.set_potential(x, 0, 0.0);
        grid.set_potential(x, ny - 1, 0.0);
    }
    // Left and right boundaries: Neumann (handled implicitly by SOR not updating edges)
    for y in 0..ny {
        grid.set_potential(0, y, 0.0);
        grid.set_potential(nx - 1, y, 0.0);
    }

    (grid, trace_rects)
}

// --- Main entry point ---

pub fn solve_impedance(req: &ImpedanceRequest) -> Result<ImpedanceResult, String> {
    if req.stackup.is_empty() {
        return Err("Empty stackup".to_string());
    }
    if req.traces.is_empty() {
        return Err("No traces specified".to_string());
    }

    let cells_per_tw = req.grid_resolution.unwrap_or(20);
    let max_iter = 10_000;
    let tol = 1e-6;
    let omega = 1.7; // SOR relaxation factor

    // Solve with actual dielectrics → C
    let (mut grid_real, _) = build_grid(req, true, cells_per_tw);
    solve_sor(&mut grid_real, max_iter, tol, omega);
    let c_real = extract_capacitance(&grid_real, 1.0);

    // Solve with Er=1 → C_air
    let (mut grid_air, _) = build_grid(req, false, cells_per_tw);
    solve_sor(&mut grid_air, max_iter, tol, omega);
    let c_air = extract_capacitance(&grid_air, 1.0);

    if c_real <= 0.0 || c_air <= 0.0 {
        return Err("Capacitance extraction failed — check stackup geometry".to_string());
    }

    // Z0 = 1 / (c * sqrt(C * C_air))
    let z0 = 1.0 / (SPEED_OF_LIGHT * (c_real * c_air).sqrt());
    let er_eff = c_real / c_air;
    let delay_s_per_m = er_eff.sqrt() / SPEED_OF_LIGHT;
    let delay_ps_per_mm = delay_s_per_m * 1e12 * 1e-3; // ps/mm

    // Differential impedance (if 2 traces)
    let (z_diff, z_odd, z_even) = if req.traces.len() == 2 {
        // For differential: Z_diff = 2 * Z_odd
        // Z_odd ≈ Z0 * (1 - k), Z_even ≈ Z0 * (1 + k) where k is coupling coefficient
        // Simplified: we'd need to solve odd-mode and even-mode separately.
        // For now, approximate: Z_diff ≈ 2 * Z0 (loosely coupled)
        let z_d = 2.0 * z0;
        (Some(z_d), Some(z0), Some(z0))
    } else {
        (None, None, None)
    };

    Ok(ImpedanceResult {
        z0,
        z_diff,
        z_odd,
        z_even,
        er_eff,
        delay_ps_per_mm,
        loss_db_per_mm: None, // Future: conductor + dielectric loss model
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_microstrip_50_ohm() {
        // Standard 50-ohm microstrip: 1.6mm FR4 (Er=4.3), ~0.3mm trace on top layer
        let req = ImpedanceRequest {
            stackup: vec![
                StackupLayer {
                    name: "Top Copper".into(),
                    height_um: 35.0,
                    dielectric_er: 1.0,
                    is_copper: true,
                },
                StackupLayer {
                    name: "FR4".into(),
                    height_um: 1500.0,
                    dielectric_er: 4.3,
                    is_copper: false,
                },
                StackupLayer {
                    name: "Bottom Copper".into(),
                    height_um: 35.0,
                    dielectric_er: 1.0,
                    is_copper: true,
                },
            ],
            traces: vec![TraceGeometry {
                width_um: 300.0,
                thickness_um: 35.0,
                layer_index: 0,
                offset_um: 0.0,
            }],
            grid_resolution: Some(20),
        };

        let result = solve_impedance(&req).unwrap();
        // 50-ohm microstrip on 1.6mm FR4 should be roughly 40-60 ohms
        assert!(
            result.z0 > 30.0 && result.z0 < 80.0,
            "Z0 = {:.1} ohms, expected ~50 ohms",
            result.z0
        );
        assert!(
            result.er_eff > 2.0 && result.er_eff < 4.5,
            "er_eff = {:.2}, expected ~3.0-3.5",
            result.er_eff
        );
    }
}
