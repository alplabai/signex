/**
 * PCB Interactive Router — Advanced routing engine
 *
 * Implements Altium-style routing modes:
 * - Walkaround: route around obstacles
 * - Push/Shove: push existing traces to make room
 * - Ignore: route through obstacles (for manual override)
 *
 * Also handles:
 * - Differential pair routing with gap control
 * - Length tuning with meander patterns
 * - Teardrop generation for pad/via transitions
 * - Online DRC during routing
 */

import type { PcbData, PcbPoint, PcbSegment, PcbVia, PcbLayerId } from "@/types/pcb";

// ═══════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════

export type RouteMode = "ignore" | "walkaround" | "push" | "hug_push";
export type CornerStyle = "45" | "90" | "arc45" | "arc90" | "any";

export interface RouteOptions {
  mode: RouteMode;
  cornerStyle: CornerStyle;
  width: number;
  layer: PcbLayerId;
  net: number;
  clearance: number;
  viaDiameter: number;
  viaDrill: number;
}

export interface DiffPairOptions extends RouteOptions {
  gap: number; // Space between P and N traces
  netP: number; // Positive net
  netN: number; // Negative net
}

export interface LengthTuneOptions {
  targetLength: number; // Target trace length in mm
  tolerance: number; // Acceptable deviation in mm
  amplitudeMin: number; // Minimum meander amplitude
  amplitudeMax: number; // Maximum meander amplitude
  spacing: number; // Meander spacing
  style: "trombone" | "sawtooth" | "accordion";
}

export interface RouteResult {
  segments: PcbSegment[];
  vias: PcbVia[];
  violations: RouteViolation[];
}

export interface RouteViolation {
  type: "clearance" | "width" | "layer";
  message: string;
  position: PcbPoint;
}

// ═══════════════════════════════════════════════════════════════
// OBSTACLE MAP
// ═══════════════════════════════════════════════════════════════

interface Obstacle {
  type: "segment" | "pad" | "via" | "zone" | "outline";
  layer: PcbLayerId;
  net: number;
  bounds: { minX: number; minY: number; maxX: number; maxY: number };
  width: number;
  // Segment-specific
  start?: PcbPoint;
  end?: PcbPoint;
  // Pad/via-specific
  center?: PcbPoint;
  radius?: number;
}

function buildObstacleMap(data: PcbData, layer: PcbLayerId, excludeNet: number): Obstacle[] {
  const obstacles: Obstacle[] = [];

  // Existing segments on this layer (different net)
  for (const seg of data.segments) {
    if (seg.layer !== layer || seg.net === excludeNet) continue;
    const hw = seg.width / 2;
    obstacles.push({
      type: "segment", layer, net: seg.net,
      bounds: {
        minX: Math.min(seg.start.x, seg.end.x) - hw,
        minY: Math.min(seg.start.y, seg.end.y) - hw,
        maxX: Math.max(seg.start.x, seg.end.x) + hw,
        maxY: Math.max(seg.start.y, seg.end.y) + hw,
      },
      width: seg.width,
      start: seg.start,
      end: seg.end,
    });
  }

  // Pads on this layer (different net)
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (!pad.layers.includes(layer) && !pad.layers.includes("*.Cu" as PcbLayerId)) continue;
      const padNet = pad.net?.number || 0;
      if (padNet === excludeNet && padNet !== 0) continue;

      const px = fp.position.x + pad.position.x;
      const py = fp.position.y + pad.position.y;
      const r = Math.max(pad.size[0], pad.size[1]) / 2;

      obstacles.push({
        type: "pad", layer, net: padNet,
        bounds: { minX: px - r, minY: py - r, maxX: px + r, maxY: py + r },
        width: r * 2,
        center: { x: px, y: py },
        radius: r,
      });
    }
  }

  // Vias (different net)
  for (const via of data.vias) {
    if (via.net === excludeNet && via.net !== 0) continue;
    const r = via.diameter / 2;
    obstacles.push({
      type: "via", layer, net: via.net,
      bounds: { minX: via.position.x - r, minY: via.position.y - r, maxX: via.position.x + r, maxY: via.position.y + r },
      width: via.diameter,
      center: via.position,
      radius: r,
    });
  }

  return obstacles;
}

