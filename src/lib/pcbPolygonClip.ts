/**
 * Polygon Boolean Operations for Copper Pour Fill
 *
 * Implements Sutherland-Hodgman polygon clipping and simple polygon
 * difference for zone fill computation. This is a simplified alternative
 * to the full Clipper library — handles convex clip regions only.
 *
 * For production use, consider integrating a proper polygon clipping library
 * like clipper2 (via WASM) for complex concave polygon operations.
 */

import type { PcbPoint } from "@/types/pcb";

/**
 * Clip a subject polygon by a convex clip polygon (Sutherland-Hodgman).
 * Returns the intersection of the two polygons.
 */
export function clipPolygon(subject: PcbPoint[], clip: PcbPoint[]): PcbPoint[] {
  if (subject.length < 3 || clip.length < 3) return [];

  let output = [...subject];

  for (let i = 0; i < clip.length; i++) {
    if (output.length === 0) return [];

    const input = [...output];
    output = [];

    const edgeStart = clip[i];
    const edgeEnd = clip[(i + 1) % clip.length];

    for (let j = 0; j < input.length; j++) {
      const current = input[j];
      const prev = input[(j + input.length - 1) % input.length];

      const currentInside = isInside(current, edgeStart, edgeEnd);
      const prevInside = isInside(prev, edgeStart, edgeEnd);

      if (currentInside) {
        if (!prevInside) {
          const inter = lineIntersection(prev, current, edgeStart, edgeEnd);
          if (inter) output.push(inter);
        }
        output.push(current);
      } else if (prevInside) {
        const inter = lineIntersection(prev, current, edgeStart, edgeEnd);
        if (inter) output.push(inter);
      }
    }
  }

  return output;
}

/**
 * Subtract a rectangular obstacle from a polygon.
 * Returns array of polygons (the result may split into multiple pieces).
 *
 * Simplified: only handles rectangular obstacles. For complex shapes,
 * use clipPolygon with the inverted obstacle.
 */
export function subtractRect(
  polygon: PcbPoint[],
  rectMin: PcbPoint,
  rectMax: PcbPoint,
): PcbPoint[][] {
  // Create clip regions for each side of the rectangle
  // This is a simplified approach — proper implementation would use
  // polygon boolean difference

  // Check if rectangle is fully outside polygon
  const polyBounds = getBounds(polygon);
  if (rectMin.x > polyBounds.maxX || rectMax.x < polyBounds.minX ||
      rectMin.y > polyBounds.maxY || rectMax.y < polyBounds.minY) {
    return [polygon]; // No intersection
  }

  // Check if rectangle fully contains polygon
  if (rectMin.x <= polyBounds.minX && rectMax.x >= polyBounds.maxX &&
      rectMin.y <= polyBounds.minY && rectMax.y >= polyBounds.maxY) {
    return []; // Fully subtracted
  }

  // Split polygon by cutting with each edge of the rectangle
  // Keep the parts outside the rectangle
  const results: PcbPoint[][] = [];

  // Left clip
  const leftClip: PcbPoint[] = [
    { x: polyBounds.minX - 1, y: polyBounds.minY - 1 },
    { x: rectMin.x, y: polyBounds.minY - 1 },
    { x: rectMin.x, y: polyBounds.maxY + 1 },
    { x: polyBounds.minX - 1, y: polyBounds.maxY + 1 },
  ];
  const leftPart = clipPolygon(polygon, leftClip);
  if (leftPart.length >= 3) results.push(leftPart);

  // Right clip
  const rightClip: PcbPoint[] = [
    { x: rectMax.x, y: polyBounds.minY - 1 },
    { x: polyBounds.maxX + 1, y: polyBounds.minY - 1 },
    { x: polyBounds.maxX + 1, y: polyBounds.maxY + 1 },
    { x: rectMax.x, y: polyBounds.maxY + 1 },
  ];
  const rightPart = clipPolygon(polygon, rightClip);
  if (rightPart.length >= 3) results.push(rightPart);

  // Top clip (between left and right rect edges)
  const topClip: PcbPoint[] = [
    { x: rectMin.x, y: polyBounds.minY - 1 },
    { x: rectMax.x, y: polyBounds.minY - 1 },
    { x: rectMax.x, y: rectMin.y },
    { x: rectMin.x, y: rectMin.y },
  ];
  const topPart = clipPolygon(polygon, topClip);
  if (topPart.length >= 3) results.push(topPart);

  // Bottom clip
  const bottomClip: PcbPoint[] = [
    { x: rectMin.x, y: rectMax.y },
    { x: rectMax.x, y: rectMax.y },
    { x: rectMax.x, y: polyBounds.maxY + 1 },
    { x: rectMin.x, y: polyBounds.maxY + 1 },
  ];
  const bottomPart = clipPolygon(polygon, bottomClip);
  if (bottomPart.length >= 3) results.push(bottomPart);

  return results.length > 0 ? results : [polygon];
}

