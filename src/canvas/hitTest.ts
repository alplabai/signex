import type { SchematicData, SchPoint } from "@/types";

export interface HitResult {
  type: "symbol" | "wire" | "junction" | "label";
  uuid: string;
}

interface Box {
  minX: number; minY: number;
  maxX: number; maxY: number;
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

function pointInBox(p: SchPoint, box: Box): boolean {
  return p.x >= box.minX && p.x <= box.maxX && p.y >= box.minY && p.y <= box.maxY;
}

function segmentIntersectsBox(a: SchPoint, b: SchPoint, box: Box): boolean {
  // Check if either endpoint is inside
  if (pointInBox(a, box) || pointInBox(b, box)) return true;
  // Check segment-edge intersections (simplified: check if segment crosses any box edge)
  const edges: [SchPoint, SchPoint][] = [
    [{ x: box.minX, y: box.minY }, { x: box.maxX, y: box.minY }],
    [{ x: box.maxX, y: box.minY }, { x: box.maxX, y: box.maxY }],
    [{ x: box.maxX, y: box.maxY }, { x: box.minX, y: box.maxY }],
    [{ x: box.minX, y: box.maxY }, { x: box.minX, y: box.minY }],
  ];
  for (const [c, d] of edges) {
    if (segmentsIntersect(a, b, c, d)) return true;
  }
  return false;
}

function segmentsIntersect(a: SchPoint, b: SchPoint, c: SchPoint, d: SchPoint): boolean {
  const cross = (o: SchPoint, p: SchPoint, q: SchPoint) =>
    (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
  const d1 = cross(c, d, a), d2 = cross(c, d, b);
  const d3 = cross(a, b, c), d4 = cross(a, b, d);
  if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
      ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) return true;
  return false;
}

/**
 * Select all objects within a drag box.
 * crossing=true: select if object intersects box (right-to-left drag)
 * crossing=false: select only if object is fully inside box (left-to-right drag)
 */
export function boxSelect(
  data: SchematicData,
  startX: number, startY: number,
  endX: number, endY: number,
): string[] {
  const crossing = endX < startX;
  const box: Box = {
    minX: Math.min(startX, endX), minY: Math.min(startY, endY),
    maxX: Math.max(startX, endX), maxY: Math.max(startY, endY),
  };

  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (sym.is_power) continue;
    if (crossing) {
      // Crossing: any part of symbol in box
      if (pointInBox(sym.position, box)) selected.push(sym.uuid);
    } else {
      // Inside: symbol center must be in box
      if (pointInBox(sym.position, box)) selected.push(sym.uuid);
    }
  }

  for (const wire of data.wires) {
    if (crossing) {
      if (segmentIntersectsBox(wire.start, wire.end, box)) selected.push(wire.uuid);
    } else {
      if (pointInBox(wire.start, box) && pointInBox(wire.end, box)) selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    if (pointInBox(label.position, box)) selected.push(label.uuid);
  }

  for (const j of data.junctions) {
    if (pointInBox(j.position, box)) selected.push(j.uuid);
  }

  return selected;
}