// ═══════════════════════════════════════════════════════════════
// ROUTING ALGORITHMS
// ═══════════════════════════════════════════════════════════════

/**
 * Route a single trace from start to end with obstacle avoidance.
 */
export function routeTrace(
  data: PcbData,
  start: PcbPoint,
  end: PcbPoint,
  options: RouteOptions,
): RouteResult {
  const obstacles = buildObstacleMap(data, options.layer, options.net);
  const hw = options.width / 2;
  const totalClearance = hw + options.clearance;

  switch (options.mode) {
    case "ignore":
      return routeIgnore(start, end, options);
    case "walkaround":
      return routeWalkaround(start, end, options, obstacles, totalClearance);
    case "push":
    case "hug_push":
      return routePush(start, end, options, obstacles, totalClearance, data);
    default:
      return routeIgnore(start, end, options);
  }
}

/**
 * Ignore mode: straight line or 45-degree bend, no obstacle checking.
 */
function routeIgnore(start: PcbPoint, end: PcbPoint, options: RouteOptions): RouteResult {
  const segments = applyCornerStyle(start, end, options.cornerStyle, options);
  return { segments, vias: [], violations: [] };
}

/**
 * Walkaround mode: find a path around obstacles.
 * Uses a simplified approach: try direct path, if blocked, try going around via waypoints.
 */
function routeWalkaround(
  start: PcbPoint,
  end: PcbPoint,
  options: RouteOptions,
  obstacles: Obstacle[],
  totalClearance: number,
): RouteResult {
  // Try direct path first
  const directPath = applyCornerStyle(start, end, options.cornerStyle, options);
  if (!hasCollision(directPath, obstacles, totalClearance)) {
    return { segments: directPath, vias: [], violations: [] };
  }

  // Find blocking obstacles
  const blockers = findBlockingObstacles(start, end, obstacles, totalClearance);

  if (blockers.length === 0) {
    return { segments: directPath, vias: [], violations: [] };
  }

  // Try routing around each blocker (simplified: offset perpendicular)
  for (const side of [-1, 1]) {
    const waypoints = generateWaypoints(start, end, blockers, totalClearance, side);
    const segments: PcbSegment[] = [];
    let valid = true;

    for (let i = 0; i < waypoints.length - 1; i++) {
      const segs = applyCornerStyle(waypoints[i], waypoints[i + 1], options.cornerStyle, options);
      if (hasCollision(segs, obstacles, totalClearance)) {
        valid = false;
        break;
      }
      segments.push(...segs);
    }

    if (valid && segments.length > 0) {
      return { segments, vias: [], violations: [] };
    }
  }

  // Fallback: direct path with violations
  const violations = checkRouteViolations(directPath, obstacles, totalClearance);
  return { segments: directPath, vias: [], violations };
}

/**
 * Push/shove mode: push existing traces to make room.
 */
function routePush(
  start: PcbPoint,
  end: PcbPoint,
  options: RouteOptions,
  obstacles: Obstacle[],
  totalClearance: number,
  _data: PcbData,
): RouteResult {
  // First try walkaround
  const walkaroundResult = routeWalkaround(start, end, options, obstacles, totalClearance);
  if (walkaroundResult.violations.length === 0) {
    return walkaroundResult;
  }

  // If walkaround fails, use direct path
  // In a full implementation, we'd move conflicting segments
  const segments = applyCornerStyle(start, end, options.cornerStyle, options);
  const violations = checkRouteViolations(segments, obstacles, totalClearance);
  return { segments, vias: [], violations };
}

/**
 * Route a differential pair.
 */
