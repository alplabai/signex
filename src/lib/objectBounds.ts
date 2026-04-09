import type { SchematicData } from "@/types";

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Return bounding rect for a schematic object by UUID */
export function getObjectBounds(uuid: string, data: SchematicData): Rect | null {
  const sym = data.symbols.find(s => s.uuid === uuid);
  if (sym) {
    const w = sym.is_power ? 5 : 20;
    const h = sym.is_power ? 10 : 15;
    return { x: sym.position.x - w / 2, y: sym.position.y - h / 2, width: w, height: h };
  }

  const wire = data.wires.find(w => w.uuid === uuid);
  if (wire) {
    const minX = Math.min(wire.start.x, wire.end.x);
    const minY = Math.min(wire.start.y, wire.end.y);
    return { x: minX, y: minY, width: Math.abs(wire.end.x - wire.start.x) || 2, height: Math.abs(wire.end.y - wire.start.y) || 2 };
  }

  const label = data.labels.find(l => l.uuid === uuid);
  if (label) {
    const w = label.text.length * 1.5 + 4;
    return { x: label.position.x - 1, y: label.position.y - 2, width: w, height: 4 };
  }

  const junction = data.junctions.find(j => j.uuid === uuid);
  if (junction) return { x: junction.position.x - 1, y: junction.position.y - 1, width: 2, height: 2 };

  const nc = data.no_connects.find(n => n.uuid === uuid);
  if (nc) return { x: nc.position.x - 1, y: nc.position.y - 1, width: 2, height: 2 };

  const bus = data.buses.find(b => b.uuid === uuid);
  if (bus) {
    const minX = Math.min(bus.start.x, bus.end.x);
    const minY = Math.min(bus.start.y, bus.end.y);
    return { x: minX, y: minY, width: Math.abs(bus.end.x - bus.start.x) || 2, height: Math.abs(bus.end.y - bus.start.y) || 2 };
  }

  const be = data.bus_entries.find(b => b.uuid === uuid);
  if (be) return { x: be.position.x - 1, y: be.position.y - 1, width: 4, height: 4 };

  const tn = data.text_notes.find(t => t.uuid === uuid);
  if (tn) {
    const w = (tn.text?.length ?? 5) * 1.5 + 4;
    return { x: tn.position.x, y: tn.position.y - 2, width: w, height: 4 };
  }

  const cs = data.child_sheets.find(c => c.uuid === uuid);
  if (cs) return { x: cs.position.x, y: cs.position.y, width: cs.size?.[0] ?? 30, height: cs.size?.[1] ?? 20 };

  const rect = data.rectangles.find(r => r.uuid === uuid);
  if (rect) {
    const minX = Math.min(rect.start.x, rect.end.x);
    const minY = Math.min(rect.start.y, rect.end.y);
    return { x: minX, y: minY, width: Math.abs(rect.end.x - rect.start.x) || 2, height: Math.abs(rect.end.y - rect.start.y) || 2 };
  }

  const drawing = data.drawings.find(d => d.uuid === uuid);
  if (drawing) {
    const pos = (drawing as any).position || (drawing as any).start || (drawing as any).center || { x: 0, y: 0 };
    return { x: pos.x - 5, y: pos.y - 5, width: 10, height: 10 };
  }

  return null;
}

export function rectCenter(r: Rect): { x: number; y: number } {
  return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
}
