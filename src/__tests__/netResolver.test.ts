import { describe, it, expect } from "vitest";
import { resolveNets } from "@/lib/netResolver";
import { createSimpleSchematic } from "./fixtures/simple";

describe("netResolver", () => {
  it("groups connected wires into a single net", () => {
    const data = createSimpleSchematic();
    const nets = resolveNets(data);
    // w1 and w2 share endpoint (20, 16.19) → should be same net
    const wireNets = nets.filter(n => n.wireUuids.length > 0);
    expect(wireNets.length).toBeGreaterThanOrEqual(1);
    // Both wires should be in the same net
    const netWithBothWires = nets.find(n => n.wireUuids.includes("w1") && n.wireUuids.includes("w2"));
    expect(netWithBothWires).toBeDefined();
  });

  it("assigns label text as net name", () => {
    const data = createSimpleSchematic();
    const nets = resolveNets(data);
    const vccNet = nets.find(n => n.name === "VCC");
    expect(vccNet).toBeDefined();
    expect(vccNet!.wireUuids.length).toBeGreaterThanOrEqual(1);
  });

  it("detects disconnected wires as separate nets", () => {
    const data = createSimpleSchematic();
    // Add a disconnected wire far away
    data.wires.push({ uuid: "w-isolated", start: { x: 100, y: 100 }, end: { x: 110, y: 100 } });
    const nets = resolveNets(data);
    const isolatedNet = nets.find(n => n.wireUuids.includes("w-isolated"));
    expect(isolatedNet).toBeDefined();
    // Isolated wire should NOT be in VCC net
    const vccNet = nets.find(n => n.name === "VCC");
    expect(vccNet?.wireUuids.includes("w-isolated")).toBe(false);
  });

  it("junctions connect wires at T-intersections", () => {
    const data = createSimpleSchematic();
    // Add a vertical wire at the junction point + junction
    data.wires.push({ uuid: "w-branch", start: { x: 20, y: 16.19 }, end: { x: 20, y: 30 } });
    data.junctions.push({ uuid: "j1", position: { x: 20, y: 16.19 } });
    const nets = resolveNets(data);
    // All 3 wires should be in same net
    const fullNet = nets.find(n => n.wireUuids.includes("w1") && n.wireUuids.includes("w-branch"));
    expect(fullNet).toBeDefined();
  });

  it("returns empty array for empty schematic", () => {
    const data = createSimpleSchematic();
    data.symbols = [];
    data.wires = [];
    data.labels = [];
    data.junctions = [];
    const nets = resolveNets(data);
    expect(nets.length).toBe(0);
  });
});
