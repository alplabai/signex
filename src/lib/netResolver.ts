import type { SchematicData, SchPoint } from "@/types";
import { getSymbolPinPositions, pointsMatch } from "./geometry";

/** A resolved net — group of electrically connected points */
export interface NetInfo {
  id: number;
  name: string | null;
  wireUuids: string[];
  labelUuids: string[];
  junctionUuids: string[];
  pins: { symbolUuid: string; symbolRef: string; pinNumber: string; pinName: string; pinType: string }[];
  points: SchPoint[]; // All unique positions in this net
}

// --- Union-Find ---
class UnionFind {
  parent: number[];
  rank: number[];
  constructor(n: number) {
    this.parent = Array.from({ length: n }, (_, i) => i);
    this.rank = new Array(n).fill(0);
  }
  find(x: number): number {
    if (this.parent[x] !== x) this.parent[x] = this.find(this.parent[x]);
    return this.parent[x];
  }
  union(a: number, b: number) {
    const ra = this.find(a), rb = this.find(b);
    if (ra === rb) return;
    if (this.rank[ra] < this.rank[rb]) this.parent[ra] = rb;
    else if (this.rank[ra] > this.rank[rb]) this.parent[rb] = ra;
    else { this.parent[rb] = ra; this.rank[ra]++; }
  }
}

// Point → index mapping. Round to 1 decimal place (0.1mm buckets)
// so the bucket size matches the connectivity tolerance (TOL = 0.05).
function pointKey(p: SchPoint): string {
  return `${Math.round(p.x * 10)},${Math.round(p.y * 10)}`;
}

/**
 * Resolve all nets in the schematic.
 * Groups wires, labels, junctions, and symbol pins into connected nets using union-find.
 */
export function resolveNets(data: SchematicData): NetInfo[] {
  const pointIndex = new Map<string, number>(); // key → node index
  const points: SchPoint[] = [];
  const TOL = 0.05;

  // Get or create a node index for a point
  function getNode(p: SchPoint): number {
    const k = pointKey(p);
    if (pointIndex.has(k)) return pointIndex.get(k)!;
    const idx = points.length;
    points.push(p);
    pointIndex.set(k, idx);
    return idx;
  }

  // Collect all nodes
  // Wire endpoints
  const wireNodes: { uuid: string; startIdx: number; endIdx: number }[] = [];
  for (const wire of data.wires) {
    const si = getNode(wire.start);
    const ei = getNode(wire.end);
    wireNodes.push({ uuid: wire.uuid, startIdx: si, endIdx: ei });
  }

  // Junction positions
  const junctionNodes: { uuid: string; idx: number }[] = [];
  for (const j of data.junctions) {
    junctionNodes.push({ uuid: j.uuid, idx: getNode(j.position) });
  }

  // Label positions
  const labelNodes: { uuid: string; text: string; type: string; idx: number }[] = [];
  for (const label of data.labels) {
    labelNodes.push({ uuid: label.uuid, text: label.text, type: label.label_type, idx: getNode(label.position) });
  }

  // Symbol pin positions
  const pinNodes: { symbolUuid: string; symbolRef: string; pinNumber: string; pinName: string; pinType: string; idx: number }[] = [];
  for (const sym of data.symbols) {
    const pins = getSymbolPinPositions(sym, data);
    for (const pin of pins) {
      pinNodes.push({
        symbolUuid: sym.uuid,
        symbolRef: sym.reference,
        pinNumber: pin.pinNumber,
        pinName: pin.pinName,
        pinType: pin.pinType,
        idx: getNode(pin.position),
      });
    }
  }

  // Build union-find
  const uf = new UnionFind(points.length);

  // Union wire start↔end (each wire connects its endpoints)
  for (const wn of wireNodes) {
    uf.union(wn.startIdx, wn.endIdx);
  }

  // Union junctions with any wire passing through them (mid-segment T-junctions).
  // getNode handles endpoint matching, but junctions at wire midpoints need explicit union.
  for (const j of junctionNodes) {
    // Junction connects all wires whose endpoints match the junction position
    for (const wn of wireNodes) {
      if (uf.find(wn.startIdx) === uf.find(j.idx) || uf.find(wn.endIdx) === uf.find(j.idx)) continue;
      // Check if junction is on this wire's line (not just endpoints)
      const jp = points[j.idx];
      const ws = points[wn.startIdx], we = points[wn.endIdx];
      if (pointOnSegment(jp, ws, we, TOL)) {
        uf.union(j.idx, wn.startIdx);
      }
    }
  }

  // Group by root
  const groups = new Map<number, number[]>();
  for (let i = 0; i < points.length; i++) {
    const root = uf.find(i);
    if (!groups.has(root)) groups.set(root, []);
    groups.get(root)!.push(i);
  }

  // Build NetInfo for each group
  const nets: NetInfo[] = [];
  let netId = 0;
  for (const [, members] of groups) {
    const memberSet = new Set(members);
    const net: NetInfo = {
      id: netId++,
      name: null,
      wireUuids: [],
      labelUuids: [],
      junctionUuids: [],
      pins: [],
      points: members.map(i => points[i]),
    };

    // Collect wires in this net
    for (const wn of wireNodes) {
      if (memberSet.has(uf.find(wn.startIdx))) {
        net.wireUuids.push(wn.uuid);
      }
    }

    // Collect labels → assign net name
    for (const ln of labelNodes) {
      if (memberSet.has(uf.find(ln.idx))) {
        net.labelUuids.push(ln.uuid);
        if (!net.name) net.name = ln.text;
      }
    }

    // Collect junctions
    for (const jn of junctionNodes) {
      if (memberSet.has(uf.find(jn.idx))) {
        net.junctionUuids.push(jn.uuid);
      }
    }

    // Collect pins
    for (const pn of pinNodes) {
      if (memberSet.has(uf.find(pn.idx))) {
        net.pins.push({
          symbolUuid: pn.symbolUuid,
          symbolRef: pn.symbolRef,
          pinNumber: pn.pinNumber,
          pinName: pn.pinName,
          pinType: pn.pinType,
        });
      }
    }

    // Only include nets that have at least one wire or pin
    if (net.wireUuids.length > 0 || net.pins.length > 0) {
      nets.push(net);
    }
  }

  return nets;
}

function pointOnSegment(p: SchPoint, a: SchPoint, b: SchPoint, tol: number): boolean {
  const dx = b.x - a.x, dy = b.y - a.y;
  const len = Math.hypot(dx, dy);
  if (len < tol) return pointsMatch(p, a, tol);
  const dist = Math.abs(dx * (a.y - p.y) - dy * (a.x - p.x)) / len;
  if (dist > tol) return false;
  const t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (len * len);
  return t >= -0.01 && t <= 1.01;
}
