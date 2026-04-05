import type { SchematicData, SchPoint } from "@/types";

export interface HitResult {
  type: "symbol" | "wire" | "junction" | "label";
  uuid: string;
}

function dist(a: SchPoint, b: SchPoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function distToSegment(p: SchPoint, a: SchPoint, b: SchPoint): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) return dist(p, a);

  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));

  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/**
 * Hit test at world coordinates. Returns the topmost element under the cursor.
 * Priority: junctions → symbols → wires → labels
 */
export function hitTest(
  data: SchematicData,
  worldX: number,
  worldY: number,
  tolerance: number = 2.0
): HitResult | null {
  const p: SchPoint = { x: worldX, y: worldY };

  // Junctions first (small targets)
  for (const j of data.junctions) {
    if (dist(p, j.position) < tolerance * 0.5) {
      return { type: "junction", uuid: j.uuid };
    }
  }

  // Symbols — check bounding box around position
  // Use a rough estimate: symbol body is ~8mm wide
  for (const sym of data.symbols) {
    if (sym.is_power) continue; // Skip power symbols for selection
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) continue;

    // Estimate bounds from pin positions + body graphics
    let minX = -4, maxX = 4, minY = -4, maxY = 4;
    for (const pin of lib.pins) {
      minX = Math.min(minX, pin.position.x - 1);
      maxX = Math.max(maxX, pin.position.x + 1);
      minY = Math.min(minY, pin.position.y - 1);
      maxY = Math.max(maxY, pin.position.y + 1);
    }
    for (const g of lib.graphics) {
      if (g.type === "Rectangle") {
        minX = Math.min(minX, g.start.x, g.end.x);
        maxX = Math.max(maxX, g.start.x, g.end.x);
        minY = Math.min(minY, g.start.y, g.end.y);
        maxY = Math.max(maxY, g.start.y, g.end.y);
      }
    }

    // Transform bounds to schematic space (rough — use center + extent)
    const extent = Math.max(maxX - minX, maxY - minY) / 2 + tolerance;
    if (dist(p, sym.position) < extent) {
      return { type: "symbol", uuid: sym.uuid };
    }
  }

  // Wires
  for (const wire of data.wires) {
    if (distToSegment(p, wire.start, wire.end) < tolerance * 0.5) {
      return { type: "wire", uuid: wire.uuid };
    }
  }

  // Labels
  for (const label of data.labels) {
    if (dist(p, label.position) < tolerance) {
      return { type: "label", uuid: label.uuid };
    }
  }

  return null;
}
