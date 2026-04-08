/**
 * Cross-probing system between Schematic and PCB editors.
 *
 * Supports:
 * - Schematic → PCB: Select component in schematic, highlight corresponding footprint in PCB
 * - PCB → Schematic: Select footprint in PCB, highlight corresponding symbol in schematic
 * - Net highlighting: Select a net in either editor, highlight all objects on that net in both
 *
 * Uses reference designators (R1, U1, C3) as the link between schematic symbols and PCB footprints.
 */

import { useSchematicStore } from "@/stores/schematic";
import { usePcbStore } from "@/stores/pcb";
import { useEditorStore } from "@/stores/editor";
import { getObjectBounds, rectCenter } from "@/lib/objectBounds";

export type CrossProbeDirection = "sch-to-pcb" | "pcb-to-sch";

/**
 * Cross-probe from schematic to PCB.
 * When a symbol is selected in the schematic, find and select
 * the corresponding footprint in the PCB.
 */
export function crossProbeSchToPcb() {
  const schStore = useSchematicStore.getState();
  const pcbStore = usePcbStore.getState();

  if (!schStore.data || !pcbStore.data) return;

  const selectedRefs: string[] = [];
  for (const uuid of schStore.selectedIds) {
    const sym = schStore.data.symbols.find((s) => s.uuid === uuid);
    if (sym && !sym.is_power) {
      selectedRefs.push(sym.reference);
    }
  }

  if (selectedRefs.length === 0) return;

  // Find footprints with matching references
  const fpUuids = pcbStore.data.footprints
    .filter((fp) => selectedRefs.includes(fp.reference))
    .map((fp) => fp.uuid);

  if (fpUuids.length > 0) {
    pcbStore.selectMultiple(fpUuids);
    // Switch to PCB mode
    useEditorStore.getState().setMode("pcb");
  }
}

/**
 * Cross-probe from PCB to schematic.
 * When a footprint is selected in the PCB, find and select
 * the corresponding symbol in the schematic.
 */
export function crossProbePcbToSch() {
  const schStore = useSchematicStore.getState();
  const pcbStore = usePcbStore.getState();

  if (!schStore.data || !pcbStore.data) return;

  const selectedRefs: string[] = [];
  for (const uuid of pcbStore.selectedIds) {
    const fp = pcbStore.data.footprints.find((f) => f.uuid === uuid);
    if (fp) {
      selectedRefs.push(fp.reference);
    }
  }

  if (selectedRefs.length === 0) return;

  // Find symbols with matching references
  const symUuids = schStore.data.symbols
    .filter((sym) => selectedRefs.includes(sym.reference))
    .map((sym) => sym.uuid);

  if (symUuids.length > 0) {
    schStore.selectMultiple(symUuids);
    useEditorStore.getState().setAutoFocus(symUuids);
    // Switch to schematic mode
    useEditorStore.getState().setMode("schematic");
  }
}

/**
 * Cross-probe a net: highlight all objects on a given net in both editors.
 */
export function crossProbeNet(netName: string) {
  const schStore = useSchematicStore.getState();
  const pcbStore = usePcbStore.getState();

  // Highlight in schematic: select labels with matching net name
  if (schStore.data) {
    const labelUuids = schStore.data.labels
      .filter((l) => l.text === netName)
      .map((l) => l.uuid);
    if (labelUuids.length > 0) {
      schStore.selectMultiple(labelUuids);
    }
  }

  // Highlight in PCB: select segments and vias on matching net
  if (pcbStore.data) {
    const netNum = pcbStore.data.nets.find((n) => n.name === netName)?.number;
    if (netNum !== undefined) {
      const uuids: string[] = [];
      for (const seg of pcbStore.data.segments) {
        if (seg.net === netNum) uuids.push(seg.uuid);
      }
      for (const via of pcbStore.data.vias) {
        if (via.net === netNum) uuids.push(via.uuid);
      }
      pcbStore.selectMultiple(uuids);
    }
  }
}

/**
 * Toggle cross-select mode. When enabled, selecting in one editor
 * automatically selects in the other.
 */
let crossSelectEnabled = false;
let unsubscribeSch: (() => void) | null = null;
let unsubscribePcb: (() => void) | null = null;

export function toggleCrossSelect(): boolean {
  crossSelectEnabled = !crossSelectEnabled;

  if (crossSelectEnabled) {
    // Clean up any existing subscriptions first (guard against double-enable)
    unsubscribeSch?.();
    unsubscribePcb?.();

    // Subscribe to schematic selection changes
    unsubscribeSch = useSchematicStore.subscribe((state) => {
      if (state.selectedIds.size > 0 && useEditorStore.getState().mode === "schematic") {
        crossProbeSchToPcb();
      }
    });

    // Subscribe to PCB selection changes
    unsubscribePcb = usePcbStore.subscribe((state) => {
      if (state.selectedIds.size > 0 && useEditorStore.getState().mode === "pcb") {
        crossProbePcbToSch();
      }
    });
  } else {
    unsubscribeSch?.();
    unsubscribePcb?.();
    unsubscribeSch = null;
    unsubscribePcb = null;
  }

  return crossSelectEnabled;
}

export function isCrossSelectEnabled(): boolean {
  return crossSelectEnabled;
}

/** Zoom the schematic canvas to center on a specific object. */
export function zoomToObject(uuid: string) {
  const store = useSchematicStore.getState();
  if (!store.data) return;
  const bounds = getObjectBounds(uuid, store.data);
  if (!bounds) return;
  const center = rectCenter(bounds);
  const extent = Math.max(bounds.width, bounds.height, 20);
  store.requestZoomTo({ x: center.x, y: center.y, zoom: 800 / extent });
}

/** Zoom to fit multiple objects. */
export function zoomToObjects(uuids: string[]) {
  const store = useSchematicStore.getState();
  if (!store.data || uuids.length === 0) return;
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const uuid of uuids) {
    const b = getObjectBounds(uuid, store.data);
    if (!b) continue;
    minX = Math.min(minX, b.x);
    minY = Math.min(minY, b.y);
    maxX = Math.max(maxX, b.x + b.width);
    maxY = Math.max(maxY, b.y + b.height);
  }
  if (minX === Infinity) return;
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  const extent = Math.max(maxX - minX, maxY - minY, 20);
  store.requestZoomTo({ x: cx, y: cy, zoom: 800 / extent });
}