export function routeDiffPair(
  data: PcbData,
  startP: PcbPoint,
  endP: PcbPoint,
  options: DiffPairOptions,
): { positive: RouteResult; negative: RouteResult } {
  const dx = endP.x - startP.x;
  const dy = endP.y - startP.y;
  const len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len;
  const ny = dx / len;
  const halfGap = (options.gap + options.width) / 2;

  // Offset start/end for negative trace
  const startN: PcbPoint = { x: startP.x + nx * halfGap, y: startP.y + ny * halfGap };
  const endN: PcbPoint = { x: endP.x + nx * halfGap, y: endP.y + ny * halfGap };

  // Offset positive trace the other way
  const startPOff: PcbPoint = { x: startP.x - nx * halfGap, y: startP.y - ny * halfGap };
  const endPOff: PcbPoint = { x: endP.x - nx * halfGap, y: endP.y - ny * halfGap };

  const positive = routeTrace(data, startPOff, endPOff, { ...options, net: options.netP });
  const negative = routeTrace(data, startN, endN, { ...options, net: options.netN });

  return { positive, negative };
}

/**
 * Generate length tuning meanders for a trace.
 */
export function generateLengthTuning(
  start: PcbPoint,
  end: PcbPoint,
  currentLength: number,
  options: LengthTuneOptions & { width: number; layer: PcbLayerId; net: number },
): PcbSegment[] {
  const deficit = options.targetLength - currentLength;
  if (deficit <= options.tolerance) {
    // Already long enough — straight line
    return [makeSegment(start, end, options.width, options.layer, options.net)];
  }

  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const directLen = Math.hypot(dx, dy);
  if (directLen < 1) return [makeSegment(start, end, options.width, options.layer, options.net)];

  // Calculate meander parameters
  const nx = -dy / directLen; // Normal direction
  const ny = dx / directLen;
  const tx = dx / directLen; // Tangent direction
  const ty = dy / directLen;

  // Number of meanders needed
  const amplitude = Math.min(options.amplitudeMax, Math.max(options.amplitudeMin, deficit / 4));
  const numMeanders = Math.max(1, Math.floor(deficit / (2 * amplitude)));
  const totalMeanderSpan = numMeanders * options.spacing;
  const startOffset = Math.max(0, (directLen - totalMeanderSpan) / 2);

  const segments: PcbSegment[] = [];
  let prevPoint = start;

  for (let i = 0; i < numMeanders; i++) {
    const t0 = startOffset + i * options.spacing;
    const t1 = t0 + options.spacing / 2;
    const t2 = t0 + options.spacing;

    const side = i % 2 === 0 ? 1 : -1;

    const p1: PcbPoint = {
      x: start.x + tx * t0,
      y: start.y + ty * t0,
    };
    const p2: PcbPoint = {
      x: start.x + tx * t1 + nx * amplitude * side,
      y: start.y + ty * t1 + ny * amplitude * side,
    };
    const p3: PcbPoint = {
      x: start.x + tx * t2,
      y: start.y + ty * t2,
    };

    segments.push(makeSegment(prevPoint, p1, options.width, options.layer, options.net));
    segments.push(makeSegment(p1, p2, options.width, options.layer, options.net));
    segments.push(makeSegment(p2, p3, options.width, options.layer, options.net));
    prevPoint = p3;
  }

  segments.push(makeSegment(prevPoint, end, options.width, options.layer, options.net));
  return segments;
}

/**
 * Generate teardrops for all pad-to-trace and via-to-trace junctions.
 */
