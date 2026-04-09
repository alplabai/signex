import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// --- Types ---

export interface SolverAvailability {
  ngspice: boolean;
  ngspice_path: string | null;
  openems: boolean;
  openems_path: string | null;
  elmer: boolean;
  elmer_path: string | null;
}

export interface WaveformData {
  name: string;
  unit: string;
  real: number[];
  imag: number[] | null;
}

export interface SimulationResult {
  sim_id: string;
  analysis_type: string;
  vectors: Record<string, WaveformData>;
  elapsed_ms: number;
  netlist: string;
}

export interface SimProgress {
  sim_id: string;
  percent: number;
  message: string;
}

export type AnalysisType = "DcOp" | "DcSweep" | "Ac" | "Transient" | "Noise";

export interface AnalysisConfig {
  analysis_type: AnalysisType;
  params: Record<string, string>;
}

// --- Store ---

interface SimulationState {
  // Solver availability
  solvers: SolverAvailability;
  solversDetected: boolean;

  // Active simulation
  isRunning: boolean;
  progress: number;
  progressMessage: string;

  // Results
  results: Record<string, SimulationResult>;
  activeResultId: string | null;

  // Analysis config
  analysisType: AnalysisType;
  analysisParams: Record<string, string>;

  // Waveform viewer
  selectedVectors: string[];
  cursorX: number | null;

  // Netlist preview
  netlistPreview: string | null;

  // Actions
  detectSolvers: () => Promise<void>;
  setAnalysisType: (type_: AnalysisType) => void;
  setAnalysisParam: (key: string, value: string) => void;
  runSimulation: (data: unknown) => Promise<void>;
  previewNetlist: (data: unknown) => Promise<void>;
  selectResult: (simId: string) => void;
  toggleVector: (name: string) => void;
  setCursorX: (x: number | null) => void;
  clearResults: () => void;
}

// Module-level listener cleanup
let progressUnlisten: UnlistenFn | null = null;

export const useSimulationStore = create<SimulationState>((set, get) => ({
  solvers: {
    ngspice: false, ngspice_path: null,
    openems: false, openems_path: null,
    elmer: false, elmer_path: null,
  },
  solversDetected: false,
  isRunning: false,
  progress: 0,
  progressMessage: "",
  results: {},
  activeResultId: null,
  analysisType: "DcOp",
  analysisParams: {},
  selectedVectors: [],
  cursorX: null,
  netlistPreview: null,

  detectSolvers: async () => {
    try {
      const solvers = await invoke<SolverAvailability>("detect_solvers");
      set({ solvers, solversDetected: true });
    } catch (e) {
      console.error("Failed to detect solvers:", e);
    }
  },

  setAnalysisType: (type_) => set({ analysisType: type_, analysisParams: {} }),
  setAnalysisParam: (key, value) =>
    set((s) => ({ analysisParams: { ...s.analysisParams, [key]: value } })),

  runSimulation: async (data) => {
    const { analysisType, analysisParams } = get();
    const simId = `sim_${Date.now()}`;

    set({ isRunning: true, progress: 0, progressMessage: "Starting..." });

    // Listen for progress events
    if (progressUnlisten) progressUnlisten();
    progressUnlisten = await listen<SimProgress>("sim:progress", (event) => {
      set({
        progress: event.payload.percent,
        progressMessage: event.payload.message,
      });
    });

    try {
      const result = await invoke<SimulationResult>("run_spice_simulation", {
        simId,
        data,
        config: { analysis_type: analysisType, params: analysisParams },
      });

      set((s) => ({
        results: { ...s.results, [result.sim_id]: result },
        activeResultId: result.sim_id,
        selectedVectors: Object.keys(result.vectors).filter(
          (n) => n.toLowerCase() !== "time" && n.toLowerCase() !== "frequency"
        ),
        isRunning: false,
        progress: 100,
        progressMessage: `Done in ${result.elapsed_ms}ms`,
      }));
    } catch (e) {
      set({
        isRunning: false,
        progress: 0,
        progressMessage: `Error: ${e}`,
      });
    } finally {
      if (progressUnlisten) {
        progressUnlisten();
        progressUnlisten = null;
      }
    }
  },

  previewNetlist: async (data) => {
    const { analysisType, analysisParams } = get();
    try {
      const netlist = await invoke<string>("get_spice_netlist", {
        data,
        config: { analysis_type: analysisType, params: analysisParams },
      });
      set({ netlistPreview: netlist });
    } catch (e) {
      set({ netlistPreview: `Error: ${e}` });
    }
  },

  selectResult: (simId) => {
    const result = get().results[simId];
    if (result) {
      set({
        activeResultId: simId,
        selectedVectors: Object.keys(result.vectors).filter(
          (n) => n.toLowerCase() !== "time" && n.toLowerCase() !== "frequency"
        ),
      });
    }
  },

  toggleVector: (name) =>
    set((s) => ({
      selectedVectors: s.selectedVectors.includes(name)
        ? s.selectedVectors.filter((n) => n !== name)
        : [...s.selectedVectors, name],
    })),

  setCursorX: (x) => set({ cursorX: x }),

  clearResults: () => set({ results: {}, activeResultId: null, selectedVectors: [] }),
}));
