import type { SchematicData, SchPoint } from "@/types";
import type { SelectionFilter } from "@/stores/editor";
import {
  isSelectable,
  symToSch,
  distToSegment,
  segmentsIntersect,
  pointInBox,
  segmentIntersectsBox,
  pointInPolygon,
} from "./hitTestUtils";
import type { Box } from "./hitTestUtils";

/**
 * Select all objects within a drag box.
 * crossing=true: select if object intersects box (right-to-left drag)
 * crossing=false: select only if object is fully inside box (left-to-right drag)
 */
export function boxSelect(
  data: SchematicData,
  startX: number, startY: number,
  endX: number, endY: number,
  filter?: SelectionFilter,
): string[] {
  const crossing = endX < startX;
  const box: Box = {
    minX: Math.min(startX, endX), minY: Math.min(startY, endY),
    maxX: Math.max(startX, endX), maxY: Math.max(startY, endY),
  };

  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (!isSelectable("symbol", filter, sym.is_power)) continue;
    if (crossing) {
      // Crossing: select if symbol bounding box overlaps selection box
      const lib = data.lib_symbols[sym.lib_id];
      if (lib) {
        let lMinX = -2, lMaxX = 2, lMinY = -2, lMaxY = 2;
        for (const g of lib.graphics) {
          if (g.type === "Rectangle") { lMinX = Math.min(lMinX, g.start.x, g.end.x); lMaxX = Math.max(lMaxX, g.start.x, g.end.x); lMinY = Math.min(lMinY, g.start.y, g.end.y); lMaxY = Math.max(lMaxY, g.start.y, g.end.y); }
          else if (g.type === "Polyline") { for (const p of g.points) { lMinX = Math.min(lMinX, p.x); lMaxX = Math.max(lMaxX, p.x); lMinY = Math.min(lMinY, p.y); lMaxY = Math.max(lMaxY, p.y); } }
          else if (g.type === "Circle") { lMinX = Math.min(lMinX, g.center.x - g.radius); lMaxX = Math.max(lMaxX, g.center.x + g.radius); lMinY = Math.min(lMinY, g.center.y - g.radius); lMaxY = Math.max(lMaxY, g.center.y + g.radius); }
        }
        for (const pin of lib.pins) { lMinX = Math.min(lMinX, pin.position.x); lMaxX = Math.max(lMaxX, pin.position.x); lMinY = Math.min(lMinY, pin.position.y); lMaxY = Math.max(lMaxY, pin.position.y); }
        const corners = [symToSch(lMinX, lMinY, sym), symToSch(lMaxX, lMinY, sym), symToSch(lMaxX, lMaxY, sym), symToSch(lMinX, lMaxY, sym)];
        const symBox = {
          minX: Math.min(...corners.map(c => c.x)), minY: Math.min(...corners.map(c => c.y)),
          maxX: Math.max(...corners.map(c => c.x)), maxY: Math.max(...corners.map(c => c.y)),
        };
        // Check if symbol bbox overlaps selection box
        if (symBox.maxX >= box.minX && symBox.minX <= box.maxX && symBox.maxY >= box.minY && symBox.minY <= box.maxY) {
          selected.push(sym.uuid);
        }
      } else if (pointInBox(sym.position, box)) {
        selected.push(sym.uuid);
      }
    } else {
      // Inside: symbol center must be in box
      if (pointInBox(sym.position, box)) selected.push(sym.uuid);
    }
  }

  for (const wire of data.wires) {
    if (!isSelectable("wire", filter)) continue;
    if (crossing) {
      if (segmentIntersectsBox(wire.start, wire.end, box)) selected.push(wire.uuid);
    } else {
      if (pointInBox(wire.start, box) && pointInBox(wire.end, box)) selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;

    if ((label.label_type === "Global" || label.label_type === "Hierarchical") && crossing) {
      // Use flag shape bounding box for crossing selection
      const fs = label.font_size || 1.27;
      const h = fs * 1.4;
      const arrowW = h * 0.5;
      const tw = label.text.length * fs * 0.65;
      const pad = fs * 0.3;
      const totalBody = arrowW + tw + pad * 2;
      const r = label.rotation;
      const lx = label.position.x, ly = label.position.y;
      const isHoriz = r === 0 || r === 180;

      let lMinX: number, lMinY: number, lMaxX: number, lMaxY: number;
      if (isHoriz) {
        const dir = r === 0 ? 1 : -1;
        if (dir > 0) {
          lMinX = lx; lMaxX = lx + totalBody + arrowW;
          lMinY = ly - h / 2; lMaxY = ly + h / 2;
        } else {
          lMinX = lx - totalBody - arrowW; lMaxX = lx;
          lMinY = ly - h / 2; lMaxY = ly + h / 2;
        }
      } else {
        lMinX = lx - h / 2; lMaxX = lx + h / 2;
        lMinY = ly - totalBody; lMaxY = ly + totalBody;
      }

      // Check if label bbox overlaps selection box
      if (lMaxX >= box.minX && lMinX <= box.maxX && lMaxY >= box.minY && lMinY <= box.maxY) {
        selected.push(label.uuid);
      }
    } else {
      if (pointInBox(label.position, box)) selected.push(label.uuid);
    }
  }

  for (const j of data.junctions) {
    if (!isSelectable("junction", filter)) continue;
    if (pointInBox(j.position, box)) selected.push(j.uuid);
  }

  for (const nc of data.no_connects) {
    if (!isSelectable("noConnect", filter)) continue;
    if (pointInBox(nc.position, box)) selected.push(nc.uuid);
  }

  for (const note of data.text_notes) {
    if (!isSelectable("textNote", filter)) continue;
    if (pointInBox(note.position, box)) selected.push(note.uuid);
  }

  for (const bus of data.buses) {
    if (!isSelectable("bus", filter)) continue;
    if (crossing) {
      if (segmentIntersectsBox(bus.start, bus.end, box)) selected.push(bus.uuid);
    } else {
      if (pointInBox(bus.start, box) && pointInBox(bus.end, box)) selected.push(bus.uuid);
    }
  }

  for (const be of data.bus_entries) {
    if (!isSelectable("busEntry", filter)) continue;
    if (pointInBox(be.position, box)) selected.push(be.uuid);
  }

  for (const sheet of data.child_sheets) {
    if (!isSelectable("childSheet", filter)) continue;
    if (pointInBox(sheet.position, box)) selected.push(sheet.uuid);
  }

  if (!isSelectable("drawing", filter)) { /* skip drawings */ }
  else for (const d of data.drawings) {
    if (d.type === "Line" && pointInBox(d.start, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Rect" && pointInBox(d.start, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Circle" && pointInBox(d.center, box)) selected.push(d.uuid);
    else if (d.type === "Arc" && pointInBox(d.start, box) && pointInBox(d.mid, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Polyline" && d.points.every(p => pointInBox(p, box))) selected.push(d.uuid);
  }

  // New directive/annotation types
  if (data.parameter_sets) for (const ps of data.parameter_sets) { if (pointInBox(ps.position, box)) selected.push(ps.uuid); }
  if (data.diff_pair_directives) for (const dp of data.diff_pair_directives) { if (pointInBox(dp.position, box)) selected.push(dp.uuid); }
  if (data.blankets) for (const bl of data.blankets) { if (bl.points.every(p => pointInBox(p, box))) selected.push(bl.uuid); }
  if (data.compile_masks) for (const cm of data.compile_masks) { if (pointInBox(cm.position, box)) selected.push(cm.uuid); }
  if (data.notes) for (const n of data.notes) { if (pointInBox(n.position, box)) selected.push(n.uuid); }

  return selected;
}

/**
 * Select all objects whose position falls inside a lasso polygon.
 * Uses ray-casting point-in-polygon test for each object.
 */
export function lassoSelect(
  data: SchematicData,
  points: SchPoint[],
  filter?: SelectionFilter,
): string[] {
  if (points.length < 3) return [];

  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (!isSelectable("symbol", filter, sym.is_power)) continue;
    if (pointInPolygon(sym.position, points)) selected.push(sym.uuid);
  }

  for (const wire of data.wires) {
    if (!isSelectable("wire", filter)) continue;
    // Wire is selected if both endpoints are inside the lasso
    if (pointInPolygon(wire.start, points) && pointInPolygon(wire.end, points)) {
      selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;
    if (pointInPolygon(label.position, points)) selected.push(label.uuid);
  }

  for (const j of data.junctions) {
    if (!isSelectable("junction", filter)) continue;
    if (pointInPolygon(j.position, points)) selected.push(j.uuid);
  }

  for (const nc of data.no_connects) {
    if (!isSelectable("noConnect", filter)) continue;
    if (pointInPolygon(nc.position, points)) selected.push(nc.uuid);
  }

  for (const note of data.text_notes) {
    if (!isSelectable("textNote", filter)) continue;
    if (pointInPolygon(note.position, points)) selected.push(note.uuid);
  }

  for (const bus of data.buses) {
    if (!isSelectable("bus", filter)) continue;
    if (pointInPolygon(bus.start, points) && pointInPolygon(bus.end, points)) {
      selected.push(bus.uuid);
    }
  }

  for (const be of data.bus_entries) {
    if (!isSelectable("busEntry", filter)) continue;
    if (pointInPolygon(be.position, points)) selected.push(be.uuid);
  }

  for (const sheet of data.child_sheets) {
    if (!isSelectable("childSheet", filter)) continue;
    if (pointInPolygon(sheet.position, points)) selected.push(sheet.uuid);
  }

  if (!isSelectable("drawing", filter)) { /* skip drawings */ }
  else for (const d of data.drawings) {
    if (d.type === "Line" && pointInPolygon(d.start, points) && pointInPolygon(d.end, points)) selected.push(d.uuid);
    else if (d.type === "Rect" && pointInPolygon(d.start, points) && pointInPolygon(d.end, points)) selected.push(d.uuid);
    else if (d.type === "Circle" && pointInPolygon(d.center, points)) selected.push(d.uuid);
    else if (d.type === "Arc" && pointInPolygon(d.start, points) && pointInPolygon(d.mid, points) && pointInPolygon(d.end, points)) selected.push(d.uuid);
    else if (d.type === "Polyline" && d.points.every(p => pointInPolygon(p, points))) selected.push(d.uuid);
  }

  return selected;
}

/**
 * Select all objects OUTSIDE a drag box (inverse of inside-mode boxSelect).
 * Returns UUIDs of objects not captured by a left-to-right box selection.
 */
export function outsideBoxSelect(
  data: SchematicData,
  startX: number, startY: number,
  endX: number, endY: number,
  filter?: SelectionFilter,
): string[] {
  // Get all selectable UUIDs
  const allUuids: string[] = [];

  for (const sym of data.symbols) {
    if (isSelectable("symbol", filter, sym.is_power)) allUuids.push(sym.uuid);
  }
  for (const wire of data.wires) {
    if (isSelectable("wire", filter)) allUuids.push(wire.uuid);
  }
  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;
    allUuids.push(label.uuid);
  }
  for (const j of data.junctions) {
    if (isSelectable("junction", filter)) allUuids.push(j.uuid);
  }
  for (const nc of data.no_connects) {
    if (isSelectable("noConnect", filter)) allUuids.push(nc.uuid);
  }
  for (const note of data.text_notes) {
    if (isSelectable("textNote", filter)) allUuids.push(note.uuid);
  }
  for (const bus of data.buses) {
    if (isSelectable("bus", filter)) allUuids.push(bus.uuid);
  }
  for (const be of data.bus_entries) {
    if (isSelectable("busEntry", filter)) allUuids.push(be.uuid);
  }
  for (const sheet of data.child_sheets) {
    if (isSelectable("childSheet", filter)) allUuids.push(sheet.uuid);
  }
  if (isSelectable("drawing", filter)) {
    for (const d of data.drawings) allUuids.push(d.uuid);
  }

  // Use inside-mode boxSelect (L→R) to find objects inside the box
  const minX = Math.min(startX, endX), minY = Math.min(startY, endY);
  const maxX = Math.max(startX, endX), maxY = Math.max(startY, endY);
  const insideUuids = new Set(boxSelect(data, minX, minY, maxX, maxY, filter));

  // Return everything NOT inside the box
  return allUuids.filter(uuid => !insideUuids.has(uuid));
}

/**
 * Select objects whose position is within `tolerance` distance of a line segment.
 * For wires/buses, checks if any of their segments intersect the selection line.
 */
export function lineSelect(
  data: SchematicData,
  lineStart: SchPoint,
  lineEnd: SchPoint,
  tolerance: number,
  filter?: SelectionFilter,
): string[] {
  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (!isSelectable("symbol", filter, sym.is_power)) continue;
    if (distToSegment(sym.position, lineStart, lineEnd) <= tolerance) {
      selected.push(sym.uuid);
    }
  }

  for (const wire of data.wires) {
    if (!isSelectable("wire", filter)) continue;
    // Check if wire midpoint or endpoints are near the line, or if segments intersect
    if (distToSegment(wire.start, lineStart, lineEnd) <= tolerance ||
        distToSegment(wire.end, lineStart, lineEnd) <= tolerance ||
        segmentsIntersect(wire.start, wire.end, lineStart, lineEnd)) {
      selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;
    if (distToSegment(label.position, lineStart, lineEnd) <= tolerance) {
      selected.push(label.uuid);
    }
  }

  for (const j of data.junctions) {
    if (!isSelectable("junction", filter)) continue;
    if (distToSegment(j.position, lineStart, lineEnd) <= tolerance) {
      selected.push(j.uuid);
    }
  }

  for (const nc of data.no_connects) {
    if (!isSelectable("noConnect", filter)) continue;
    if (distToSegment(nc.position, lineStart, lineEnd) <= tolerance) {
      selected.push(nc.uuid);
    }
  }

  for (const note of data.text_notes) {
    if (!isSelectable("textNote", filter)) continue;
    if (distToSegment(note.position, lineStart, lineEnd) <= tolerance) {
      selected.push(note.uuid);
    }
  }

  for (const bus of data.buses) {
    if (!isSelectable("bus", filter)) continue;
    if (distToSegment(bus.start, lineStart, lineEnd) <= tolerance ||
        distToSegment(bus.end, lineStart, lineEnd) <= tolerance ||
        segmentsIntersect(bus.start, bus.end, lineStart, lineEnd)) {
      selected.push(bus.uuid);
    }
  }

  for (const be of data.bus_entries) {
    if (!isSelectable("busEntry", filter)) continue;
    if (distToSegment(be.position, lineStart, lineEnd) <= tolerance) {
      selected.push(be.uuid);
    }
  }

  for (const sheet of data.child_sheets) {
    if (!isSelectable("childSheet", filter)) continue;
    if (distToSegment(sheet.position, lineStart, lineEnd) <= tolerance) {
      selected.push(sheet.uuid);
    }
  }

  if (!isSelectable("drawing", filter)) { /* skip drawings */ }
  else for (const d of data.drawings) {
    if (d.type === "Circle" && distToSegment(d.center, lineStart, lineEnd) <= tolerance) selected.push(d.uuid);
    else if ((d.type === "Line" || d.type === "Rect") && (distToSegment(d.start, lineStart, lineEnd) <= tolerance || distToSegment(d.end, lineStart, lineEnd) <= tolerance)) selected.push(d.uuid);
    else if (d.type === "Arc" && (distToSegment(d.start, lineStart, lineEnd) <= tolerance || distToSegment(d.mid, lineStart, lineEnd) <= tolerance || distToSegment(d.end, lineStart, lineEnd) <= tolerance)) selected.push(d.uuid);
    else if (d.type === "Polyline" && d.points.some(p => distToSegment(p, lineStart, lineEnd) <= tolerance)) selected.push(d.uuid);
  }

  return selected;
}
