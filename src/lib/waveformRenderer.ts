import type { WaveformData } from "@/stores/simulation";

// --- Types ---

export interface WaveformTrace {
  data: WaveformData;
  color: string;
  visible: boolean;
}

export interface WaveformBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WaveformRange {
  min: number;
  max: number;
}

export interface WaveformOptions {
  bounds: WaveformBounds;
  xRange: WaveformRange;
  yRange: WaveformRange;
  xLabel: string;
  yLabel: string;
  dbScale: boolean;
  gridColor: string;
  textColor: string;
  bgColor: string;
  cursorX: number | null;
}

// 8 distinct trace colors
const TRACE_COLORS = [
  "#22d3ee", // cyan
  "#f97316", // orange
  "#a78bfa", // purple
  "#34d399", // emerald
  "#f472b6", // pink
  "#facc15", // yellow
  "#60a5fa", // blue
  "#fb923c", // amber
];

export function getTraceColor(index: number): string {
  return TRACE_COLORS[index % TRACE_COLORS.length];
}

// --- Engineering format ---

export function formatEngineering(value: number, unit: string): string {
  if (value === 0) return `0 ${unit}`;
  const abs = Math.abs(value);
  const prefixes: [number, string][] = [
    [1e15, "P"], [1e12, "T"], [1e9, "G"], [1e6, "M"], [1e3, "k"],
    [1, ""], [1e-3, "m"], [1e-6, "μ"], [1e-9, "n"], [1e-12, "p"], [1e-15, "f"],
  ];
  for (const [threshold, prefix] of prefixes) {
    if (abs >= threshold * 0.999) {
      const scaled = value / threshold;
      const decimals = Math.abs(scaled) >= 100 ? 1 : Math.abs(scaled) >= 10 ? 2 : 3;
      return `${scaled.toFixed(decimals)} ${prefix}${unit}`;
    }
  }
  return `${value.toExponential(2)} ${unit}`;
}

// --- Auto-scale ---

export function autoScale(vectors: WaveformData[], xVecName: string): { xRange: WaveformRange; yRange: WaveformRange } {
  let xMin = Infinity, xMax = -Infinity;
  let yMin = Infinity, yMax = -Infinity;

  for (const v of vectors) {
    if (v.name.toLowerCase() === xVecName.toLowerCase()) {
      for (const val of v.real) {
        if (val < xMin) xMin = val;
        if (val > xMax) xMax = val;
      }
    } else {
      for (const val of v.real) {
        if (val < yMin) yMin = val;
        if (val > yMax) yMax = val;
      }
    }
  }

  // Add 10% padding
  const xPad = (xMax - xMin) * 0.1 || 1;
  const yPad = (yMax - yMin) * 0.1 || 1;

  return {
    xRange: { min: xMin - xPad, max: xMax + xPad },
    yRange: { min: yMin - yPad, max: yMax + yPad },
  };
}

// --- Main renderer ---

export function renderWaveforms(
  ctx: CanvasRenderingContext2D,
  xData: number[],
  traces: WaveformTrace[],
  opts: WaveformOptions,
) {
  const { bounds, xRange, yRange, xLabel, yLabel, gridColor, textColor, bgColor, cursorX } = opts;
  const { x: bx, y: by, width: bw, height: bh } = bounds;

  // Background
  ctx.fillStyle = bgColor;
  ctx.fillRect(bx, by, bw, bh);

  // Plot area (with margins for axes)
  const margin = { left: 70, right: 20, top: 20, bottom: 40 };
  const px = bx + margin.left;
  const py = by + margin.top;
  const pw = bw - margin.left - margin.right;
  const ph = bh - margin.top - margin.bottom;

  if (pw <= 0 || ph <= 0) return;

  // Transform functions
  const toScreenX = (v: number) => px + ((v - xRange.min) / (xRange.max - xRange.min)) * pw;
  const toScreenY = (v: number) => py + ph - ((v - yRange.min) / (yRange.max - yRange.min)) * ph;

  // Grid lines
  ctx.strokeStyle = gridColor;
  ctx.lineWidth = 0.5;
  ctx.font = "11px monospace";
  ctx.fillStyle = textColor;
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";

  const yTicks = niceScale(yRange.min, yRange.max, 6);
  for (const tick of yTicks) {
    const sy = toScreenY(tick);
    if (sy < py || sy > py + ph) continue;
    ctx.beginPath();
    ctx.moveTo(px, sy);
    ctx.lineTo(px + pw, sy);
    ctx.stroke();
    ctx.fillText(formatEngineering(tick, ""), px - 6, sy);
  }

  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  const xTicks = niceScale(xRange.min, xRange.max, 8);
  for (const tick of xTicks) {
    const sx = toScreenX(tick);
    if (sx < px || sx > px + pw) continue;
    ctx.beginPath();
    ctx.moveTo(sx, py);
    ctx.lineTo(sx, py + ph);
    ctx.stroke();
    ctx.fillText(formatEngineering(tick, ""), sx, py + ph + 4);
  }

  // Axis labels
  ctx.fillStyle = textColor;
  ctx.textAlign = "center";
  ctx.fillText(xLabel, px + pw / 2, py + ph + 24);

  ctx.save();
  ctx.translate(bx + 14, py + ph / 2);
  ctx.rotate(-Math.PI / 2);
  ctx.fillText(yLabel, 0, 0);
  ctx.restore();

  // Plot border
  ctx.strokeStyle = gridColor;
  ctx.lineWidth = 1;
  ctx.strokeRect(px, py, pw, ph);

  // Clip to plot area
  ctx.save();
  ctx.beginPath();
  ctx.rect(px, py, pw, ph);
  ctx.clip();

  // Draw traces
  for (const trace of traces) {
    if (!trace.visible || trace.data.real.length === 0) continue;

    ctx.strokeStyle = trace.color;
    ctx.lineWidth = 1.5;
    ctx.beginPath();

    for (let i = 0; i < trace.data.real.length && i < xData.length; i++) {
      const sx = toScreenX(xData[i]);
      const sy = toScreenY(trace.data.real[i]);
      if (i === 0) ctx.moveTo(sx, sy);
      else ctx.lineTo(sx, sy);
    }
    ctx.stroke();
  }

  // Cursor
  if (cursorX !== null) {
    const sx = toScreenX(cursorX);
    if (sx >= px && sx <= px + pw) {
      ctx.strokeStyle = "#ffffff80";
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(sx, py);
      ctx.lineTo(sx, py + ph);
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }

  ctx.restore();
}

// --- Nice scale ticks ---

function niceScale(min: number, max: number, targetTicks: number): number[] {
  const range = max - min;
  if (range <= 0) return [min];

  const roughStep = range / targetTicks;
  const mag = Math.pow(10, Math.floor(Math.log10(roughStep)));
  const normalized = roughStep / mag;

  let step: number;
  if (normalized < 1.5) step = mag;
  else if (normalized < 3.5) step = 2 * mag;
  else if (normalized < 7.5) step = 5 * mag;
  else step = 10 * mag;

  const ticks: number[] = [];
  let tick = Math.ceil(min / step) * step;
  while (tick <= max + step * 0.01) {
    ticks.push(tick);
    tick += step;
  }
  return ticks;
}
