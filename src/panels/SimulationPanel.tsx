import { useEffect } from "react";
import { useSimulationStore, type AnalysisType } from "@/stores/simulation";
import { useSchematicStore } from "@/stores/schematic";

const ANALYSIS_TYPES: { value: AnalysisType; label: string }[] = [
  { value: "DcOp", label: "DC Operating Point" },
  { value: "DcSweep", label: "DC Sweep" },
  { value: "Ac", label: "AC Analysis" },
  { value: "Transient", label: "Transient" },
  { value: "Noise", label: "Noise" },
];

// Parameter fields per analysis type
const PARAM_FIELDS: Record<AnalysisType, { key: string; label: string; placeholder: string }[]> = {
  DcOp: [],
  DcSweep: [
    { key: "source", label: "Source", placeholder: "V1" },
    { key: "start", label: "Start", placeholder: "0" },
    { key: "stop", label: "Stop", placeholder: "5" },
    { key: "step", label: "Step", placeholder: "0.1" },
  ],
  Ac: [
    { key: "variation", label: "Type", placeholder: "dec" },
    { key: "points", label: "Points", placeholder: "100" },
    { key: "fstart", label: "Start Freq", placeholder: "1" },
    { key: "fstop", label: "Stop Freq", placeholder: "1G" },
  ],
  Transient: [
    { key: "tstep", label: "Time Step", placeholder: "1u" },
    { key: "tstop", label: "Stop Time", placeholder: "10m" },
    { key: "tstart", label: "Start Time", placeholder: "" },
  ],
  Noise: [
    { key: "output", label: "Output", placeholder: "V(out)" },
    { key: "source", label: "Source", placeholder: "V1" },
    { key: "variation", label: "Type", placeholder: "dec" },
    { key: "points", label: "Points", placeholder: "100" },
    { key: "fstart", label: "Start Freq", placeholder: "1" },
    { key: "fstop", label: "Stop Freq", placeholder: "1G" },
  ],
};

