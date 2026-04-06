import type { SchematicData, SchPoint } from "@/types";
import { resolveNets, type NetInfo } from "./netResolver";
import { getSymbolPinPositions, pointsMatch } from "./geometry";
import { checkPinConnection } from "./ercMatrix";

export type ErcViolationType =
  | "duplicate_designator" | "unconnected_pin" | "floating_wire"
  | "no_driver" | "single_pin_net" | "missing_power"
  | "output_to_output" | "multiple_net_names" | "unannotated"
  | "missing_no_connect" | "wire_not_connected"
  | "pin_conflict" | "power_pin_not_driven" | "net_no_label";

export interface ErcViolation {
  type: ErcViolationType;
  severity: "error" | "warning";
  message: string;
  uuids: string[];
  position?: SchPoint;
}

/**
 * Run basic Electrical Rules Check on the schematic.
 */
export function runErc(data: SchematicData): { violations: ErcViolation[]; nets: NetInfo[] } {
  const violations: ErcViolation[] = [];
  const nets = resolveNets(data);

  // No ERC directive positions — suppress violations at these points
  const noErcPositions = (data.no_erc_directives || []).map(d => d.position);

  // 1. Duplicate designators
  checkDuplicateDesignators(data, violations);

  // 2. Unconnected pins (pin not on any net with wires)
  checkUnconnectedPins(data, nets, violations);

  // 3. Floating wires (nets with wires but no pins and no labels)
  checkFloatingWires(nets, violations);

  // 4. Nets with single pin (likely missing connection)
  checkSinglePinNets(nets, violations);

  // 5. No driver (net has inputs but no output/bidirectional/passive)
  checkNoDriver(nets, violations);

  // 6. Output-to-output conflict (two output pins on same net)
  checkOutputConflict(nets, violations);

  // 7. Multiple net names on same net
  checkMultipleNetNames(nets, violations, data);

  // 8. Unannotated components (designator ends with ?)
  checkUnannotated(data, violations);

  // 9. Pin-to-pin connection matrix violations
  checkPinConnectionMatrix(nets, violations);

  // 10. Power pins not driven (power_in without power_out on net)
  checkPowerPinNotDriven(nets, violations);

  // 11. Nets with wires but no label
  checkNetNoLabel(nets, violations);

  // Filter out violations at No ERC directive positions
  const filtered = violations.filter(v => {
    if (!v.position || noErcPositions.length === 0) return true;
    return !noErcPositions.some(p => pointsMatch(p, v.position!, 1.0));
  });

  return { violations: filtered, nets };
}

function checkDuplicateDesignators(data: SchematicData, violations: ErcViolation[]) {
  const refMap = new Map<string, string[]>();
  for (const sym of data.symbols) {
    if (sym.is_power) continue;
    const ref = sym.reference;
    if (!ref || ref.endsWith("?")) continue; // Skip unannotated
    if (!refMap.has(ref)) refMap.set(ref, []);
    refMap.get(ref)!.push(sym.uuid);
  }
  for (const [ref, uuids] of refMap) {
    if (uuids.length > 1) {
      violations.push({
        type: "duplicate_designator",
        severity: "error",
        message: `Duplicate designator: ${ref} (${uuids.length} instances)`,
        uuids,
      });
    }
  }
}

function checkUnconnectedPins(data: SchematicData, nets: NetInfo[], violations: ErcViolation[]) {
  // Build set of all pin positions that are on a net with at least one wire
  const connectedPinKeys = new Set<string>();
  for (const net of nets) {
    if (net.wireUuids.length > 0) {
      for (const pin of net.pins) {
        connectedPinKeys.add(`${pin.symbolUuid}:${pin.pinNumber}`);
      }
    }
  }

  // Check all symbol pins
  for (const sym of data.symbols) {
    if (sym.is_power) continue;
    const pins = getSymbolPinPositions(sym, data);
    for (const pin of pins) {
      const key = `${sym.uuid}:${pin.pinNumber}`;
      if (connectedPinKeys.has(key)) continue;
      // Check if there's a no-connect at this pin
      const hasNoConnect = data.no_connects.some(nc => pointsMatch(nc.position, pin.position));
      if (hasNoConnect) continue;
      violations.push({
        type: "unconnected_pin",
        severity: "warning",
        message: `Unconnected pin: ${sym.reference} pin ${pin.pinNumber} (${pin.pinName})`,
        uuids: [sym.uuid],
        position: pin.position,
      });
    }
  }
}

function checkFloatingWires(nets: NetInfo[], violations: ErcViolation[]) {
  for (const net of nets) {
    if (net.wireUuids.length > 0 && net.pins.length === 0 && net.labelUuids.length === 0) {
      violations.push({
        type: "floating_wire",
        severity: "warning",
        message: `Floating wire: not connected to any pin or labeled`,
        uuids: net.wireUuids,
        position: net.points[0],
      });
    }
  }
}

function checkSinglePinNets(nets: NetInfo[], violations: ErcViolation[]) {
  for (const net of nets) {
    if (net.pins.length === 1 && net.wireUuids.length > 0) {
      const pin = net.pins[0];
      violations.push({
        type: "single_pin_net",
        severity: "warning",
        message: `Net ${net.name || "(unnamed)"} has only one pin: ${pin.symbolRef} pin ${pin.pinNumber}`,
        uuids: [pin.symbolUuid, ...net.wireUuids],
      });
    }
  }
}

