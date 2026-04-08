import type { SchematicData, SchPoint, SchSymbol } from "@/types";
import type { SelectionFilter } from "@/stores/editor";
import { isSelectable, symToSch } from "./hitTestUtils";
import { floodFillWires } from "./hitTestFloodFill";

/**
 * Select all objects on the same net as the clicked object.
 * Traces connectivity through wires, labels, and symbol pins.
 */
export function connectionSelect(
  data: SchematicData,
  startUuid: string,
  filter?: SelectionFilter,
): string[] {
  const EPSILON = 0.5; // Tolerance for endpoint matching

  // Helper: check if two points are coincident
  const ptEq = (a: SchPoint, b: SchPoint) =>
    Math.abs(a.x - b.x) < EPSILON && Math.abs(a.y - b.y) < EPSILON;

  // Helper: get symbol pin positions in schematic space
  const getSymbolPinPositions = (sym: SchSymbol): SchPoint[] => {
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) return [];
    return lib.pins.map(pin => symToSch(pin.position.x, pin.position.y, sym));
  };

  const wireByUuid = new Map(data.wires.map(w => [w.uuid, w]));

  // Find the clicked object
  const clickedWire = wireByUuid.get(startUuid);
  const clickedLabel = data.labels.find(l => l.uuid === startUuid);
  const clickedSymbol = data.symbols.find(s => s.uuid === startUuid);
  const clickedJunction = data.junctions.find(j => j.uuid === startUuid);

  // Strategy 1: If clicked a label, find all objects with the same net name
  if (clickedLabel) {
    const netName = clickedLabel.text;
    const result: string[] = [];

    // All labels with the same text
    for (const label of data.labels) {
      if (!isSelectable("label", filter)) continue;
      if (label.text === netName) result.push(label.uuid);
    }

    // Find wires connected to those labels
    const labelPositions = data.labels.filter(l => l.text === netName).map(l => l.position);
    const connectedWireUuids = new Set<string>();
    floodFillWires(data, labelPositions, connectedWireUuids, EPSILON);

    for (const uuid of connectedWireUuids) {
      if (isSelectable("wire", filter)) result.push(uuid);
    }

    // Find junctions on connected wires
    for (const j of data.junctions) {
      if (!isSelectable("junction", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(j.position, w.start) || ptEq(j.position, w.end))) {
          result.push(j.uuid);
          break;
        }
      }
    }

    // Find symbols with pins touching connected wires
    for (const sym of data.symbols) {
      if (!isSelectable("symbol", filter, sym.is_power)) continue;
      const pinPositions = getSymbolPinPositions(sym);
      for (const pinPos of pinPositions) {
        let connected = false;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(pinPos, w.start) || ptEq(pinPos, w.end))) {
            connected = true;
            break;
          }
        }
        if (connected) { result.push(sym.uuid); break; }
      }
    }

    return [...new Set(result)];
  }

  // Strategy 2: If clicked a wire, flood-fill connected wires and find labels/symbols
  if (clickedWire) {
    const result: string[] = [];
    const connectedWireUuids = new Set<string>();
    floodFillWires(data, [clickedWire.start, clickedWire.end], connectedWireUuids, EPSILON);

    for (const uuid of connectedWireUuids) {
      if (isSelectable("wire", filter)) result.push(uuid);
    }

    // Find labels at wire endpoints
    for (const label of data.labels) {
      if (!isSelectable("label", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(label.position, w.start) || ptEq(label.position, w.end))) {
          result.push(label.uuid);
          break;
        }
      }
    }

    // Find junctions on connected wires
    for (const j of data.junctions) {
      if (!isSelectable("junction", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(j.position, w.start) || ptEq(j.position, w.end))) {
          result.push(j.uuid);
          break;
        }
      }
    }

    // Find symbols with pins touching connected wires
    for (const sym of data.symbols) {
      if (!isSelectable("symbol", filter, sym.is_power)) continue;
      const pinPositions = getSymbolPinPositions(sym);
      for (const pinPos of pinPositions) {
        let connected = false;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(pinPos, w.start) || ptEq(pinPos, w.end))) {
            connected = true;
            break;
          }
        }
        if (connected) { result.push(sym.uuid); break; }
      }
    }

    // Also include labels with same net name (for global connectivity)
    const netLabels = data.labels.filter(l => {
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(l.position, w.start) || ptEq(l.position, w.end))) return true;
      }
      return false;
    });
    if (netLabels.length > 0) {
      const netName = netLabels[0].text;
      for (const label of data.labels) {
        if (label.text === netName && !result.includes(label.uuid)) {
          if (isSelectable("label", filter)) result.push(label.uuid);
        }
      }
    }

    return [...new Set(result)];
  }

  // Strategy 3: If clicked a symbol, find wires/labels connected to its pins
  if (clickedSymbol) {
    const result: string[] = [];
    if (isSelectable("symbol", filter, clickedSymbol.is_power)) {
      result.push(clickedSymbol.uuid);
    }

    const pinPositions = getSymbolPinPositions(clickedSymbol);
    for (const pinPos of pinPositions) {
      // Find wires touching this pin
      const connectedWireUuids = new Set<string>();
      floodFillWires(data, [pinPos], connectedWireUuids, EPSILON);

      for (const uuid of connectedWireUuids) {
        if (isSelectable("wire", filter)) result.push(uuid);
      }

      // Find labels at wire endpoints
      for (const label of data.labels) {
        if (!isSelectable("label", filter)) continue;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(label.position, w.start) || ptEq(label.position, w.end))) {
            result.push(label.uuid);
            break;
          }
        }
      }

      // Find junctions on connected wires
      for (const j of data.junctions) {
        if (!isSelectable("junction", filter)) continue;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(j.position, w.start) || ptEq(j.position, w.end))) {
            result.push(j.uuid);
            break;
          }
        }
      }
    }

    return [...new Set(result)];
  }

  // Strategy 4: If clicked a junction, treat like a wire endpoint
  if (clickedJunction) {
    const result: string[] = [];
    if (isSelectable("junction", filter)) result.push(clickedJunction.uuid);

    const connectedWireUuids = new Set<string>();
    floodFillWires(data, [clickedJunction.position], connectedWireUuids, EPSILON);

    for (const uuid of connectedWireUuids) {
      if (isSelectable("wire", filter)) result.push(uuid);
    }

    for (const label of data.labels) {
      if (!isSelectable("label", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(label.position, w.start) || ptEq(label.position, w.end))) {
          result.push(label.uuid);
          break;
        }
      }
    }

    for (const sym of data.symbols) {
      if (!isSelectable("symbol", filter, sym.is_power)) continue;
      const pinPositions = getSymbolPinPositions(sym);
      for (const pinPos of pinPositions) {
        let connected = false;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(pinPos, w.start) || ptEq(pinPos, w.end))) {
            connected = true;
            break;
          }
        }
        if (connected) { result.push(sym.uuid); break; }
      }
    }

    return [...new Set(result)];
  }

  // Fallback: just return the clicked object if it exists
  return [startUuid];
}