export function generateTeardrops(
  data: PcbData,
  teardropRatio: number = 0.5, // Width ratio (0-1)
  teardropLength: number = 0.5, // Length in mm
): PcbSegment[] {
  const teardrops: PcbSegment[] = [];

  // For each pad, find connected trace segments
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (!pad.net || pad.net.number === 0) continue;
      const px = fp.position.x + pad.position.x;
      const py = fp.position.y + pad.position.y;
      const padR = Math.max(pad.size[0], pad.size[1]) / 2;

      // Find segments connected to this pad
      for (const seg of data.segments) {
        if (seg.net !== pad.net.number) continue;
        for (const endpoint of [seg.start, seg.end]) {
          if (Math.abs(endpoint.x - px) < 0.1 && Math.abs(endpoint.y - py) < 0.1) {
            // This segment connects to this pad — generate teardrop
            const other = endpoint === seg.start ? seg.end : seg.start;
            const dx = other.x - px;
            const dy = other.y - py;
            const dist = Math.hypot(dx, dy);
            if (dist < 0.1) continue;

            const tdLen = Math.min(teardropLength, dist * 0.5);
            const tdWidth = padR * teardropRatio;
            const nx = -dy / dist;
            const ny = dx / dist;

            // Teardrop triangle: two segments from pad edge to trace
            const tdTip: PcbPoint = {
              x: px + (dx / dist) * tdLen,
              y: py + (dy / dist) * tdLen,
            };
            const tdLeft: PcbPoint = {
              x: px + nx * tdWidth,
              y: py + ny * tdWidth,
            };
            const tdRight: PcbPoint = {
              x: px - nx * tdWidth,
              y: py - ny * tdWidth,
            };

            teardrops.push(makeSegment(tdLeft, tdTip, seg.width, seg.layer, seg.net));
            teardrops.push(makeSegment(tdRight, tdTip, seg.width, seg.layer, seg.net));
          }
        }
      }
    }
  }

  return teardrops;
}

/**
 * Online DRC: check a route-in-progress for violations.
 */
export function checkOnlineDrc(
  data: PcbData,
  routeSegments: PcbSegment[],
  clearance: number,
): RouteViolation[] {
  if (routeSegments.length === 0) return [];
  const obstacles = buildObstacleMap(
    data,
    routeSegments[0]?.layer || "F.Cu",
    routeSegments[0]?.net || 0,
  );
  const totalClearance = (routeSegments[0]?.width || 0.25) / 2 + clearance;
  return checkRouteViolations(routeSegments, obstacles, totalClearance);
}

// ═══════════════════════════════════════════════════════════════
// GEOMETRY HELPERS
// ═══════════════════════════════════════════════════════════════

function makeSegment(start: PcbPoint, end: PcbPoint, width: number, layer: PcbLayerId, net: number): PcbSegment {
  return { uuid: crypto.randomUUID(), start: { ...start }, end: { ...end }, width, layer, net };
}

function applyCornerStyle(start: PcbPoint, end: PcbPoint, style: CornerStyle, options: RouteOptions): PcbSegment[] {
  const { width, layer, net } = options;

  if (style === "any" || (Math.abs(end.x - start.x) < 0.01) || (Math.abs(end.y - start.y) < 0.01)) {
    return [makeSegment(start, end, width, layer, net)];
  }

  const dx = end.x - start.x;
  const dy = end.y - start.y;

  if (style === "90") {
    // Right angle: horizontal then vertical
    const mid: PcbPoint = { x: end.x, y: start.y };
    return [
      makeSegment(start, mid, width, layer, net),
      makeSegment(mid, end, width, layer, net),
    ];
  }

  if (style === "45") {
    // 45-degree entry then orthogonal
    const absDx = Math.abs(dx);
    const absDy = Math.abs(dy);
    const diag = Math.min(absDx, absDy);
    const diagX = Math.sign(dx) * diag;
    const diagY = Math.sign(dy) * diag;

    if (absDx > absDy) {
      // Diagonal first, then horizontal
      const mid: PcbPoint = { x: start.x + diagX, y: start.y + diagY };
      return [
        makeSegment(start, mid, width, layer, net),
        makeSegment(mid, end, width, layer, net),
      ];
    } else {
      // Vertical first, then diagonal
      const mid: PcbPoint = { x: end.x - diagX, y: end.y - diagY };
      return [
        makeSegment(start, mid, width, layer, net),
        makeSegment(mid, end, width, layer, net),
      ];
    }
  }

  // Default: straight line
  return [makeSegment(start, end, width, layer, net)];
}

function hasCollision(segments: PcbSegment[], obstacles: Obstacle[], clearance: number): boolean {
  for (const seg of segments) {
    for (const obs of obstacles) {
      if (obs.layer !== seg.layer) continue;
      if (segmentObstacleCollision(seg, obs, clearance)) return true;
    }
  }
  return false;
}

