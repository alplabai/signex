import { useSchematicStore } from "@/stores/schematic";
import { runErc } from "./erc";
import type { SchPoint } from "@/types";

interface ToolResult {
  success: boolean;
  message: string;
}

/**
 * Execute a Signal AI tool call against the schematic store.
 * Returns a result string to send back to Claude as tool_result.
 */
export function executeToolCall(
  name: string,
  input: Record<string, unknown>,
): ToolResult {
  const store = useSchematicStore.getState();
  const data = store.data;
  if (!data) return { success: false, message: "No schematic loaded" };

  switch (name) {
    case "add_component": {
      const prefix = String(input.reference_prefix || "U");
      const value = String(input.value || "");
      const x = Number(input.x || 100);
      const y = Number(input.y || 100);

      // Find next available reference number
      const existing = data.symbols
        .filter((s) => s.reference.startsWith(prefix) && /^\d/.test(s.reference.slice(prefix.length)))
        .map((s) => parseInt(s.reference.slice(prefix.length), 10))
        .filter((n) => !isNaN(n));
      let nextNum = 1;
      while (existing.includes(nextNum)) nextNum++;
      const reference = `${prefix}${nextNum}`;

      // Add a minimal symbol (no lib_id — will show as a box)
      store.pushUndo();
      const newData = structuredClone(data);
      newData.symbols.push({
        uuid: crypto.randomUUID(),
        lib_id: "",
        reference,
        value,
        footprint: "",
        position: { x, y },
        rotation: 0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: { position: { x: x + 1, y: y - 2 }, rotation: 0, font_size: 1.27, justify_h: "left", justify_v: "center", hidden: false },
        val_text: { position: { x: x + 1, y: y + 2 }, rotation: 0, font_size: 1.27, justify_h: "left", justify_v: "center", hidden: false },
        fields_autoplaced: true,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: {},
      });
      useSchematicStore.setState({ data: newData, dirty: true });

      return { success: true, message: `Added ${reference} = ${value} at (${x}, ${y})` };
    }

    case "add_wire": {
      const start: SchPoint = { x: Number(input.start_x || 0), y: Number(input.start_y || 0) };
      const end: SchPoint = { x: Number(input.end_x || 0), y: Number(input.end_y || 0) };
      store.pushUndo();
      const newData = structuredClone(data);
      newData.wires.push({ uuid: crypto.randomUUID(), start, end });
      useSchematicStore.setState({ data: newData, dirty: true });
      return { success: true, message: `Added wire from (${start.x}, ${start.y}) to (${end.x}, ${end.y})` };
    }

    case "set_component_value": {
      const ref = String(input.reference || "");
      const newValue = String(input.value || "");
      const sym = data.symbols.find((s) => s.reference === ref);
      if (!sym) return { success: false, message: `Component ${ref} not found` };
      store.updateSymbolProp(sym.uuid, "value", newValue);
      return { success: true, message: `Set ${ref} value to ${newValue}` };
    }

    case "add_net_label": {
      const text = String(input.text || "");
      const x = Number(input.x || 0);
      const y = Number(input.y || 0);
      store.placeNetLabel({ x, y }, text);
      return { success: true, message: `Added net label "${text}" at (${x}, ${y})` };
    }

    case "run_erc": {
      const result = runErc(data);
      const errors = result.violations.filter((v) => v.severity === "error");
      const warnings = result.violations.filter((v) => v.severity === "warning");
      let msg = `ERC: ${errors.length} errors, ${warnings.length} warnings\n`;
      for (const v of result.violations.slice(0, 15)) {
        msg += `- [${v.severity}] ${v.message}\n`;
      }
      if (result.violations.length > 15) {
        msg += `... and ${result.violations.length - 15} more\n`;
      }
      return { success: true, message: msg };
    }

    default:
      return { success: false, message: `Unknown tool: ${name}` };
  }
}
