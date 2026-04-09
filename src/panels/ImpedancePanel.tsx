import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface StackupLayer {
  name: string;
  height_um: number;
  dielectric_er: number;
  is_copper: boolean;
}

interface TraceGeometry {
  width_um: number;
  thickness_um: number;
  layer_index: number;
  offset_um: number;
}

interface ImpedanceResult {
  z0: number;
  z_diff: number | null;
  z_odd: number | null;
  z_even: number | null;
  er_eff: number;
  delay_ps_per_mm: number;
  loss_db_per_mm: number | null;
}

type TraceMode = "single" | "differential";

const PRESETS: Record<string, number> = {
  "2L 1.6mm": 2,
  "4L 1.2mm": 4,
  "6L 2.0mm": 6,
};

export default function ImpedancePanel() {
  const [stackup, setStackup] = useState<StackupLayer[]>([
    { name: "Top", height_um: 35, dielectric_er: 1.0, is_copper: true },
    { name: "FR4", height_um: 1500, dielectric_er: 4.3, is_copper: false },
    { name: "Bottom", height_um: 35, dielectric_er: 1.0, is_copper: true },
  ]);

  const [traceWidth, setTraceWidth] = useState(300);
  const [traceThickness, setTraceThickness] = useState(35);
  const [traceLayer, setTraceLayer] = useState(0);
  const [traceMode, setTraceMode] = useState<TraceMode>("single");
  const [diffGap, setDiffGap] = useState(150);
  const [result, setResult] = useState<ImpedanceResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [computing, setComputing] = useState(false);

  const copperLayers = stackup
    .map((l, i) => ({ ...l, index: i }))
    .filter((l) => l.is_copper);

  const calculate = useCallback(async () => {
    setComputing(true);
    setError(null);
    try {
      const traces: TraceGeometry[] =
        traceMode === "single"
          ? [{ width_um: traceWidth, thickness_um: traceThickness, layer_index: traceLayer, offset_um: 0 }]
          : [
              { width_um: traceWidth, thickness_um: traceThickness, layer_index: traceLayer, offset_um: -(traceWidth + diffGap) / 2 },
              { width_um: traceWidth, thickness_um: traceThickness, layer_index: traceLayer, offset_um: (traceWidth + diffGap) / 2 },
            ];

      const res = await invoke<ImpedanceResult>("calculate_impedance", {
        request: { stackup, traces, grid_resolution: 20 },
      });
      setResult(res);
    } catch (e) {
      setError(String(e));
      setResult(null);
    } finally {
      setComputing(false);
    }
  }, [stackup, traceWidth, traceThickness, traceLayer, traceMode, diffGap]);

  const loadPreset = async (layers: number) => {
    try {
      const preset = await invoke<StackupLayer[]>("get_default_stackup", { layerCount: layers });
      setStackup(preset);
      setTraceLayer(0);
      setResult(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const updateLayer = (idx: number, field: keyof StackupLayer, value: string | number | boolean) => {
    setStackup((s) => s.map((l, i) => (i === idx ? { ...l, [field]: value } : l)));
  };

  return (
    <div className="p-2 text-xs overflow-y-auto h-full space-y-3">
      {/* Presets */}
      <section>
        <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
          Presets
        </div>
        <div className="flex gap-1">
          {Object.entries(PRESETS).map(([label, layers]) => (
            <button
              key={label}
              onClick={() => loadPreset(layers)}
              className="px-2 py-1 rounded border border-[var(--border)] text-[var(--text-primary)] hover:bg-[var(--hover)]"
            >
              {label}
            </button>
          ))}
        </div>
      </section>

      {/* Stackup Editor */}
      <section>
        <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
          Stackup
        </div>
        <table className="w-full text-[10px]">
          <thead>
            <tr className="text-[var(--text-secondary)]">
              <th className="text-left px-1">Name</th>
              <th className="text-right px-1">Height</th>
              <th className="text-right px-1">Er</th>
              <th className="text-center px-1">Cu</th>
            </tr>
          </thead>
          <tbody>
            {stackup.map((layer, i) => (
              <tr key={i} className="border-t border-[var(--border)]">
                <td className="px-1 py-0.5">
                  <input
                    value={layer.name}
                    onChange={(e) => updateLayer(i, "name", e.target.value)}
                    className="w-full bg-transparent text-[var(--text-primary)]"
                  />
                </td>
                <td className="px-1 py-0.5">
                  <input
                    type="number"
                    value={layer.height_um}
                    onChange={(e) => updateLayer(i, "height_um", Number(e.target.value))}
                    className="w-16 bg-[var(--input-bg)] border border-[var(--border)] rounded px-1 text-right text-[var(--text-primary)]"
                  />
                </td>
                <td className="px-1 py-0.5">
                  <input
                    type="number"
                    step="0.1"
                    value={layer.dielectric_er}
                    onChange={(e) => updateLayer(i, "dielectric_er", Number(e.target.value))}
                    disabled={layer.is_copper}
                    className="w-12 bg-[var(--input-bg)] border border-[var(--border)] rounded px-1 text-right text-[var(--text-primary)] disabled:opacity-40"
                  />
                </td>
                <td className="px-1 py-0.5 text-center">
                  <input
                    type="checkbox"
                    checked={layer.is_copper}
                    onChange={(e) => updateLayer(i, "is_copper", e.target.checked)}
                    className="w-3 h-3"
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {/* Trace Config */}
      <section>
        <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
          Trace
        </div>
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <label className="w-16 text-[var(--text-secondary)] text-right">Mode</label>
            <select
              value={traceMode}
              onChange={(e) => setTraceMode(e.target.value as TraceMode)}
              className="flex-1 bg-[var(--input-bg)] border border-[var(--border)] rounded px-2 py-0.5 text-[var(--text-primary)]"
            >
              <option value="single">Single-ended</option>
              <option value="differential">Differential</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            <label className="w-16 text-[var(--text-secondary)] text-right">Width</label>
            <input
              type="number"
              value={traceWidth}
              onChange={(e) => setTraceWidth(Number(e.target.value))}
              className="flex-1 bg-[var(--input-bg)] border border-[var(--border)] rounded px-2 py-0.5 text-[var(--text-primary)]"
            />
            <span className="text-[var(--text-secondary)]">um</span>
          </div>
          <div className="flex items-center gap-2">
            <label className="w-16 text-[var(--text-secondary)] text-right">Thick</label>
            <input
              type="number"
              value={traceThickness}
              onChange={(e) => setTraceThickness(Number(e.target.value))}
              className="flex-1 bg-[var(--input-bg)] border border-[var(--border)] rounded px-2 py-0.5 text-[var(--text-primary)]"
            />
            <span className="text-[var(--text-secondary)]">um</span>
          </div>
          <div className="flex items-center gap-2">
            <label className="w-16 text-[var(--text-secondary)] text-right">Layer</label>
            <select
              value={traceLayer}
              onChange={(e) => setTraceLayer(Number(e.target.value))}
              className="flex-1 bg-[var(--input-bg)] border border-[var(--border)] rounded px-2 py-0.5 text-[var(--text-primary)]"
            >
              {copperLayers.map((l) => (
                <option key={l.index} value={l.index}>{l.name}</option>
              ))}
            </select>
          </div>
          {traceMode === "differential" && (
            <div className="flex items-center gap-2">
              <label className="w-16 text-[var(--text-secondary)] text-right">Gap</label>
              <input
                type="number"
                value={diffGap}
                onChange={(e) => setDiffGap(Number(e.target.value))}
                className="flex-1 bg-[var(--input-bg)] border border-[var(--border)] rounded px-2 py-0.5 text-[var(--text-primary)]"
              />
              <span className="text-[var(--text-secondary)]">um</span>
            </div>
          )}
        </div>
      </section>

      {/* Calculate */}
      <button
        onClick={calculate}
        disabled={computing}
        className="w-full px-3 py-1.5 rounded text-white font-medium bg-blue-600 hover:bg-blue-500 disabled:opacity-40"
      >
        {computing ? "Computing..." : "Calculate"}
      </button>

      {/* Results */}
      {result && (
        <section className="bg-[var(--input-bg)] border border-[var(--border)] rounded p-2 space-y-1">
          <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide mb-1">
            Results
          </div>
          <ResultRow label="Z0" value={`${result.z0.toFixed(1)} \u03A9`} />
          {result.z_diff != null && (
            <ResultRow label="Z_diff" value={`${result.z_diff.toFixed(1)} \u03A9`} />
          )}
          <ResultRow label="Er_eff" value={result.er_eff.toFixed(2)} />
          <ResultRow label="Delay" value={`${result.delay_ps_per_mm.toFixed(2)} ps/mm`} />
        </section>
      )}

      {error && (
        <div className="text-red-400 text-[10px] bg-red-900/20 rounded p-2">{error}</div>
      )}
    </div>
  );
}

function ResultRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-[var(--text-secondary)]">{label}</span>
      <span className="text-[var(--text-primary)] font-mono">{value}</span>
    </div>
  );
}