function checkNoDriver(nets: NetInfo[], violations: ErcViolation[]) {
  const driverTypes = new Set(["output", "bidirectional", "passive", "power_out", "tri_state"]);
  for (const net of nets) {
    if (net.pins.length < 2) continue;
    if (!net.name) continue; // Only check named nets
    const hasDriver = net.pins.some(p => driverTypes.has(p.pinType));
    if (!hasDriver) {
      violations.push({
        type: "no_driver",
        severity: "warning",
        message: `Net "${net.name}" has no driving source (${net.pins.length} pins, all inputs)`,
        uuids: net.pins.map(p => p.symbolUuid),
      });
    }
  }
}

function checkOutputConflict(nets: NetInfo[], violations: ErcViolation[]) {
  for (const net of nets) {
    const outputs = net.pins.filter(p => p.pinType === "output" || p.pinType === "power_out");
    if (outputs.length > 1) {
      violations.push({
        type: "output_to_output",
        severity: "error",
        message: `Net "${net.name || "(unnamed)"}" has ${outputs.length} output drivers: ${outputs.map(o => `${o.symbolRef}:${o.pinNumber}`).join(", ")}`,
        uuids: outputs.map(o => o.symbolUuid),
      });
    }
  }
}

function checkMultipleNetNames(nets: NetInfo[], violations: ErcViolation[], data: SchematicData) {
  for (const net of nets) {
    if (net.labelUuids.length < 2) continue;
    // Get actual label texts and check for conflicts
    const labelTexts = new Set<string>();
    for (const uuid of net.labelUuids) {
      const label = data.labels.find(l => l.uuid === uuid);
      if (label) labelTexts.add(label.text);
    }
    if (labelTexts.size > 1) {
      violations.push({
        type: "multiple_net_names",
        severity: "warning",
        message: `Net has conflicting names: ${[...labelTexts].join(", ")}`,
        uuids: net.labelUuids,
      });
    }
  }
}

function checkUnannotated(data: SchematicData, violations: ErcViolation[]) {
  for (const sym of data.symbols) {
    if (sym.is_power) continue;
    if (sym.reference.endsWith("?") || sym.reference.match(/^[A-Z]+$/)) {
      violations.push({
        type: "unannotated",
        severity: "error",
        message: `Unannotated component: ${sym.reference} (${sym.value})`,
        uuids: [sym.uuid],
        position: sym.position,
      });
    }
  }
}

function checkPinConnectionMatrix(nets: NetInfo[], violations: ErcViolation[]) {
  for (const net of nets) {
    if (net.pins.length < 2) continue;
    // Check every pair of pins on the same net
    const checked = new Set<string>();
    for (let i = 0; i < net.pins.length; i++) {
      for (let j = i + 1; j < net.pins.length; j++) {
        const a = net.pins[i], b = net.pins[j];
        // Avoid duplicate reports for same symbol pair
        const pairKey = [a.symbolUuid, b.symbolUuid].sort().join(":");
        if (checked.has(pairKey)) continue;
        checked.add(pairKey);
        const severity = checkPinConnection(a.pinType, b.pinType);
        if (severity === "error") {
          violations.push({
            type: "pin_conflict",
            severity: "error",
            message: `Pin conflict on net "${net.name || "(unnamed)"}": ${a.symbolRef}:${a.pinNumber} (${a.pinType}) vs ${b.symbolRef}:${b.pinNumber} (${b.pinType})`,
            uuids: [a.symbolUuid, b.symbolUuid],
          });
        } else if (severity === "warning") {
          violations.push({
            type: "pin_conflict",
            severity: "warning",
            message: `Suspicious connection on "${net.name || "(unnamed)"}": ${a.symbolRef}:${a.pinNumber} (${a.pinType}) with ${b.symbolRef}:${b.pinNumber} (${b.pinType})`,
            uuids: [a.symbolUuid, b.symbolUuid],
          });
        }
      }
    }
  }
}

function checkPowerPinNotDriven(nets: NetInfo[], violations: ErcViolation[]) {
  for (const net of nets) {
    const powerInputs = net.pins.filter(p => p.pinType === "power_in" || p.pinType === "power_input");
    if (powerInputs.length === 0) continue;
    const hasPowerSource = net.pins.some(p =>
      p.pinType === "power_out" || p.pinType === "power_output" || p.pinType === "output"
    );
    const hasPowerLabel = net.labelUuids.length > 0;
    if (!hasPowerSource && !hasPowerLabel) {
      violations.push({
        type: "power_pin_not_driven",
        severity: "error",
        message: `Power pin not driven: ${powerInputs.map(p => `${p.symbolRef}:${p.pinNumber}`).join(", ")} on net "${net.name || "(unnamed)"}"`,
        uuids: powerInputs.map(p => p.symbolUuid),
      });
    }
  }
}

function checkNetNoLabel(nets: NetInfo[], violations: ErcViolation[]) {
  for (const net of nets) {
    if (net.pins.length >= 2 && net.labelUuids.length === 0 && net.wireUuids.length > 0) {
      violations.push({
        type: "net_no_label",
        severity: "warning",
        message: `Net with ${net.pins.length} pins has no label (${net.pins.map(p => p.symbolRef + ":" + p.pinNumber).slice(0, 3).join(", ")}${net.pins.length > 3 ? "..." : ""})`,
        uuids: net.wireUuids.slice(0, 1),
        position: net.points[0],
      });
    }
  }
}
