/**
 * PCB Output Formats — pick-and-place, assembly, IPC-2581 (simplified).
 */

import type { PcbData } from "@/types/pcb";

/**
 * Generate pick-and-place (centroid) file for automated assembly.
 * CSV format with columns: Reference, Value, Package, X, Y, Rotation, Side
 */
export function generatePickAndPlace(data: PcbData): string {
  const lines: string[] = [];
  lines.push("Ref,Val,Package,PosX,PosY,Rot,Side");

  const sorted = [...data.footprints].sort((a, b) =>
    a.reference.localeCompare(b.reference, undefined, { numeric: true })
  );

  for (const fp of sorted) {
    const side = fp.layer === "B.Cu" ? "bottom" : "top";
    lines.push(
      `${csvEscape(fp.reference)},${csvEscape(fp.value)},${csvEscape(fp.footprintId)},` +
      `${fp.position.x.toFixed(4)},${fp.position.y.toFixed(4)},${fp.rotation.toFixed(1)},${side}`
    );
  }

  return lines.join("\n") + "\n";
}

/**
 * Generate assembly drawing data as SVG.
 * Shows component outlines, reference designators, and orientation marks.
 */
export function generateAssemblySvg(data: PcbData, side: "top" | "bottom"): string {
  const layer = side === "top" ? "F.Cu" : "B.Cu";
  const fabLayer = side === "top" ? "F.Fab" : "B.Fab";
  const silkLayer = side === "top" ? "F.SilkS" : "B.SilkS";

  // Compute board bounds
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const p of data.board.outline) {
    minX = Math.min(minX, p.x); minY = Math.min(minY, p.y);
    maxX = Math.max(maxX, p.x); maxY = Math.max(maxY, p.y);
  }
  if (!isFinite(minX)) { minX = 0; minY = 0; maxX = 100; maxY = 80; }

  const margin = 5;
  const w = maxX - minX + margin * 2;
  const h = maxY - minY + margin * 2;

  let svg = `<?xml version="1.0" encoding="UTF-8"?>\n`;
  svg += `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${minX - margin} ${minY - margin} ${w} ${h}" width="${w * 10}" height="${h * 10}">\n`;
  svg += `<style>text { font-family: sans-serif; font-size: 1px; fill: #333; text-anchor: middle; dominant-baseline: central; }</style>\n`;

  // Board outline
  if (data.board.outline.length >= 3) {
    const points = data.board.outline.map((p) => `${p.x},${p.y}`).join(" ");
    svg += `<polygon points="${points}" fill="#f5f5dc" stroke="#333" stroke-width="0.2"/>\n`;
  }

  // Footprints on this side
  for (const fp of data.footprints) {
    if (fp.layer !== layer) continue;

    // Draw courtyard/fab rectangle
    for (const g of fp.graphics) {
      if (g.layer !== fabLayer && g.layer !== silkLayer) continue;
      if (g.type === "rect" && g.start && g.end) {
        const x1 = fp.position.x + g.start.x;
        const y1 = fp.position.y + g.start.y;
        const x2 = fp.position.x + g.end.x;
        const y2 = fp.position.y + g.end.y;
        svg += `<rect x="${Math.min(x1, x2)}" y="${Math.min(y1, y2)}" width="${Math.abs(x2 - x1)}" height="${Math.abs(y2 - y1)}" fill="none" stroke="#666" stroke-width="0.1"/>\n`;
      }
      if (g.type === "line" && g.start && g.end) {
        svg += `<line x1="${fp.position.x + g.start.x}" y1="${fp.position.y + g.start.y}" x2="${fp.position.x + g.end.x}" y2="${fp.position.y + g.end.y}" stroke="#666" stroke-width="0.1"/>\n`;
      }
    }

    // Reference text
    svg += `<text x="${fp.position.x}" y="${fp.position.y}" font-size="0.8">${escapeXml(fp.reference)}</text>\n`;

    // Pin 1 marker
    if (fp.pads.length > 0) {
      const pad1 = fp.pads[0];
      const px = fp.position.x + pad1.position.x;
      const py = fp.position.y + pad1.position.y;
      svg += `<circle cx="${px}" cy="${py}" r="0.3" fill="red" opacity="0.5"/>\n`;
    }
  }

  svg += `</svg>\n`;
  return svg;
}

/**
 * Generate IPC-2581 (simplified XML format).
 * Real IPC-2581 is very complex — this generates the basic structure.
 */
export function generateIpc2581(data: PcbData): string {
  let xml = `<?xml version="1.0" encoding="UTF-8"?>\n`;
  xml += `<IPC-2581 revision="C">\n`;
  xml += `  <Content>\n`;

  // Board info
  xml += `    <Board name="board" thickness="${data.board.thickness}">\n`;

  // Layer stack
  for (const layer of data.board.layers.layers) {
    xml += `      <Layer name="${escapeXml(layer.name)}" type="${layer.type}"/>\n`;
  }
  xml += `    </Board>\n`;

  // Nets
  xml += `    <Nets>\n`;
  for (const net of data.nets) {
    if (net.number === 0) continue;
    xml += `      <Net name="${escapeXml(net.name)}" number="${net.number}"/>\n`;
  }
  xml += `    </Nets>\n`;

  // Components
  xml += `    <Components>\n`;
  for (const fp of data.footprints) {
    xml += `      <Component refDes="${escapeXml(fp.reference)}" ` +
      `value="${escapeXml(fp.value)}" ` +
      `package="${escapeXml(fp.footprintId)}" ` +
      `x="${fp.position.x}" y="${fp.position.y}" ` +
      `rotation="${fp.rotation}" side="${fp.layer === "B.Cu" ? "BOT" : "TOP"}">\n`;
    for (const pad of fp.pads) {
      const netName = pad.net?.name || "";
      xml += `        <Pin number="${escapeXml(pad.number)}" net="${escapeXml(netName)}"/>\n`;
    }
    xml += `      </Component>\n`;
  }
  xml += `    </Components>\n`;

  xml += `  </Content>\n`;
  xml += `</IPC-2581>\n`;
  return xml;
}

/**
 * Generate net inspector report — per-net routing statistics.
 */
export function generateNetReport(data: PcbData): { name: string; number: number; segments: number; vias: number; totalLength: number; routed: boolean }[] {
  const report: { name: string; number: number; segments: number; vias: number; totalLength: number; routed: boolean }[] = [];

  for (const net of data.nets) {
    if (net.number === 0) continue;
    const segs = data.segments.filter((s) => s.net === net.number);
    const vias = data.vias.filter((v) => v.net === net.number);
    const totalLength = segs.reduce((sum, s) =>
      sum + Math.hypot(s.end.x - s.start.x, s.end.y - s.start.y), 0
    );

    // Check if fully routed (all pads connected)
    const padCount = data.footprints.reduce((sum, fp) =>
      sum + fp.pads.filter((p) => p.net?.number === net.number).length, 0
    );

    report.push({
      name: net.name,
      number: net.number,
      segments: segs.length,
      vias: vias.length,
      totalLength: Math.round(totalLength * 1000) / 1000,
      routed: segs.length > 0 || padCount <= 1,
    });
  }

  return report.sort((a, b) => a.name.localeCompare(b.name));
}

// --- Helpers ---

function csvEscape(s: string): string {
  if (s.includes(",") || s.includes('"') || s.includes("\n")) {
    return `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

function escapeXml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}
