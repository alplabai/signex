import { useEffect, useRef, useCallback } from "react";
import { useSimulationStore } from "@/stores/simulation";
import {
  renderWaveforms,
  autoScale,
  getTraceColor,
  type WaveformTrace,
} from "@/lib/waveformRenderer";
import { useThemeStore } from "@/stores/theme";

export default function WaveformPanel() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const activeResultId = useSimulationStore((s) => s.activeResultId);
  const results = useSimulationStore((s) => s.results);
  const selectedVectors = useSimulationStore((s) => s.selectedVectors);
  const cursorX = useSimulationStore((s) => s.cursorX);
  const toggleVector = useSimulationStore((s) => s.toggleVector);
  const getActiveTheme = useThemeStore((s) => s.getActiveTheme);
  const theme = getActiveTheme();

  const result = activeResultId ? results[activeResultId] : null;

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container || !result) return;

    const rect = container.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);

    const vectors = result.vectors;
    const vecNames = Object.keys(vectors);

    // Find x-axis vector (time or frequency)
    const xVecName = vecNames.find(
      (n) => n.toLowerCase() === "time" || n.toLowerCase() === "frequency"
    );
    if (!xVecName) {
      ctx.fillStyle = theme?.tokens.textPrimary || "#ccc";
      ctx.font = "13px monospace";
      ctx.fillText("No time/frequency vector found", 20, 30);
      return;
    }

    const xData = vectors[xVecName].real;
    const xUnit = vectors[xVecName].unit || "";

    // Build traces
    const traces: WaveformTrace[] = [];
    let colorIdx = 0;
    for (const name of vecNames) {
      if (name === xVecName) continue;
      traces.push({
        data: vectors[name],
        color: getTraceColor(colorIdx),
        visible: selectedVectors.includes(name),
      });
      colorIdx++;
    }

    const visibleTraces = traces.filter((t) => t.visible);
    const allVecs = [vectors[xVecName], ...visibleTraces.map((t) => t.data)];
    const { xRange, yRange } = autoScale(allVecs, xVecName);

    renderWaveforms(ctx, xData, traces, {
      bounds: { x: 0, y: 0, width: rect.width, height: rect.height },
      xRange,
      yRange,
      xLabel: `${xVecName} (${xUnit})`,
      yLabel: "",
      dbScale: false,
      gridColor: theme?.tokens.canvas.grid || "#333",
      textColor: theme?.tokens.textPrimary || "#ccc",
      bgColor: theme?.tokens.canvas.bg || "#1e1e2e",
      cursorX,
    });
  }, [result, selectedVectors, cursorX, theme]);

  useEffect(() => {
    draw();
    const observer = new ResizeObserver(draw);
    if (containerRef.current) observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, [draw]);

  if (!result) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--text-secondary)] text-xs">
        No simulation results. Run a simulation first.
      </div>
    );
  }

  const vecNames = Object.keys(result.vectors).filter(
    (n) => n.toLowerCase() !== "time" && n.toLowerCase() !== "frequency"
  );

  return (
    <div className="flex h-full">
      {/* Trace list sidebar */}
      <div className="w-36 border-r border-[var(--border)] overflow-y-auto flex-shrink-0 p-1">
        <div className="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide px-1 mb-1">
          Vectors
        </div>
        {vecNames.map((name, i) => (
          <label
            key={name}
            className="flex items-center gap-1.5 px-1 py-0.5 text-xs cursor-pointer hover:bg-[var(--hover)]"
          >
            <input
              type="checkbox"
              checked={selectedVectors.includes(name)}
              onChange={() => toggleVector(name)}
              className="w-3 h-3"
            />
            <span
              className="w-2 h-2 rounded-full flex-shrink-0"
              style={{ backgroundColor: getTraceColor(i) }}
            />
            <span className="truncate text-[var(--text-primary)]">{name}</span>
          </label>
        ))}
      </div>

      {/* Waveform canvas */}
      <div ref={containerRef} className="flex-1 relative">
        <canvas ref={canvasRef} className="absolute inset-0" />
      </div>
    </div>
  );
}