/**
 * Expand a polygon outward by a given offset (simplified Minkowski sum).
 * For convex polygons, offsets each edge outward.
 */
export function offsetPolygon(polygon: PcbPoint[], offset: number): PcbPoint[] {
  if (polygon.length < 3) return polygon;

  const result: PcbPoint[] = [];
  const n = polygon.length;

  for (let i = 0; i < n; i++) {
    const prev = polygon[(i + n - 1) % n];
    const curr = polygon[i];
    const next = polygon[(i + 1) % n];

    // Compute normals of adjacent edges
    const dx1 = curr.x - prev.x, dy1 = curr.y - prev.y;
    const len1 = Math.hypot(dx1, dy1) || 1;
    const nx1 = -dy1 / len1, ny1 = dx1 / len1;

    const dx2 = next.x - curr.x, dy2 = next.y - curr.y;
    const len2 = Math.hypot(dx2, dy2) || 1;
    const nx2 = -dy2 / len2, ny2 = dx2 / len2;

    // Average normal for the vertex
    const nx = (nx1 + nx2) / 2;
    const ny = (ny1 + ny2) / 2;
    const nlen = Math.hypot(nx, ny) || 1;

    result.push({
      x: curr.x + (nx / nlen) * offset,
      y: curr.y + (ny / nlen) * offset,
    });
  }

  return result;
}

/**
 * Shrink a polygon inward by offset. Same as offsetPolygon with negative offset.
 */
export function shrinkPolygon(polygon: PcbPoint[], offset: number): PcbPoint[] {
  return offsetPolygon(polygon, -offset);
}

/**
 * Check if a point is inside a polygon (ray casting).
 */
export function pointInPolygon(point: PcbPoint, polygon: PcbPoint[]): boolean {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const xi = polygon[i].x, yi = polygon[i].y;
    const xj = polygon[j].x, yj = polygon[j].y;
    if (((yi > point.y) !== (yj > point.y)) &&
        (point.x < (xj - xi) * (point.y - yi) / (yj - yi) + xi)) {
      inside = !inside;
    }
  }
  return inside;
}

/**
 * Compute polygon area (signed — positive for CCW, negative for CW).
 */
export function polygonArea(polygon: PcbPoint[]): number {
  let area = 0;
  for (let i = 0; i < polygon.length; i++) {
    const j = (i + 1) % polygon.length;
    area += polygon[i].x * polygon[j].y;
    area -= polygon[j].x * polygon[i].y;
  }
  return area / 2;
}

/**
 * Remove dead copper islands — polygons with no net connections.
 */
export function removeDeadCopper(
  fillPolygons: PcbPoint[][],
  padPositions: PcbPoint[],
  _tolerance: number = 0.5,
): PcbPoint[][] {
  return fillPolygons.filter((poly) => {
    // Keep polygon if any pad is inside it
    return padPositions.some((pad) => pointInPolygon(pad, poly));
  });
}

// --- Internal helpers ---

function isInside(point: PcbPoint, edgeStart: PcbPoint, edgeEnd: PcbPoint): boolean {
  return (edgeEnd.x - edgeStart.x) * (point.y - edgeStart.y) -
         (edgeEnd.y - edgeStart.y) * (point.x - edgeStart.x) >= 0;
}

function lineIntersection(
  p1: PcbPoint, p2: PcbPoint,
  p3: PcbPoint, p4: PcbPoint,
): PcbPoint | null {
  const x1 = p1.x, y1 = p1.y, x2 = p2.x, y2 = p2.y;
  const x3 = p3.x, y3 = p3.y, x4 = p4.x, y4 = p4.y;

  const denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
  if (Math.abs(denom) < 1e-10) return null;

  const t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / denom;
  return {
    x: x1 + t * (x2 - x1),
    y: y1 + t * (y2 - y1),
  };
}

function getBounds(polygon: PcbPoint[]): { minX: number; minY: number; maxX: number; maxY: number } {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const p of polygon) {
    minX = Math.min(minX, p.x); minY = Math.min(minY, p.y);
    maxX = Math.max(maxX, p.x); maxY = Math.max(maxY, p.y);
  }
  return { minX, minY, maxX, maxY };
}
