import { useRef, useEffect, useCallback } from "react";
import { useEditorStore } from "@/stores/editor";
import type { SchematicData } from "@/types";

interface SchematicRendererProps {
  data: SchematicData;
}

interface Camera {
  x: number;
  y: number;
  zoom: number;
}

const COLORS = {
  background: "#1a1b2e",
  grid: "#222440",
  gridMajor: "#2a2d4a",
  wire: "#4fc3f7",
  junction: "#4fc3f7",
  symbolBody: "#7986cb",
  symbolPin: "#aaa",
  reference: "#e8c66a",
  value: "#9598b3",
  labelNet: "#81c784",
  labelGlobal: "#ff8a65",
  labelHierarchical: "#ba68c8",
  sheetBox: "#5b8def",
  sheetText: "#cdd6f4",
  noConnect: "#e8667a",
  cursor: "#5b8def33",
};

export function SchematicRenderer({ data }: SchematicRendererProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const cameraRef = useRef<Camera>({ x: 0, y: 0, zoom: 3 });
  const dragging = useRef(false);
  const lastMouse = useRef({ x: 0, y: 0 });
  const updateStatusBar = useEditorStore((s) => s.updateStatusBar);
  const animFrameRef = useRef<number>(0);

  const screenToWorld = useCallback((sx: number, sy: number) => {
    const cam = cameraRef.current;
    return {
      x: (sx - cam.x) / cam.zoom,
      y: (sy - cam.y) / cam.zoom,
    };
  }, []);

  const render = useCallback(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = container.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.scale(dpr, dpr);
    const cam = cameraRef.current;
    const w = rect.width;
    const h = rect.height;

    // Background
    ctx.fillStyle = COLORS.background;
    ctx.fillRect(0, 0, w, h);

    // Grid
    const gridSize = 1.27; // KiCad uses 1.27mm grid
    const gridPx = gridSize * cam.zoom;

    if (gridPx > 4) {
      const startWorld = screenToWorld(0, 0);
      const endWorld = screenToWorld(w, h);

      const startX = Math.floor(startWorld.x / gridSize) * gridSize;
      const startY = Math.floor(startWorld.y / gridSize) * gridSize;

      ctx.beginPath();
      for (let gx = startX; gx <= endWorld.x; gx += gridSize) {
        const sx = gx * cam.zoom + cam.x;
        const isMajor = Math.abs(gx % (gridSize * 10)) < 0.01;
        ctx.strokeStyle = isMajor ? COLORS.gridMajor : COLORS.grid;
        ctx.lineWidth = isMajor ? 0.5 : 0.3;
        ctx.beginPath();
        ctx.moveTo(sx, 0);
        ctx.lineTo(sx, h);
        ctx.stroke();
      }
      for (let gy = startY; gy <= endWorld.y; gy += gridSize) {
        const sy = gy * cam.zoom + cam.y;
        const isMajor = Math.abs(gy % (gridSize * 10)) < 0.01;
        ctx.strokeStyle = isMajor ? COLORS.gridMajor : COLORS.grid;
        ctx.lineWidth = isMajor ? 0.5 : 0.3;
        ctx.beginPath();
        ctx.moveTo(0, sy);
        ctx.lineTo(w, sy);
        ctx.stroke();
      }
    }

    ctx.save();
    ctx.translate(cam.x, cam.y);
    ctx.scale(cam.zoom, cam.zoom);

    // Wires
    ctx.strokeStyle = COLORS.wire;
    ctx.lineWidth = 0.3;
    ctx.lineCap = "round";
    for (const wire of data.wires) {
      ctx.beginPath();
      ctx.moveTo(wire.start.x, wire.start.y);
      ctx.lineTo(wire.end.x, wire.end.y);
      ctx.stroke();
    }

    // Junctions
    ctx.fillStyle = COLORS.junction;
    for (const j of data.junctions) {
      ctx.beginPath();
      ctx.arc(j.position.x, j.position.y, 0.5, 0, Math.PI * 2);
      ctx.fill();
    }

    // No-connect markers
    ctx.strokeStyle = COLORS.noConnect;
    ctx.lineWidth = 0.2;
    for (const nc of data.no_connects) {
      const s = 0.8;
      ctx.beginPath();
      ctx.moveTo(nc.x - s, nc.y - s);
      ctx.lineTo(nc.x + s, nc.y + s);
      ctx.moveTo(nc.x + s, nc.y - s);
      ctx.lineTo(nc.x - s, nc.y + s);
      ctx.stroke();
    }

    // Symbols (non-power)
    const realSymbols = data.symbols.filter((s) => !s.is_power);
    for (const sym of realSymbols) {
      ctx.save();
      ctx.translate(sym.position.x, sym.position.y);
      ctx.rotate((sym.rotation * Math.PI) / 180);

      // Symbol body rectangle
      ctx.strokeStyle = COLORS.symbolBody;
      ctx.lineWidth = 0.15;
      ctx.strokeRect(-3, -2, 6, 4);

      // Pin stubs
      ctx.strokeStyle = COLORS.symbolPin;
      ctx.lineWidth = 0.1;
      ctx.beginPath();
      ctx.moveTo(-3, 0);
      ctx.lineTo(-4.5, 0);
      ctx.moveTo(3, 0);
      ctx.lineTo(4.5, 0);
      ctx.stroke();

      ctx.restore();

      // Reference designator (above)
      ctx.fillStyle = COLORS.reference;
      ctx.font = `bold ${1.2}px Roboto, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "bottom";
      ctx.fillText(sym.reference, sym.position.x, sym.position.y - 2.5);

      // Value (below)
      ctx.fillStyle = COLORS.value;
      ctx.font = `${1.0}px Roboto, sans-serif`;
      ctx.textBaseline = "top";
      ctx.fillText(sym.value, sym.position.x, sym.position.y + 2.5);
    }

    // Power symbols (smaller, just text)
    const powerSymbols = data.symbols.filter((s) => s.is_power);
    for (const sym of powerSymbols) {
      ctx.fillStyle = COLORS.noConnect;
      ctx.font = `bold ${1.0}px Roboto, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(sym.value || sym.reference, sym.position.x, sym.position.y);
    }

    // Labels
    for (const label of data.labels) {
      let color: string;
      switch (label.label_type) {
        case "Net":
          color = COLORS.labelNet;
          break;
        case "Global":
          color = COLORS.labelGlobal;
          break;
        case "Hierarchical":
          color = COLORS.labelHierarchical;
          break;
        default:
          color = COLORS.labelNet;
      }

      ctx.fillStyle = color;
      ctx.font = `${1.1}px Roboto, sans-serif`;
      ctx.textAlign = "left";
      ctx.textBaseline = "middle";
      ctx.fillText(label.text, label.position.x, label.position.y);
    }

    // Child sheet boxes
    ctx.strokeStyle = COLORS.sheetBox;
    ctx.lineWidth = 0.2;
    ctx.setLineDash([0.5, 0.3]);
    for (const sheet of data.child_sheets) {
      ctx.strokeRect(
        sheet.position.x,
        sheet.position.y,
        sheet.size[0],
        sheet.size[1]
      );

      // Sheet name
      ctx.fillStyle = COLORS.sheetText;
      ctx.font = `bold ${1.3}px Roboto, sans-serif`;
      ctx.textAlign = "left";
      ctx.textBaseline = "bottom";
      ctx.fillText(
        sheet.name,
        sheet.position.x + 0.5,
        sheet.position.y - 0.3
      );

      // Sheet filename (inside box)
      ctx.fillStyle = COLORS.sheetBox;
      ctx.font = `${0.9}px Roboto Mono, monospace`;
      ctx.textBaseline = "top";
      ctx.fillText(
        sheet.filename,
        sheet.position.x + 0.5,
        sheet.position.y + 0.5
      );
    }
    ctx.setLineDash([]);

    ctx.restore();
  }, [data, screenToWorld]);

  // Initial fit-to-view
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !data) return;

    const rect = container.getBoundingClientRect();
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;

    for (const w of data.wires) {
      minX = Math.min(minX, w.start.x, w.end.x);
      minY = Math.min(minY, w.start.y, w.end.y);
      maxX = Math.max(maxX, w.start.x, w.end.x);
      maxY = Math.max(maxY, w.start.y, w.end.y);
    }
    for (const s of data.symbols) {
      minX = Math.min(minX, s.position.x - 5);
      minY = Math.min(minY, s.position.y - 5);
      maxX = Math.max(maxX, s.position.x + 5);
      maxY = Math.max(maxY, s.position.y + 5);
    }
    for (const sh of data.child_sheets) {
      minX = Math.min(minX, sh.position.x);
      minY = Math.min(minY, sh.position.y);
      maxX = Math.max(maxX, sh.position.x + sh.size[0]);
      maxY = Math.max(maxY, sh.position.y + sh.size[1]);
    }

    if (!isFinite(minX)) {
      minX = 0; minY = 0; maxX = 297; maxY = 210; // A4
    }

    const padding = 20;
    const contentW = maxX - minX;
    const contentH = maxY - minY;
    const zoom = Math.min(
      (rect.width - padding * 2) / contentW,
      (rect.height - padding * 2) / contentH
    );

    cameraRef.current = {
      zoom,
      x: (rect.width - contentW * zoom) / 2 - minX * zoom,
      y: (rect.height - contentH * zoom) / 2 - minY * zoom,
    };

    updateStatusBar({ zoom: Math.round(zoom * 100 / 3) });
    render();
  }, [data, render, updateStatusBar]);

  // Resize observer
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const obs = new ResizeObserver(() => render());
    obs.observe(container);
    return () => obs.disconnect();
  }, [render]);

  // Mouse handlers
  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      const cam = cameraRef.current;
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;

      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;

      const newZoom = Math.min(100, Math.max(0.5, cam.zoom * factor));
      cam.x = mx - (mx - cam.x) * (newZoom / cam.zoom);
      cam.y = my - (my - cam.y) * (newZoom / cam.zoom);
      cam.zoom = newZoom;

      updateStatusBar({ zoom: Math.round(newZoom * 100 / 3) });
      cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = requestAnimationFrame(render);
    },
    [render, updateStatusBar]
  );

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 1 || (e.button === 0 && e.altKey)) {
      dragging.current = true;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      e.preventDefault();
    }
  }, []);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (rect) {
        const world = screenToWorld(e.clientX - rect.left, e.clientY - rect.top);
        updateStatusBar({
          cursorPosition: {
            x: Math.round(world.x * 100) / 100,
            y: Math.round(world.y * 100) / 100,
          },
        });
      }

      if (dragging.current) {
        const dx = e.clientX - lastMouse.current.x;
        const dy = e.clientY - lastMouse.current.y;
        cameraRef.current.x += dx;
        cameraRef.current.y += dy;
        lastMouse.current = { x: e.clientX, y: e.clientY };
        cancelAnimationFrame(animFrameRef.current);
        animFrameRef.current = requestAnimationFrame(render);
      }
    },
    [render, screenToWorld, updateStatusBar]
  );

  const handleMouseUp = useCallback(() => {
    dragging.current = false;
  }, []);

  return (
    <div ref={containerRef} className="w-full h-full overflow-hidden relative">
      <canvas
        ref={canvasRef}
        className="absolute inset-0"
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onContextMenu={(e) => e.preventDefault()}
      />
      {/* Stats overlay */}
      <div className="absolute top-2 left-2 text-[10px] text-text-muted/40 bg-bg-primary/60 px-2 py-1 rounded pointer-events-none">
        {data.symbols.filter(s => !s.is_power).length} components | {data.wires.length} wires | {data.labels.length} labels | {data.child_sheets.length} sheets
      </div>
    </div>
  );
}
