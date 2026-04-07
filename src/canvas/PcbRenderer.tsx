import { useRef, useEffect, useCallback, useState, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { usePcbStore } from "@/stores/pcb";
import { useEditorStore } from "@/stores/editor";
import { useProjectStore } from "@/stores/project";
import { DEFAULT_LAYER_COLORS, LAYER_DISPLAY_NAMES } from "@/types/pcb";
import { computeRatsnest, getPadPosition } from "@/lib/pcbRatsnest";
import { ContextMenu, type ContextMenuItem } from "@/components/ContextMenu";
import type { PcbData, PcbPoint, PcbLayerId } from "@/types/pcb";

interface Camera { x: number; y: number; zoom: number }

// ═══════════════════════════════════════════════════════════════
// PCB RENDERER — Canvas2D (Phase 6 MVP, WebGL2 migration planned)
//
// Architecture follows KiCad's GAL pattern:
// - All drawing through abstracted primitives
// - Layer-based rendering order
// - Viewport culling via bounding box checks
// - Static/dynamic split for future framebuffer caching
// ═══════════════════════════════════════════════════════════════

export function PcbRenderer() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const camRef = useRef<Camera>({ x: 0, y: 0, zoom: 5 }); // PCB uses ~5 px/mm default
  const animRef = useRef(0);
  const dragging = useRef(false);
  const lastMouse = useRef({ x: 0, y: 0 });

  const project = useProjectStore((s) => s.project);
  const [pcbLoading, setPcbLoading] = useState(false);
  const [pcbError, setPcbError] = useState<string | null>(null);
  const data = usePcbStore((s) => s.data);
  const selectedIds = usePcbStore((s) => s.selectedIds);
  const activeLayer = usePcbStore((s) => s.activeLayer);
  const visibleLayers = usePcbStore((s) => s.visibleLayers);
  const editMode = usePcbStore((s) => s.editMode);
  const routingActive = usePcbStore((s) => s.routingActive);
  const routingPoints = usePcbStore((s) => s.routingPoints);
  const singleLayerMode = usePcbStore((s) => s.singleLayerMode);
  const boardFlipped = usePcbStore((s) => s.boardFlipped);
  const netColorEnabled = usePcbStore((s) => s.netColorEnabled);
  const netColors = usePcbStore((s) => s.netColors);
  const gridVisible = useEditorStore((s) => s.gridVisible);

  // Load PCB data when mounted and no data exists
  useEffect(() => {
    if (data || !project?.pcb_file) return;

    let cancelled = false;
    setPcbLoading(true);
    setPcbError(null);

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    invoke<any>("get_pcb", {
      projectDir: project.dir,
      filename: project.pcb_file,
    })
      .then((raw) => {
        if (!cancelled) {
          // Rust returns flat PcbBoard with snake_case fields; map to PcbData
          const p = (pt: any) => ({ x: pt?.x ?? 0, y: pt?.y ?? 0 });
          const pcbData: PcbData = {
            board: {
              uuid: raw.uuid ?? "",
              version: raw.version ?? "",
              generator: raw.generator ?? "",
              thickness: raw.thickness ?? 1.6,
              outline: (raw.outline ?? []).map(p),
              layers: {
                layers: (raw.layers ?? []).map((l: any) => ({ id: l.id ?? l.name, name: l.name ?? l.id, type: l.layer_type ?? "signal", visible: true, color: "", opacity: 1 })),
                copperCount: (raw.layers ?? []).filter((l: any) => (l.id ?? l.name ?? "").endsWith(".Cu")).length || 2,
              },
              setup: raw.setup ? {
                gridSize: raw.setup.grid_size ?? 0.25,
                traceWidth: raw.setup.trace_width ?? 0.25,
                viaDiameter: raw.setup.via_diameter ?? 0.8,
                viaDrill: raw.setup.via_drill ?? 0.4,
                clearance: raw.setup.clearance ?? 0.2,
                trackMinWidth: raw.setup.track_min_width ?? 0.1,
                viaMinDiameter: raw.setup.via_min_diameter ?? 0.4,
                viaMinDrill: raw.setup.via_min_drill ?? 0.2,
                copperFinish: "",
              } : { gridSize: 0.25, traceWidth: 0.25, viaDiameter: 0.8, viaDrill: 0.4, clearance: 0.2, trackMinWidth: 0.1, viaMinDiameter: 0.4, viaMinDrill: 0.2, copperFinish: "" },
            },
            footprints: (raw.footprints ?? []).map((fp: any) => ({
              uuid: fp.uuid ?? "",
              reference: fp.reference ?? "",
              value: fp.value ?? "",
              footprintId: fp.footprint_id ?? "",
              position: p(fp.position),
              rotation: fp.rotation ?? 0,
              layer: fp.layer ?? "F.Cu",
              locked: fp.locked ?? false,
              pads: (fp.pads ?? []).map((pad: any) => ({
                uuid: pad.uuid ?? "",
                number: pad.number ?? "",
                type: pad.pad_type ?? "smd",
                shape: pad.shape ?? "rect",
                position: p(pad.position),
                size: [pad.size?.[0] ?? 1, pad.size?.[1] ?? 1] as [number, number],
                drill: pad.drill ? { diameter: pad.drill.diameter ?? 0 } : undefined,
                layers: pad.layers ?? [],
                net: pad.net ? { number: pad.net.number ?? 0, name: pad.net.name ?? "" } : undefined,
                roundrectRatio: pad.roundrect_ratio,
              })),
              graphics: (fp.graphics ?? []).map((g: any) => {
                const gt = g.graphic_type ?? "line";
                const layer = g.layer ?? "F.SilkS";
                const width = g.width ?? 0.12;
                if (gt === "circle") {
                  return { type: "circle" as const, center: p(g.center), radius: g.radius ?? 0, layer, width, fill: g.fill ?? undefined };
                } else if (gt === "text") {
                  return { type: "text" as const, text: g.text ?? "", position: p(g.position), layer, fontSize: g.font_size ?? 1.0, rotation: g.rotation ?? 0 };
                } else if (gt === "arc") {
                  return { type: "arc" as const, start: p(g.start), mid: p(g.mid), end: p(g.end), layer, width };
                } else if (gt === "poly") {
                  return { type: "poly" as const, points: (g.points ?? []).map(p), layer, width, fill: g.fill ?? undefined };
                } else if (gt === "rect") {
                  return { type: "rect" as const, start: p(g.start), end: p(g.end), layer, width, fill: g.fill ?? undefined };
                } else {
                  return { type: "line" as const, start: p(g.start), end: p(g.end), layer, width };
                }
              }),
            })),
            segments: (raw.segments ?? []).map((s: any) => ({
              uuid: s.uuid ?? "",
              start: p(s.start),
              end: p(s.end),
              width: s.width ?? 0.25,
              layer: s.layer ?? "F.Cu",
              net: s.net ?? 0,
            })),
            vias: (raw.vias ?? []).map((v: any) => ({
              uuid: v.uuid ?? "",
              position: p(v.position),
              diameter: v.diameter ?? 0.8,
              drill: v.drill ?? 0.4,
              layers: v.layers ?? ["F.Cu", "B.Cu"],
              net: v.net ?? 0,
              type: (v.via_type ?? "through") as "through",
            })),
            zones: (raw.zones ?? []).map((z: any) => ({
              uuid: z.uuid ?? "",
              net: z.net ?? 0,
              netName: z.net_name ?? "",
              layer: z.layer ?? "F.Cu",
              outline: (z.outline ?? []).map(p),
              fillPolygons: (z.fill_polygons ?? []).map((poly: any) => (poly ?? []).map(p)),
              minThickness: z.min_thickness ?? 0.25,
              thermalGap: z.thermal_gap ?? 0.5,
              thermalBridgeWidth: z.thermal_bridge_width ?? 0.5,
              connectPads: z.connect_pads ?? "thermal_relief",
              priority: z.priority ?? 0,
            })),
            nets: (raw.nets ?? []).map((n: any) => ({
              number: n.number ?? 0,
              name: n.name ?? "",
            })),
            graphics: (raw.graphics ?? []).map((g: any) => {
              const gt = g.graphic_type ?? "line";
              const layer = g.layer ?? "Edge.Cuts";
              const width = g.width ?? 0.05;
              if (gt === "circle") {
                return { type: "circle" as const, center: p(g.center), radius: g.radius ?? 0, layer, width };
              } else if (gt === "rect") {
                return { type: "rect" as const, start: p(g.start), end: p(g.end), layer, width };
              } else if (gt === "arc") {
                // mid is stored in points[0] from Rust BoardGraphic
                const mid = g.points?.[0] ? p(g.points[0]) : p(g.start);
                return { type: "arc" as const, start: p(g.start), mid, end: p(g.end), layer, width };
              } else if (gt === "poly") {
                return { type: "poly" as const, points: (g.points ?? []).map(p), layer, width };
              } else {
                return { type: "line" as const, start: p(g.start), end: p(g.end), layer, width };
              }
            }),
            texts: (raw.texts ?? []).map((t: any) => ({
              uuid: t.uuid ?? "",
              text: t.text ?? "",
              position: p(t.position),
              layer: t.layer ?? "F.SilkS",
              fontSize: t.font_size ?? 1.0,
              rotation: t.rotation ?? 0,
            })),
            designRules: [],
          };
          usePcbStore.getState().loadPcb(pcbData);
          setPcbLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setPcbError(String(err));
          setPcbLoading(false);
        }
      });

    return () => { cancelled = true; };
  }, [data, project]);

  // Memoize ratsnest — only recompute when data changes, not every frame
  const ratsnestLines = useMemo(() => {
    if (!data) return [];
    return computeRatsnest(data);
  }, [data]);

  const s2w = useCallback((sx: number, sy: number): PcbPoint => {
    const cam = camRef.current;
    return { x: (sx - cam.x) / cam.zoom, y: (sy - cam.y) / cam.zoom };
  }, []);

  const getLayerColor = useCallback((layer: PcbLayerId, alpha = 1): string => {
    const hex = DEFAULT_LAYER_COLORS[layer] || "#808080";
    if (alpha >= 1) return hex;
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgba(${r},${g},${b},${alpha})`;
  }, []);

  // --- Render ---
  const render = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const cam = camRef.current;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    // Background
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, w, h);

    if (!data) {
      ctx.fillStyle = "#4a4a6a";
      ctx.font = "14px sans-serif";
      ctx.textAlign = "center";
      const msg = pcbError ? `Error: ${pcbError}` : pcbLoading ? "Loading PCB..." : "No PCB loaded";
      ctx.fillText(msg, w / 2, h / 2);
      return;
    }

    try {
    ctx.save();
    ctx.translate(cam.x, cam.y);
    ctx.scale(cam.zoom, cam.zoom);

    // Board flip: mirror horizontally when viewing from bottom
    if (boardFlipped) {
      const boardCenterX = data.board.outline.length > 0
        ? data.board.outline.reduce((s, p) => s + p.x, 0) / data.board.outline.length
        : w / 2 / cam.zoom;
      ctx.translate(boardCenterX, 0);
      ctx.scale(-1, 1);
      ctx.translate(-boardCenterX, 0);
    }

    // Grid
    if (gridVisible && cam.zoom > 1) {
      const gs = data.board.setup.gridSize || 1.27;
      const tl = s2w(0, 0);
      const br = s2w(w, h);
      ctx.globalAlpha = 0.2;
      ctx.fillStyle = "#4a4a6a";
      const startX = Math.floor(tl.x / gs) * gs;
      const startY = Math.floor(tl.y / gs) * gs;
      for (let x = startX; x <= br.x; x += gs) {
        for (let y = startY; y <= br.y; y += gs) {
          ctx.fillRect(x - 0.02, y - 0.02, 0.04, 0.04);
        }
      }
      ctx.globalAlpha = 1;
    }

    // Board outline
    if (data.board.outline.length >= 2) {
      ctx.strokeStyle = getLayerColor("Edge.Cuts");
      ctx.lineWidth = 0.15;
      ctx.beginPath();
      ctx.moveTo(data.board.outline[0].x, data.board.outline[0].y);
      for (let i = 1; i < data.board.outline.length; i++) {
        ctx.lineTo(data.board.outline[i].x, data.board.outline[i].y);
      }
      if (data.board.outline.length >= 3) {
        ctx.closePath();
        ctx.fillStyle = "#1e1e3a";
        ctx.fill();
      }
      ctx.stroke();
    }

    // Render layers back-to-front: B.Cu zones → B.Cu traces → ... → F.Cu traces → F.Cu zones → silk
    const layerOrder: PcbLayerId[] = [
      "B.Fab", "B.CrtYd", "B.SilkS", "B.Mask", "B.Paste", "B.Cu",
      ...Array.from({ length: 30 }, (_, i) => `In${i + 1}.Cu` as PcbLayerId).filter((l) => visibleLayers.has(l)),
      "F.Cu", "F.Mask", "F.Paste", "F.SilkS", "F.CrtYd", "F.Fab",
      "Edge.Cuts",
    ];

    for (const layer of layerOrder) {
      if (!visibleLayers.has(layer)) continue;
      const color = getLayerColor(layer);
      const isActive = layer === activeLayer;
      // Single layer mode: hide/grayscale/mono non-active layers
      let alpha = isActive ? 1.0 : 0.5;
      if (singleLayerMode === "hide" && !isActive) continue;
      if (singleLayerMode === "grayscale" && !isActive) alpha = 0.15;
      if (singleLayerMode === "monochrome" && !isActive) alpha = 0.08;

      // Zones on this layer
      for (const zone of data.zones) {
        if (zone.layer !== layer) continue;
        ctx.globalAlpha = alpha * 0.3;
        ctx.fillStyle = color;
        if (zone.outline.length >= 3) {
          ctx.beginPath();
          ctx.moveTo(zone.outline[0].x, zone.outline[0].y);
          for (let i = 1; i < zone.outline.length; i++) ctx.lineTo(zone.outline[i].x, zone.outline[i].y);
          ctx.closePath();
          ctx.fill();
        }
        ctx.globalAlpha = 1;
      }

      // Trace segments on this layer
      ctx.strokeStyle = color;
      ctx.lineCap = "round";
      for (const seg of data.segments) {
        if (seg.layer !== layer) continue;
        const sel = selectedIds.has(seg.uuid);
        ctx.globalAlpha = alpha;
        const segColor = (netColorEnabled && netColors[seg.net]) ? netColors[seg.net] : color;
        ctx.strokeStyle = sel ? "#00e5ff" : segColor;
        ctx.lineWidth = seg.width;
        ctx.beginPath();
        ctx.moveTo(seg.start.x, seg.start.y);
        ctx.lineTo(seg.end.x, seg.end.y);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;

      // Footprint pads on this layer
      for (const fp of data.footprints) {
        if (!fp.position) continue;
        for (const pad of fp.pads) {
          if (!pad.position || !pad.size || !pad.layers) continue;
          if (!pad.layers.includes(layer) && !pad.layers.includes("*.Cu")) continue;
          const px = fp.position.x + pad.position.x;
          const py = fp.position.y + pad.position.y;
          const sel = selectedIds.has(fp.uuid);

          ctx.fillStyle = sel ? "#00e5ff" : color;
          ctx.globalAlpha = alpha;

          if (pad.shape === "circle") {
            ctx.beginPath();
            ctx.arc(px, py, pad.size[0] / 2, 0, Math.PI * 2);
            ctx.fill();
          } else if (pad.shape === "rect" || pad.shape === "roundrect") {
            const hw = pad.size[0] / 2, hh = pad.size[1] / 2;
            if (pad.shape === "roundrect" && pad.roundrectRatio) {
              const r = Math.min(hw, hh) * (pad.roundrectRatio || 0.25);
              ctx.beginPath();
              ctx.moveTo(px - hw + r, py - hh);
              ctx.lineTo(px + hw - r, py - hh);
              ctx.arcTo(px + hw, py - hh, px + hw, py - hh + r, r);
              ctx.lineTo(px + hw, py + hh - r);
              ctx.arcTo(px + hw, py + hh, px + hw - r, py + hh, r);
              ctx.lineTo(px - hw + r, py + hh);
              ctx.arcTo(px - hw, py + hh, px - hw, py + hh - r, r);
              ctx.lineTo(px - hw, py - hh + r);
              ctx.arcTo(px - hw, py - hh, px - hw + r, py - hh, r);
              ctx.fill();
            } else {
              ctx.fillRect(px - hw, py - hh, pad.size[0], pad.size[1]);
            }
          } else if (pad.shape === "oval") {
            const hw = pad.size[0] / 2, hh = pad.size[1] / 2;
            ctx.beginPath();
            ctx.ellipse(px, py, hw, hh, 0, 0, Math.PI * 2);
            ctx.fill();
          }

          // Drill hole
          if (pad.drill && pad.type === "thru_hole") {
            ctx.fillStyle = "#1a1a2e";
            ctx.beginPath();
            ctx.arc(px, py, pad.drill.diameter / 2, 0, Math.PI * 2);
            ctx.fill();
          }

          ctx.globalAlpha = 1;
        }
      }

      // Footprint graphics on this layer
      for (const fp of data.footprints) {
        if (!fp.position || !fp.graphics) continue;
        for (const g of fp.graphics) {
          if (g.layer !== layer) continue;
          ctx.strokeStyle = color;
          ctx.lineWidth = g.type === "text" ? 0.1 : ("width" in g ? g.width : 0.1);
          ctx.globalAlpha = alpha;

          const ox = fp.position.x, oy = fp.position.y;
          const rot = fp.rotation || 0;

          // Apply footprint rotation around its origin
          if (rot !== 0) {
            ctx.save();
            ctx.translate(ox, oy);
            ctx.rotate((rot * Math.PI) / 180);
            // Draw relative to (0,0) since we translated
            if (g.type === "line") {
              ctx.beginPath();
              ctx.moveTo(g.start.x, g.start.y);
              ctx.lineTo(g.end.x, g.end.y);
              ctx.stroke();
            } else if (g.type === "rect") {
              if ("fill" in g && g.fill) {
                ctx.fillStyle = color;
                ctx.fillRect(g.start.x, g.start.y, g.end.x - g.start.x, g.end.y - g.start.y);
              }
              ctx.strokeRect(g.start.x, g.start.y, g.end.x - g.start.x, g.end.y - g.start.y);
            } else if (g.type === "circle") {
              ctx.beginPath();
              ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
              if ("fill" in g && g.fill) { ctx.fillStyle = color; ctx.fill(); }
              ctx.stroke();
            } else if (g.type === "poly" && "points" in g && g.points.length >= 2) {
              ctx.beginPath();
              ctx.moveTo(g.points[0].x, g.points[0].y);
              for (let i = 1; i < g.points.length; i++) ctx.lineTo(g.points[i].x, g.points[i].y);
              ctx.closePath();
              if ("fill" in g && g.fill) { ctx.fillStyle = color; ctx.fill(); }
              ctx.stroke();
            } else if (g.type === "text") {
              ctx.fillStyle = color;
              ctx.font = `${g.fontSize}px sans-serif`;
              ctx.textAlign = "center";
              ctx.textBaseline = "middle";
              const textRot = ("rotation" in g ? g.rotation : 0) || 0;
              if (textRot !== 0) {
                ctx.save();
                ctx.translate(g.position.x, g.position.y);
                ctx.rotate((textRot * Math.PI) / 180);
                ctx.fillText(g.text === "%R" ? fp.reference : g.text === "%V" ? fp.value : g.text, 0, 0);
                ctx.restore();
              } else {
                ctx.fillText(g.text === "%R" ? fp.reference : g.text === "%V" ? fp.value : g.text, g.position.x, g.position.y);
              }
            }
            ctx.restore();
          } else {
            // No rotation — draw with offset
            if (g.type === "line") {
              ctx.beginPath();
              ctx.moveTo(ox + g.start.x, oy + g.start.y);
              ctx.lineTo(ox + g.end.x, oy + g.end.y);
              ctx.stroke();
            } else if (g.type === "rect") {
              if ("fill" in g && g.fill) {
                ctx.fillStyle = color;
                ctx.fillRect(ox + g.start.x, oy + g.start.y, g.end.x - g.start.x, g.end.y - g.start.y);
              }
              ctx.strokeRect(ox + g.start.x, oy + g.start.y, g.end.x - g.start.x, g.end.y - g.start.y);
            } else if (g.type === "circle") {
              ctx.beginPath();
              ctx.arc(ox + g.center.x, oy + g.center.y, g.radius, 0, Math.PI * 2);
              if ("fill" in g && g.fill) { ctx.fillStyle = color; ctx.fill(); }
              ctx.stroke();
            } else if (g.type === "poly" && "points" in g && g.points.length >= 2) {
              ctx.beginPath();
              ctx.moveTo(ox + g.points[0].x, oy + g.points[0].y);
              for (let i = 1; i < g.points.length; i++) ctx.lineTo(ox + g.points[i].x, oy + g.points[i].y);
              ctx.closePath();
              if ("fill" in g && g.fill) { ctx.fillStyle = color; ctx.fill(); }
              ctx.stroke();
            } else if (g.type === "text") {
              ctx.fillStyle = color;
              ctx.font = `${g.fontSize}px sans-serif`;
              ctx.textAlign = "center";
              ctx.textBaseline = "middle";
              const textRot = ("rotation" in g ? g.rotation : 0) || 0;
              if (textRot !== 0) {
                ctx.save();
                ctx.translate(ox + g.position.x, oy + g.position.y);
                ctx.rotate((textRot * Math.PI) / 180);
                ctx.fillText(g.text === "%R" ? fp.reference : g.text === "%V" ? fp.value : g.text, 0, 0);
                ctx.restore();
              } else {
                ctx.fillText(g.text === "%R" ? fp.reference : g.text === "%V" ? fp.value : g.text, ox + g.position.x, oy + g.position.y);
              }
            }
          }
          ctx.globalAlpha = 1;
        }
      }

      // Board-level graphics on this layer
      for (const g of data.graphics) {
        if (g.layer !== layer) continue;
        ctx.strokeStyle = color;
        ctx.lineWidth = "width" in g ? g.width : 0.1;
        ctx.globalAlpha = alpha;
        if (g.type === "line") {
          ctx.beginPath();
          ctx.moveTo(g.start.x, g.start.y);
          ctx.lineTo(g.end.x, g.end.y);
          ctx.stroke();
        } else if (g.type === "rect") {
          ctx.strokeRect(g.start.x, g.start.y, g.end.x - g.start.x, g.end.y - g.start.y);
        } else if (g.type === "circle") {
          ctx.beginPath();
          ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
          ctx.stroke();
        } else if (g.type === "arc" && "mid" in g) {
          // Approximate arc through 3 points
          ctx.beginPath();
          ctx.moveTo(g.start.x, g.start.y);
          ctx.quadraticCurveTo(g.mid.x, g.mid.y, g.end.x, g.end.y);
          ctx.stroke();
        } else if (g.type === "poly" && "points" in g && g.points.length >= 2) {
          ctx.beginPath();
          ctx.moveTo(g.points[0].x, g.points[0].y);
          for (let i = 1; i < g.points.length; i++) ctx.lineTo(g.points[i].x, g.points[i].y);
          ctx.closePath();
          ctx.stroke();
        }
        ctx.globalAlpha = 1;
      }

      // Board-level texts on this layer
      for (const t of data.texts) {
        if (t.layer !== layer) continue;
        ctx.fillStyle = color;
        ctx.globalAlpha = alpha;
        const fs = t.fontSize || 1.0;
        ctx.font = `${fs}px sans-serif`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        if (t.rotation) {
          ctx.save();
          ctx.translate(t.position.x, t.position.y);
          ctx.rotate((t.rotation * Math.PI) / 180);
          ctx.fillText(t.text, 0, 0);
          ctx.restore();
        } else {
          ctx.fillText(t.text, t.position.x, t.position.y);
        }
        ctx.globalAlpha = 1;
      }
    }

    // Vias (render on top of all copper)
    for (const via of data.vias) {
      const sel = selectedIds.has(via.uuid);
      ctx.fillStyle = sel ? "#00e5ff" : "#c0c0c0";
      ctx.beginPath();
      ctx.arc(via.position.x, via.position.y, via.diameter / 2, 0, Math.PI * 2);
      ctx.fill();
      // Drill
      ctx.fillStyle = "#1a1a2e";
      ctx.beginPath();
      ctx.arc(via.position.x, via.position.y, via.drill / 2, 0, Math.PI * 2);
      ctx.fill();
    }

    // Ratsnest (unrouted connections) — uses memoized computation
    if (ratsnestLines.length > 0) {
      ctx.strokeStyle = "#ffff00";
      ctx.lineWidth = 0.05;
      ctx.setLineDash([0.3, 0.2]);
      ctx.globalAlpha = 0.6;
      for (const line of ratsnestLines) {
        const posA = getPadPosition(data, line.padA);
        const posB = getPadPosition(data, line.padB);
        if (posA && posB) {
          ctx.beginPath();
          ctx.moveTo(posA.x, posA.y);
          ctx.lineTo(posB.x, posB.y);
          ctx.stroke();
        }
      }
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }

    // Routing preview
    if (routingActive && routingPoints.length > 0) {
      ctx.strokeStyle = getLayerColor(usePcbStore.getState().routingLayer);
      ctx.lineWidth = usePcbStore.getState().routingWidth;
      ctx.lineCap = "round";
      ctx.setLineDash([0.5, 0.3]);
      ctx.globalAlpha = 0.7;
      ctx.beginPath();
      ctx.moveTo(routingPoints[0].x, routingPoints[0].y);
      for (let i = 1; i < routingPoints.length; i++) {
        ctx.lineTo(routingPoints[i].x, routingPoints[i].y);
      }
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }

    // Selection highlight
    for (const fp of data.footprints) {
      if (!selectedIds.has(fp.uuid) || !fp.position) continue;
      ctx.strokeStyle = "#00e5ff";
      ctx.lineWidth = 0.15;
      ctx.setLineDash([0.3, 0.2]);
      // Simple bbox
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const pad of fp.pads) {
        if (!pad.position || !pad.size) continue;
        const px = fp.position.x + pad.position.x;
        const py = fp.position.y + pad.position.y;
        minX = Math.min(minX, px - pad.size[0]); maxX = Math.max(maxX, px + pad.size[0]);
        minY = Math.min(minY, py - pad.size[1]); maxY = Math.max(maxY, py + pad.size[1]);
      }
      if (isFinite(minX)) {
        ctx.strokeRect(minX - 0.5, minY - 0.5, maxX - minX + 1, maxY - minY + 1);
      }
      ctx.setLineDash([]);
    }

    ctx.restore();
    } catch (renderErr) {
      console.error("PCB render error:", renderErr);
      ctx.restore();
    }

    // Status overlay
    ctx.fillStyle = "#cdd6f4";
    ctx.font = "11px sans-serif";
    ctx.textAlign = "left";
    const unrouted = ratsnestLines.length;
    ctx.fillText(
      `Layer: ${activeLayer} | Mode: ${editMode} | Zoom: ${(cam.zoom * 100).toFixed(0)}% | ` +
      `Nets: ${data.nets.length} | Unrouted: ${unrouted} | Segments: ${data.segments.length} | Vias: ${data.vias.length}`,
      10, h - 10
    );
  }, [data, selectedIds, activeLayer, visibleLayers, editMode, routingActive, routingPoints, gridVisible, s2w, getLayerColor, singleLayerMode, boardFlipped, netColorEnabled, netColors, ratsnestLines, pcbLoading, pcbError]);

  // --- Resize ---
  useEffect(() => {
    const container = containerRef.current;
    const canvas = canvasRef.current;
    if (!container || !canvas) return;

    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = container.getBoundingClientRect();
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      canvas.style.width = rect.width + "px";
      canvas.style.height = rect.height + "px";
      if (!data) {
        camRef.current.x = rect.width / 2;
        camRef.current.y = rect.height / 2;
      } else if (!(camRef.current as any)._fitted) {
        // Auto-fit to board bounds on first load
        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        for (const p of data.board.outline) { minX = Math.min(minX, p.x); minY = Math.min(minY, p.y); maxX = Math.max(maxX, p.x); maxY = Math.max(maxY, p.y); }
        // Fallback: use footprint positions if no outline
        if (!isFinite(minX)) {
          for (const fp of data.footprints) { if (!fp.position) continue; minX = Math.min(minX, fp.position.x - 5); minY = Math.min(minY, fp.position.y - 5); maxX = Math.max(maxX, fp.position.x + 5); maxY = Math.max(maxY, fp.position.y + 5); }
        }
        if (isFinite(minX)) {
          const bw = maxX - minX, bh = maxY - minY;
          const pad = Math.max(bw, bh) * 0.1;
          const zoom = Math.min(rect.width / (bw + pad * 2), rect.height / (bh + pad * 2));
          camRef.current.zoom = zoom;
          camRef.current.x = rect.width / 2 - (minX + bw / 2) * zoom;
          camRef.current.y = rect.height / 2 - (minY + bh / 2) * zoom;
          (camRef.current as any)._fitted = true;
        }
      }
      render();
    };

    const ro = new ResizeObserver(resize);
    ro.observe(container);
    resize();
    return () => ro.disconnect();
  }, [render, data]);

  // Animation loop
  useEffect(() => {
    const loop = () => {
      render();
      animRef.current = requestAnimationFrame(loop);
    };
    animRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(animRef.current);
  }, [render]);

  // --- Mouse handlers ---
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (ctxMenu) setCtxMenu(null);

    // Middle button = pan
    if (e.button === 1) {
      dragging.current = true;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      e.preventDefault();
      return;
    }

    // Right click = context menu or pan
    if (e.button === 2) {
      e.preventDefault();
      const store = usePcbStore.getState();

      if (store.routingActive) {
        store.finishRoute();
        return;
      }

      if (store.editMode !== "select") {
        store.setEditMode("select");
        return;
      }

      // Build context menu
      const items: ContextMenuItem[] = [];
      const sel = store.selectedIds;

      if (sel.size > 0) {
        items.push({ label: "Delete", shortcut: "Del", action: () => store.deleteSelected() });
        items.push({ label: "Rotate 90\u00b0", shortcut: "Space", action: () => { for (const id of sel) store.rotateFootprint(id, 90); } });
        items.push({ label: "Flip Side", shortcut: "F", action: () => { for (const id of sel) store.flipFootprint(id); } });
        items.push({ separator: true, label: "", action: () => {} });
        items.push({ label: "Select All", shortcut: "Ctrl+A", action: () => store.selectAll() });
      } else {
        items.push({ label: "Select All", shortcut: "Ctrl+A", action: () => store.selectAll() });
        items.push({ separator: true, label: "", action: () => {} });
        items.push({ label: "Route Track", shortcut: "X", action: () => store.setEditMode("routeTrack") });
        items.push({ label: "Place Via", action: () => store.setEditMode("placeVia") });
        items.push({ label: "Board Outline", action: () => store.setEditMode("drawBoardOutline") });
        items.push({ label: "Place Zone", action: () => store.setEditMode("placeZone") });
      }

      if (items.length > 0) {
        setCtxMenu({ x: e.clientX, y: e.clientY, items });
        return;
      }

      // Fallback: pan
      dragging.current = true;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      return;
    }

    if (e.button === 0 && data) {
      const r = canvasRef.current?.getBoundingClientRect();
      if (!r) return;
      const world = s2w(e.clientX - r.left, e.clientY - r.top);
      const store = usePcbStore.getState();

      if (store.editMode === "routeTrack") {
        if (store.routingActive) {
          store.addRoutePoint(world);
        } else {
          // Find net from nearest pad
          let nearestNet = 0;
          for (const fp of data.footprints) {
            if (!fp.position) continue;
            for (const pad of fp.pads) {
              if (!pad.net || !pad.position || !pad.size) continue;
              const px = fp.position.x + pad.position.x;
              const py = fp.position.y + pad.position.y;
              const d = Math.hypot(world.x - px, world.y - py);
              if (d < Math.max(pad.size[0], pad.size[1])) {
                nearestNet = pad.net.number;
              }
            }
          }
          store.startRoute(world, nearestNet);
        }
        return;
      }

      // Board outline drawing
      if (store.editMode === "drawBoardOutline") {
        const outline = [...(store.data?.board.outline || []), world];
        store.setBoardOutline(outline);
        return;
      }

      // Simple footprint hit test
      for (const fp of data.footprints) {
        if (!fp.position) continue;
        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        for (const pad of fp.pads) {
          if (!pad.position || !pad.size) continue;
          const px = fp.position.x + pad.position.x;
          const py = fp.position.y + pad.position.y;
          minX = Math.min(minX, px - pad.size[0] / 2); maxX = Math.max(maxX, px + pad.size[0] / 2);
          minY = Math.min(minY, py - pad.size[1] / 2); maxY = Math.max(maxY, py + pad.size[1] / 2);
        }
        if (world.x >= minX && world.x <= maxX && world.y >= minY && world.y <= maxY) {
          if (e.shiftKey) store.toggleSelect(fp.uuid);
          else store.select(fp.uuid);
          return;
        }
      }

      // Segment hit test
      const hitTol = 1.0 / camRef.current.zoom;
      for (const seg of data.segments) {
        const dx = seg.end.x - seg.start.x, dy = seg.end.y - seg.start.y;
        const lenSq = dx * dx + dy * dy;
        if (lenSq === 0) continue;
        let t = ((world.x - seg.start.x) * dx + (world.y - seg.start.y) * dy) / lenSq;
        t = Math.max(0, Math.min(1, t));
        const nearest = { x: seg.start.x + t * dx, y: seg.start.y + t * dy };
        if (Math.hypot(world.x - nearest.x, world.y - nearest.y) < seg.width / 2 + hitTol) {
          if (e.shiftKey) store.toggleSelect(seg.uuid);
          else store.select(seg.uuid);
          return;
        }
      }

      // Via hit test
      for (const via of data.vias) {
        if (Math.hypot(world.x - via.position.x, world.y - via.position.y) < via.diameter / 2 + hitTol) {
          if (e.shiftKey) store.toggleSelect(via.uuid);
          else store.select(via.uuid);
          return;
        }
      }

      store.deselectAll();
    }
  }, [data, s2w]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (dragging.current) {
      camRef.current.x += e.clientX - lastMouse.current.x;
      camRef.current.y += e.clientY - lastMouse.current.y;
      lastMouse.current = { x: e.clientX, y: e.clientY };
    }
  }, []);

  const handleMouseUp = useCallback(() => {
    dragging.current = false;
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const r = canvasRef.current?.getBoundingClientRect();
    if (!r) return;
    const sx = e.clientX - r.left, sy = e.clientY - r.top;
    const cam = camRef.current;
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    const newZoom = Math.max(0.1, Math.min(500, cam.zoom * factor));
    cam.x = sx - (sx - cam.x) * (newZoom / cam.zoom);
    cam.y = sy - (sy - cam.y) * (newZoom / cam.zoom);
    cam.zoom = newZoom;
  }, []);

  // Keyboard
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const store = usePcbStore.getState();

      switch (e.key) {
        case "Escape":
          if (store.routingActive) store.cancelRoute();
          else store.setEditMode("select");
          break;
        case "Delete":
          store.deleteSelected();
          break;
        case " ":
          if (store.selectedIds.size > 0) {
            e.preventDefault();
            for (const id of store.selectedIds) store.rotateFootprint(id, 90);
          }
          break;
        case "f":
        case "F":
          if (e.ctrlKey) {
            e.preventDefault();
            store.toggleBoardFlip();
          } else if (store.selectedIds.size > 0) {
            for (const id of store.selectedIds) store.flipFootprint(id);
          }
          break;
        case "s":
        case "S":
          if (e.shiftKey && !e.ctrlKey) {
            e.preventDefault();
            store.cycleSingleLayerMode();
          }
          break;
        case "x":
        case "X":
          if (!e.ctrlKey) store.setEditMode("routeTrack");
          break;
        case "+":
        case "=":
          store.setActiveLayer(store.activeLayer === "F.Cu" ? "B.Cu" : "F.Cu");
          break;
        case "F5":
          store.toggleNetColors();
          break;
        case "z":
          if (e.ctrlKey) { e.preventDefault(); store.undo(); }
          break;
        case "y":
          if (e.ctrlKey) { e.preventDefault(); store.redo(); }
          break;
        case "a":
          if (e.ctrlKey) { e.preventDefault(); store.selectAll(); }
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div ref={containerRef} className="w-full h-full relative bg-[#1a1a2e]">
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
        onContextMenu={(e) => e.preventDefault()}
        className="absolute inset-0"
        style={{ cursor: editMode !== "select" ? "crosshair" : "default" }}
      />
      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={ctxMenu.items} onClose={() => setCtxMenu(null)} />}

      {/* Altium-style floating Active Bar — centered top */}
      <div className="absolute top-3 left-1/2 -translate-x-1/2 flex items-center gap-0.5 bg-bg-surface/90 backdrop-blur-sm border border-border-subtle rounded-lg px-1.5 py-1 shadow-lg shadow-black/30 z-20">
        <PcbCanvasBtn active={editMode === "select"} label="Select (Esc)" onClick={() => usePcbStore.getState().setEditMode("select")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.51L3 3z"/></svg>
        </PcbCanvasBtn>
        <div className="w-px h-4 bg-border-subtle mx-0.5" />
        <PcbCanvasBtn active={editMode === "routeTrack"} label="Route Track (X)" onClick={() => usePcbStore.getState().setEditMode("routeTrack")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M4 20L12 12L20 4"/></svg>
        </PcbCanvasBtn>
        <PcbCanvasBtn active={editMode === "placeVia"} label="Place Via" onClick={() => usePcbStore.getState().setEditMode("placeVia")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="2.5"/></svg>
        </PcbCanvasBtn>
        <div className="w-px h-4 bg-border-subtle mx-0.5" />
        <PcbCanvasBtn active={editMode === "drawBoardOutline"} label="Board Outline" onClick={() => usePcbStore.getState().setEditMode("drawBoardOutline")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="3" width="18" height="18" rx="2"/></svg>
        </PcbCanvasBtn>
        <PcbCanvasBtn active={editMode === "placeZone"} label="Copper Pour" onClick={() => usePcbStore.getState().setEditMode("placeZone")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 4h16v16H4z" fill="currentColor" opacity="0.15"/><rect x="4" y="4" width="16" height="16"/></svg>
        </PcbCanvasBtn>
        <PcbCanvasBtn active={editMode === "placeText"} label="Text" onClick={() => usePcbStore.getState().setEditMode("placeText")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 7V4h16v3"/><path d="M12 4v16"/><path d="M8 20h8"/></svg>
        </PcbCanvasBtn>
        <div className="w-px h-4 bg-border-subtle mx-0.5" />
        {/* Layer indicator */}
        <div className="flex items-center gap-1 px-1.5">
          <div className="w-2.5 h-2.5 rounded-sm" style={{ backgroundColor: DEFAULT_LAYER_COLORS[activeLayer] || "#808080" }} />
          <span className="text-[9px] text-text-muted/70 font-mono">{LAYER_DISPLAY_NAMES[activeLayer] || activeLayer}</span>
        </div>
      </div>
    </div>
  );
}

function PcbCanvasBtn({ children, label, active, onClick }: {
  children: React.ReactNode; label: string; active?: boolean; onClick: () => void;
}) {
  return (
    <button title={label} onClick={onClick}
      className={`p-1.5 rounded transition-colors ${
        active ? "bg-accent/25 text-accent" : "text-text-muted/60 hover:bg-bg-hover hover:text-text-primary"
      }`}>
      {children}
    </button>
  );
}
