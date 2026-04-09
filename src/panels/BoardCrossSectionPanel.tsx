import { useRef, useEffect } from "react";
import { usePcbStore } from "@/stores/pcb";
import { useEditorStore } from "@/stores/editor";
import { useProjectStore } from "@/stores/project";
import { DEFAULT_LAYER_COLORS, LAYER_DISPLAY_NAMES } from "@/types/pcb";
import { Layers } from "lucide-react";

/**
 * Board Cross-Section View — visualizes the PCB layer stackup.
 */
export function BoardCrossSectionPanel() {
  const editorMode = useEditorStore((s) => s.mode);
  const project = useProjectStore((s) => s.project);

  if (!project || editorMode !== "pcb") {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-muted/40 text-xs gap-2 p-6">
        <Layers size={24} className="opacity-20" />
        <span>Cross section available in PCB view</span>
      </div>
    );
  }
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const data = usePcbStore((s) => s.data);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !data) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.parentElement?.getBoundingClientRect();
    if (!rect) return;

    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = rect.width + "px";
    canvas.style.height = rect.height + "px";
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const w = rect.width;
    const h = rect.height;

    // Background
    ctx.fillStyle = "#1a1b2e";
    ctx.fillRect(0, 0, w, h);

    const copperCount = data.board.layers.copperCount || 2;
    const boardThickness = data.board.thickness || 1.6;

    // Layout parameters
    const margin = 20;
    const boardWidth = w - margin * 2;
    const scaleFactor = (h - margin * 4) / boardThickness;
    const boardTop = margin * 2;

    // Draw title
    ctx.fillStyle = "#cdd6f4";
    ctx.font = "11px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(`Board Cross-Section — ${copperCount} layers, ${boardThickness}mm thick`, w / 2, 14);

    // Layer thicknesses (approximate, mm)
    const silkThickness = 0.02;
    const maskThickness = 0.025;
    const copperThickness = 0.035; // 1oz copper
    const prepreg = boardThickness / (copperCount + 1);

    let y = boardTop;

    // Top silkscreen
    drawLayer(ctx, margin, y, boardWidth, silkThickness * scaleFactor,
      DEFAULT_LAYER_COLORS["F.SilkS"], LAYER_DISPLAY_NAMES["F.SilkS"] || "F.SilkS", w);
    y += silkThickness * scaleFactor;

    // Top solder mask
    drawLayer(ctx, margin, y, boardWidth, maskThickness * scaleFactor,
      DEFAULT_LAYER_COLORS["F.Mask"], LAYER_DISPLAY_NAMES["F.Mask"] || "F.Mask", w);
    y += maskThickness * scaleFactor;

    // Copper and dielectric layers
    const copperLayers = ["F.Cu"];
    for (let i = 1; i < copperCount - 1; i++) copperLayers.push(`In${i}.Cu`);
    copperLayers.push("B.Cu");

    for (let i = 0; i < copperLayers.length; i++) {
      const layerId = copperLayers[i];

      // Copper
      drawLayer(ctx, margin, y, boardWidth, copperThickness * scaleFactor,
        DEFAULT_LAYER_COLORS[layerId] || "#cc8800",
        LAYER_DISPLAY_NAMES[layerId] || layerId, w);
      y += copperThickness * scaleFactor;

      // Dielectric (prepreg/core) between copper layers
      if (i < copperLayers.length - 1) {
        const dielThick = prepreg - copperThickness;
        const dielColor = i === 0 || i === copperLayers.length - 2 ? "#2a5a2a" : "#3a6a3a";
        const dielLabel = i % 2 === 0 ? "Core" : "Prepreg";
        drawLayer(ctx, margin, y, boardWidth, dielThick * scaleFactor,
          dielColor, `${dielLabel} (${dielThick.toFixed(2)}mm)`, w);
        y += dielThick * scaleFactor;
      }
    }

    // Bottom solder mask
    drawLayer(ctx, margin, y, boardWidth, maskThickness * scaleFactor,
      DEFAULT_LAYER_COLORS["B.Mask"], LAYER_DISPLAY_NAMES["B.Mask"] || "B.Mask", w);
    y += maskThickness * scaleFactor;

    // Bottom silkscreen
    drawLayer(ctx, margin, y, boardWidth, silkThickness * scaleFactor,
      DEFAULT_LAYER_COLORS["B.SilkS"], LAYER_DISPLAY_NAMES["B.SilkS"] || "B.SilkS", w);

    // Dimension arrows
    const totalH = y + silkThickness * scaleFactor - boardTop;
    ctx.strokeStyle = "#808080";
    ctx.lineWidth = 0.5;
    ctx.setLineDash([2, 2]);
    ctx.beginPath();
    ctx.moveTo(w - margin + 5, boardTop);
    ctx.lineTo(w - margin + 5, boardTop + totalH);
    ctx.stroke();
    ctx.setLineDash([]);

    ctx.fillStyle = "#a0a0a0";
    ctx.font = "9px sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(`${boardThickness}mm`, w - margin + 8, boardTop + totalH / 2 + 3);

  }, [data]);

  if (!data) {
    return <div className="p-4 text-xs text-text-muted/50">No PCB loaded</div>;
  }

  return (
    <div className="w-full h-full min-h-[150px]">
      <canvas ref={canvasRef} className="w-full h-full" />
    </div>
  );
}

function drawLayer(
  ctx: CanvasRenderingContext2D,
  x: number, y: number, w: number, h: number,
  color: string, label: string, _canvasWidth: number,
) {
  const minH = Math.max(h, 4); // Minimum visible height

  ctx.fillStyle = color;
  ctx.globalAlpha = 0.8;
  ctx.fillRect(x, y, w, minH);
  ctx.globalAlpha = 1;

  ctx.strokeStyle = "#404060";
  ctx.lineWidth = 0.5;
  ctx.strokeRect(x, y, w, minH);

  // Label
  if (minH >= 6) {
    ctx.fillStyle = "#ffffff";
    ctx.font = "9px sans-serif";
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillText(label, x + 5, y + minH / 2);
  }
}
