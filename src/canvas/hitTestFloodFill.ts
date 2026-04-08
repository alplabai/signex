import type { SchematicData, SchPoint } from "@/types";

function wirePointKey(p: SchPoint, epsilon: number): string {
  const bucket = Math.round(1 / epsilon);
  return `${Math.round(p.x * bucket)},${Math.round(p.y * bucket)}`;
}

/**
 * Flood-fill connected wires starting from seed points.
 * Uses a spatial index (Map<pointKey, wireUuids[]>) for O(n) instead of O(n^2).
 */
export function floodFillWires(
  data: SchematicData,
  seedPoints: SchPoint[],
  visited: Set<string>,
  epsilon: number,
): void {
  // Build spatial index: pointKey → [wireUuid, ...]
  const index = new Map<string, string[]>();
  for (const wire of data.wires) {
    const ks = wirePointKey(wire.start, epsilon);
    const ke = wirePointKey(wire.end, epsilon);
    if (!index.has(ks)) index.set(ks, []);
    if (!index.has(ke)) index.set(ke, []);
    index.get(ks)!.push(wire.uuid);
    index.get(ke)!.push(wire.uuid);
  }

  const wireByUuid = new Map(data.wires.map(w => [w.uuid, w]));

  const frontier: SchPoint[] = [...seedPoints];
  while (frontier.length > 0) {
    const point = frontier.pop()!;
    const key = wirePointKey(point, epsilon);
    const candidates = index.get(key) || [];
    for (const uuid of candidates) {
      if (visited.has(uuid)) continue;
      visited.add(uuid);
      const wire = wireByUuid.get(uuid)!;
      frontier.push(wire.start, wire.end);
    }
  }
}