export default function SimulationPanel() {
  const solvers = useSimulationStore((s) => s.solvers);
  const solversDetected = useSimulationStore((s) => s.solversDetected);
  const detectSolvers = useSimulationStore((s) => s.detectSolvers);
  const analysisType = useSimulationStore((s) => s.analysisType);
  const analysisParams = useSimulationStore((s) => s.analysisParams);
  const setAnalysisType = useSimulationStore((s) => s.setAnalysisType);
  const setAnalysisParam = useSimulationStore((s) => s.setAnalysisParam);
  const isRunning = useSimulationStore((s) => s.isRunning);
  const progress = useSimulationStore((s) => s.progress);
  const progressMessage = useSimulationStore((s) => s.progressMessage);
  const runSimulation = useSimulationStore((s) => s.runSimulation);
  const previewNetlist = useSimulationStore((s) => s.previewNetlist);
  const netlistPreview = useSimulationStore((s) => s.netlistPreview);
  const results = useSimulationStore((s) => s.results);
  const activeResultId = useSimulationStore((s) => s.activeResultId);
  const selectResult = useSimulationStore((s) => s.selectResult);

  const schData = useSchematicStore((s) => s.data);

  useEffect(() => {
    if (!solversDetected) detectSolvers();
  }, [solversDetected, detectSolvers]);

  const fields = PARAM_FIELDS[analysisType];

  const handleRun = () => {
    if (!schData) return;
    runSimulation(schData);
  };

  const handlePreview = () => {
    if (!schData) return;
    previewNetlist(schData);
  };

  return (
    <div className="p-2 text-xs overflow-y-auto h-full space-y-3">
      {/* Solver Status */}
      <section>
        <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
          Solvers
        </div>
        <div className="space-y-0.5">
          <SolverRow label="ngspice" available={solvers.ngspice} path={solvers.ngspice_path} />
          <SolverRow label="OpenEMS" available={solvers.openems} path={solvers.openems_path} />
          <SolverRow label="Elmer" available={solvers.elmer} path={solvers.elmer_path} />
        </div>
      </section>

      {/* Analysis Setup */}
      <section>
        <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
          Analysis
        </div>
        <select
          value={analysisType}
          onChange={(e) => setAnalysisType(e.target.value as AnalysisType)}
          className="w-full bg-[var(--input-bg)] border border-[var(--border)] rounded px-2 py-1 text-[var(--text-primary)]"
        >
          {ANALYSIS_TYPES.map((at) => (
            <option key={at.value} value={at.value}>{at.label}</option>
          ))}
        </select>

        {fields.length > 0 && (
          <div className="mt-2 space-y-1">
            {fields.map((f) => (
              <div key={f.key} className="flex items-center gap-2">
                <label className="w-20 text-[var(--text-secondary)] text-right flex-shrink-0">
                  {f.label}
                </label>
                <input
                  type="text"
                  value={analysisParams[f.key] || ""}
                  onChange={(e) => setAnalysisParam(f.key, e.target.value)}
                  placeholder={f.placeholder}
                  className="flex-1 bg-[var(--input-bg)] border border-[var(--border)] rounded px-2 py-0.5 text-[var(--text-primary)]"
                />
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Run Controls */}
      <section className="flex gap-2">
        <button
          onClick={handleRun}
          disabled={isRunning || !schData || !solvers.ngspice}
          className="flex-1 px-3 py-1.5 rounded text-white font-medium bg-emerald-600 hover:bg-emerald-500 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {isRunning ? "Running..." : "Run"}
        </button>
        <button
          onClick={handlePreview}
          disabled={!schData}
          className="px-3 py-1.5 rounded border border-[var(--border)] text-[var(--text-primary)] hover:bg-[var(--hover)] disabled:opacity-40"
        >
          Netlist
        </button>
      </section>

      {/* Progress */}
      {isRunning && (
        <div>
          <div className="h-1.5 bg-[var(--border)] rounded overflow-hidden">
            <div
              className="h-full bg-emerald-500 transition-all"
              style={{ width: `${progress}%` }}
            />
          </div>
          <div className="text-[10px] text-[var(--text-secondary)] mt-0.5">{progressMessage}</div>
        </div>
      )}

      {/* Netlist Preview */}
      {netlistPreview && (
        <section>
          <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
            Netlist Preview
          </div>
          <pre className="bg-[var(--input-bg)] border border-[var(--border)] rounded p-2 text-[10px] text-[var(--text-primary)] font-mono overflow-auto max-h-48 whitespace-pre">
            {netlistPreview}
          </pre>
        </section>
      )}

      {/* History */}
      {Object.keys(results).length > 0 && (
        <section>
          <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
            History
          </div>
          <div className="space-y-0.5">
            {Object.values(results).map((r) => (
              <button
                key={r.sim_id}
                onClick={() => selectResult(r.sim_id)}
                className={`w-full text-left px-2 py-1 rounded text-[var(--text-primary)] hover:bg-[var(--hover)] ${
                  activeResultId === r.sim_id ? "bg-[var(--hover)]" : ""
                }`}
              >
                <span className="font-medium">{r.analysis_type}</span>
                <span className="text-[var(--text-secondary)] ml-2">
                  {r.elapsed_ms}ms — {Object.keys(r.vectors).length} vectors
                </span>
              </button>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

function SolverRow({ label, available, path }: { label: string; available: boolean; path: string | null }) {
  return (
    <div className="flex items-center gap-2">
      <span className={`w-2 h-2 rounded-full ${available ? "bg-emerald-500" : "bg-red-500"}`} />
      <span className="text-[var(--text-primary)]">{label}</span>
      {available && path && (
        <span className="text-[var(--text-secondary)] truncate text-[10px]">{path}</span>
      )}
      {!available && (
        <span className="text-[var(--text-secondary)] text-[10px]">Not found</span>
      )}
    </div>
  );
}
