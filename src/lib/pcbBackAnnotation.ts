/**
 * Back Annotation — sync PCB changes back to schematic.
 *
 * Detects differences between schematic and PCB (reference designators,
 * values, footprints) and generates an ECO (Engineering Change Order)
 * that can be applied to the schematic.
 */

import type { SchematicData } from "@/types";
import type { PcbData } from "@/types/pcb";

export interface EcoChange {
  type: "reference_changed" | "value_changed" | "footprint_changed" | "component_added" | "component_removed";
  reference: string;
  field: string;
  oldValue: string;
  newValue: string;
}

/**
 * Compare schematic and PCB to detect differences.
 * Returns a list of changes that need to be applied to the schematic.
 */
export function detectEcoChanges(schematic: SchematicData, pcb: PcbData): EcoChange[] {
  const changes: EcoChange[] = [];

  // Build maps by reference designator
  const schMap = new Map<string, { value: string; footprint: string; uuid: string }>();
  for (const sym of schematic.symbols) {
    if (sym.is_power) continue;
    schMap.set(sym.reference, {
      value: sym.value,
      footprint: sym.footprint,
      uuid: sym.uuid,
    });
  }

  const pcbMap = new Map<string, { value: string; footprint: string; uuid: string }>();
  for (const fp of pcb.footprints) {
    pcbMap.set(fp.reference, {
      value: fp.value,
      footprint: fp.footprintId,
      uuid: fp.uuid,
    });
  }

  // Check for changes in existing components
  for (const [ref, pcbComp] of pcbMap) {
    const schComp = schMap.get(ref);
    if (!schComp) {
      changes.push({
        type: "component_added",
        reference: ref,
        field: "reference",
        oldValue: "",
        newValue: ref,
      });
      continue;
    }

    if (schComp.value !== pcbComp.value) {
      changes.push({
        type: "value_changed",
        reference: ref,
        field: "value",
        oldValue: schComp.value,
        newValue: pcbComp.value,
      });
    }

    if (schComp.footprint !== pcbComp.footprint) {
      changes.push({
        type: "footprint_changed",
        reference: ref,
        field: "footprint",
        oldValue: schComp.footprint,
        newValue: pcbComp.footprint,
      });
    }
  }

  // Check for components in schematic but not in PCB
  for (const [ref] of schMap) {
    if (!pcbMap.has(ref)) {
      changes.push({
        type: "component_removed",
        reference: ref,
        field: "reference",
        oldValue: ref,
        newValue: "",
      });
    }
  }

  return changes;
}

/**
 * Apply ECO changes to the schematic.
 * Modifies the schematic data in-place and returns the number of changes applied.
 */
export function applyEcoChanges(schematic: SchematicData, changes: EcoChange[]): number {
  let applied = 0;

  for (const change of changes) {
    if (change.type === "value_changed") {
      const sym = schematic.symbols.find((s) => s.reference === change.reference);
      if (sym) {
        sym.value = change.newValue;
        applied++;
      }
    } else if (change.type === "footprint_changed") {
      const sym = schematic.symbols.find((s) => s.reference === change.reference);
      if (sym) {
        sym.footprint = change.newValue;
        applied++;
      }
    }
    // component_added and component_removed are informational — user decides
  }

  return applied;
}