function segmentObstacleCollision(seg: PcbSegment, obs: Obstacle, clearance: number): boolean {
  // Quick AABB check
  const hw = seg.width / 2 + clearance;
  const segMinX = Math.min(seg.start.x, seg.end.x) - hw;
  const segMaxX = Math.max(seg.start.x, seg.end.x) + hw;
  const segMinY = Math.min(seg.start.y, seg.end.y) - hw;
  const segMaxY = Math.max(seg.start.y, seg.end.y) + hw;

  if (segMaxX < obs.bounds.minX || segMinX > obs.bounds.maxX ||
      segMaxY < obs.bounds.minY || segMinY > obs.bounds.maxY) {
    return false;
  }

  // Detailed check based on obstacle type
  if (obs.center && obs.radius) {
    // Point obstacle (pad/via)
    const dist = pointToSegmentDist(obs.center, seg.start, seg.end);
    return dist < obs.radius + clearance;
  }

  if (obs.start && obs.end) {
    // Segment obstacle
    const dist = segmentToSegmentDist(seg.start, seg.end, obs.start, obs.end);
    return dist < obs.width / 2 + clearance;
  }

  return false;
}

function findBlockingObstacles(start: PcbPoint, end: PcbPoint, obstacles: Obstacle[], clearance: number): Obstacle[] {
  return obstacles.filter((obs) => {
    if (obs.center) {
      return pointToSegmentDist(obs.center, start, end) < (obs.radius || 0) + clearance;
    }
    if (obs.start && obs.end) {
      return segmentToSegmentDist(start, end, obs.start, obs.end) < obs.width / 2 + clearance;
    }
    return false;
  });
}

function generateWaypoints(
  start: PcbPoint,
  end: PcbPoint,
  blockers: Obstacle[],
  clearance: number,
  side: number,
): PcbPoint[] {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len * side;
  const ny = dx / len * side;

  const waypoints: PcbPoint[] = [start];

  for (const obs of blockers) {
    const cx = obs.center?.x || (obs.bounds.minX + obs.bounds.maxX) / 2;
    const cy = obs.center?.y || (obs.bounds.minY + obs.bounds.maxY) / 2;
    const r = (obs.radius || Math.max(obs.bounds.maxX - obs.bounds.minX, obs.bounds.maxY - obs.bounds.minY) / 2) + clearance;

    waypoints.push({ x: cx + nx * r, y: cy + ny * r });
  }

  waypoints.push(end);
  return waypoints;
}

function checkRouteViolations(segments: PcbSegment[], obstacles: Obstacle[], clearance: number): RouteViolation[] {
  const violations: RouteViolation[] = [];
  for (const seg of segments) {
    for (const obs of obstacles) {
      if (obs.layer !== seg.layer) continue;
      if (segmentObstacleCollision(seg, obs, clearance)) {
        violations.push({
          type: "clearance",
          message: `Clearance violation with ${obs.type} (net ${obs.net})`,
          position: { x: (seg.start.x + seg.end.x) / 2, y: (seg.start.y + seg.end.y) / 2 },
        });
      }
    }
  }
  return violations;
}

function pointToSegmentDist(p: PcbPoint, a: PcbPoint, b: PcbPoint): number {
  const dx = b.x - a.x, dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1e-12) return Math.hypot(p.x - a.x, p.y - a.y);
  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

function segmentsIntersect(a1: PcbPoint, a2: PcbPoint, b1: PcbPoint, b2: PcbPoint): boolean {
  const cross = (o: PcbPoint, p: PcbPoint, q: PcbPoint) =>
    (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
  const d1 = cross(b1, b2, a1), d2 = cross(b1, b2, a2);
  const d3 = cross(a1, a2, b1), d4 = cross(a1, a2, b2);
  return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
         ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
}

function segmentToSegmentDist(a1: PcbPoint, a2: PcbPoint, b1: PcbPoint, b2: PcbPoint): number {
  if (segmentsIntersect(a1, a2, b1, b2)) return 0;
  return Math.min(
    pointToSegmentDist(a1, b1, b2),
    pointToSegmentDist(a2, b1, b2),
    pointToSegmentDist(b1, a1, a2),
    pointToSegmentDist(b2, a1, a2),
  );
}
