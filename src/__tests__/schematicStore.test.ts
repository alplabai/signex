import { describe, it, expect, beforeEach } from "vitest";
import { useSchematicStore } from "@/stores/schematic";
import { createSimpleSchematic } from "./fixtures/simple";

describe("schematicStore", () => {
  beforeEach(() => {
    // Reset store before each test
    const data = createSimpleSchematic();
    useSchematicStore.getState().loadSchematic(data);
  });

  describe("selection", () => {
    it("select sets single selection", () => {
      useSchematicStore.getState().select("sym-R1");
      // Must re-read state after mutation (Zustand creates new Set)
      const sel = useSchematicStore.getState().selectedIds;
      expect(sel.has("sym-R1")).toBe(true);
      expect(sel.size).toBe(1);
    });

    it("toggleSelect adds/removes from selection", () => {
      const store = useSchematicStore.getState();
      store.select("sym-R1");
      store.toggleSelect("sym-R2");
      const s = useSchematicStore.getState();
      expect(s.selectedIds.has("sym-R1")).toBe(true);
      expect(s.selectedIds.has("sym-R2")).toBe(true);
      // Toggle again removes
      s.toggleSelect("sym-R1");
      expect(useSchematicStore.getState().selectedIds.has("sym-R1")).toBe(false);
    });

    it("deselectAll clears selection", () => {
      const store = useSchematicStore.getState();
      store.select("sym-R1");
      store.deselectAll();
      expect(useSchematicStore.getState().selectedIds.size).toBe(0);
    });

    it("selectAll selects all objects", () => {
      useSchematicStore.getState().selectAll();
      const sel = useSchematicStore.getState().selectedIds;
      expect(sel.size).toBeGreaterThan(0);
      expect(sel.has("sym-R1")).toBe(true);
      expect(sel.has("w1")).toBe(true);
      expect(sel.has("l1")).toBe(true);
      expect(sel.has("nc1")).toBe(true);
      expect(sel.has("tn1")).toBe(true);
    });
  });

  describe("editing", () => {
    it("deleteSelected removes selected elements", () => {
      const store = useSchematicStore.getState();
      store.select("w1");
      store.deleteSelected();
      const data = useSchematicStore.getState().data!;
      expect(data.wires.find(w => w.uuid === "w1")).toBeUndefined();
      expect(data.wires.length).toBe(1); // w2 remains
    });

    it("moveElements moves symbol position", () => {
      const store = useSchematicStore.getState();
      const origX = store.data!.symbols[0].position.x;
      store.moveElements(["sym-R1"], 5, 3);
      const newX = useSchematicStore.getState().data!.symbols[0].position.x;
      expect(newX).toBeCloseTo(origX + 5);
    });

    it("breakWire splits wire into two + junction", () => {
      const store = useSchematicStore.getState();
      const origWireCount = store.data!.wires.length;
      const origJuncCount = store.data!.junctions.length;
      store.breakWire("w1", { x: 15, y: 16.19 });
      const data = useSchematicStore.getState().data!;
      expect(data.wires.length).toBe(origWireCount + 1);
      expect(data.junctions.length).toBe(origJuncCount + 1);
    });
  });

  describe("undo/redo", () => {
    it("undo restores previous state", () => {
      const store = useSchematicStore.getState();
      const origCount = store.data!.wires.length;
      store.select("w1");
      store.deleteSelected();
      expect(useSchematicStore.getState().data!.wires.length).toBe(origCount - 1);
      useSchematicStore.getState().undo();
      expect(useSchematicStore.getState().data!.wires.length).toBe(origCount);
    });

    it("redo reapplies undone change", () => {
      const store = useSchematicStore.getState();
      store.select("w1");
      store.deleteSelected();
      useSchematicStore.getState().undo();
      useSchematicStore.getState().redo();
      expect(useSchematicStore.getState().data!.wires.find(w => w.uuid === "w1")).toBeUndefined();
    });
  });

  describe("wire drawing", () => {
    it("startWire → addWirePoint → finishWire creates wire segments", () => {
      const store = useSchematicStore.getState();
      const origWireCount = store.data!.wires.length;
      store.startWire({ x: 0, y: 0 });
      expect(useSchematicStore.getState().wireDrawing.active).toBe(true);
      store.addWirePoint({ x: 10, y: 0 });
      store.finishWire();
      const data = useSchematicStore.getState().data!;
      expect(data.wires.length).toBeGreaterThan(origWireCount);
      expect(useSchematicStore.getState().wireDrawing.active).toBe(false);
    });

    it("cancelWire does not create wires", () => {
      const store = useSchematicStore.getState();
      const origWireCount = store.data!.wires.length;
      store.startWire({ x: 0, y: 0 });
      store.cancelWire();
      expect(useSchematicStore.getState().data!.wires.length).toBe(origWireCount);
    });
  });

  describe("clipboard", () => {
    it("copySelected + pasteClipboard duplicates elements", () => {
      const store = useSchematicStore.getState();
      store.select("sym-R1");
      store.copySelected();
      const origCount = useSchematicStore.getState().data!.symbols.length;
      useSchematicStore.getState().pasteClipboard({ x: 5, y: 5 });
      expect(useSchematicStore.getState().data!.symbols.length).toBe(origCount + 1);
    });

    it("pasted elements have new UUIDs", () => {
      const store = useSchematicStore.getState();
      store.select("sym-R1");
      store.copySelected();
      useSchematicStore.getState().pasteClipboard({ x: 5, y: 5 });
      const syms = useSchematicStore.getState().data!.symbols;
      const uuids = syms.map(s => s.uuid);
      const uniqueUuids = new Set(uuids);
      expect(uniqueUuids.size).toBe(uuids.length); // All unique
    });
  });

  describe("annotation", () => {
    it("annotateAll assigns sequential designators", () => {
      const store = useSchematicStore.getState();
      // Make both symbols unannotated
      const data = store.data!;
      data.symbols[0].reference = "R?";
      data.symbols[1].reference = "R?";
      useSchematicStore.setState({ data: { ...data } });
      useSchematicStore.getState().annotateAll();
      const syms = useSchematicStore.getState().data!.symbols;
      const refs = syms.filter(s => !s.is_power).map(s => s.reference);
      expect(refs).toContain("R1");
      expect(refs).toContain("R2");
    });
  });

  describe("findSimilar", () => {
    it("selects all objects of same type/lib_id", () => {
      const store = useSchematicStore.getState();
      store.select("sym-R1");
      store.findSimilar();
      const sel = useSchematicStore.getState().selectedIds;
      expect(sel.has("sym-R1")).toBe(true);
      expect(sel.has("sym-R2")).toBe(true); // Same lib_id
    });
  });

  describe("properties", () => {
    it("updateSymbolProp changes symbol value", () => {
      useSchematicStore.getState().updateSymbolProp("sym-R1", "value", "22k");
      const sym = useSchematicStore.getState().data!.symbols.find(s => s.uuid === "sym-R1");
      expect(sym!.value).toBe("22k");
    });

    it("updateSymbolProp with x=0 works (not treated as falsy)", () => {
      useSchematicStore.getState().updateSymbolProp("sym-R1", "x", "0");
      const sym = useSchematicStore.getState().data!.symbols.find(s => s.uuid === "sym-R1");
      expect(sym!.position.x).toBe(0);
    });

    it("updateLabelProp changes label text", () => {
      useSchematicStore.getState().updateLabelProp("l1", "text", "GND");
      const label = useSchematicStore.getState().data!.labels.find(l => l.uuid === "l1");
      expect(label!.text).toBe("GND");
    });
  });
});
