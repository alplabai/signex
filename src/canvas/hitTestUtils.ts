import type { SchPoint, SchSymbol } from "@/types";
import type { SelectionFilter } from "@/stores/editor";

export type { SelectionFilter };

export interface HitResult {
  type: "symbol" | "wire" | "wireEndpoint" | "junction" | "label" | "noConnect" | "textNote" | "bus" | "busEntry" | "childSheet" | "drawing";
  uuid: string;
  endpoint?: "start" | "end"; // Only for wireEndpoint
}

// Map hit result types to filter keys
export const FILTER_MAP: Record<string, string> = {
  symbol: "components",
  wire: "wires",
  wireEndpoint: "wires",
  junction: "junctions",
  label: "labels",
  noConnect: "noConnects",
  textNote: "textNotes",
  bus: "buses",
  busEntry: "buses",
  childSheet: "sheetSymbols",
  drawing: "drawings",
};

export function isSelectable(type: string, filter?: SelectionFilter, isPower?: boolean): boolean {
  if (!filter) return true;
  if (isPower) return filter.powerPorts?.selectable !== false;
  const key = FILTER_MAP[type];
  if (!key) return true;
  return filter[key]?.selectable !== false;
}

export interface Box {
  minX: number; minY: number;
  maxX: number; maxY: number;
}

export function symToSch(lx: number, ly: number, sym: SchSymbol): SchPoint {
  const y = -ly; // Flip Y
  const rad = -(sym.rotation * Math.PI) / 180;
  const cos = Math.cos(rad), sin = Math.sin(rad);
  let rx = lx * cos - y * sin;
  let ry = lx * sin + y * cos;
  if (sym.mirror_x) ry = -ry;
  if (sym.mirror_y) rx = -rx;
  return { x: sym.position.x + rx, y: sym.position.y + ry };
}

export function dist(a: SchPoint, b: SchPoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

export function distToSegment(p: SchPoint, a: SchPoint, b: SchPoint): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) return dist(p, a);

  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));

  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

export function pointInBox(p: SchPoint, box: Box): boolean {
  return p.x >= box.minX && p.x <= box.maxX && p.y >= box.minY && p.y <= box.maxY;
}

export function segmentIntersectsBox(a: SchPoint, b: SchPoint, box: Box): boolean {
  if (pointInBox(a, box) || pointInBox(b, box)) return true;
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

export function segmentsIntersect(a: SchPoint, b: SchPoint, c: SchPoint, d: SchPoint): boolean {
  const cross = (o: SchPoint, p: SchPoint, q: SchPoint) =>
    (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
  const d1 = cross(c, d, a), d2 = cross(c, d, b);
  const d3 = cross(a, b, c), d4 = cross(a, b, d);
  if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
      ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) return true;
  return false;
}

export function pointInPolygon(p: SchPoint, polygon: SchPoint[]): boolean {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const xi = polygon[i].x, yi = polygon[i].y;
    const xj = polygon[j].x, yj = polygon[j].y;
    if (((yi > p.y) !== (yj > p.y)) && (p.x < (xj - xi) * (p.y - yi) / (yj - yi) + xi)) {
      inside = !inside;
    }
  }
  return inside;
}
