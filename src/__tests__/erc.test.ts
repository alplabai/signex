import { describe, it, expect } from "vitest";
import { runErc } from "@/lib/erc";
import { createSimpleSchematic, createErcTestSchematic } from "./fixtures/simple";

describe("erc", () => {
  it("clean schematic has no errors", () => {
    const data = createSimpleSchematic();
    const { violations } = runErc(data);
    const errors = violations.filter(v => v.severity === "error");
    expect(errors.length).toBe(0);
  });

  it("detects duplicate designators", () => {
    const data = createErcTestSchematic();
    const { violations } = runErc(data);
    const dupes = violations.filter(v => v.type === "duplicate_designator");
    expect(dupes.length).toBeGreaterThanOrEqual(1);
    expect(dupes[0].message).toContain("R1");
  });

  it("detects unannotated components", () => {
    const data = createErcTestSchematic();
    const { violations } = runErc(data);
    const unannotated = violations.filter(v => v.type === "unannotated");
    expect(unannotated.length).toBeGreaterThanOrEqual(1);
    expect(unannotated[0].message).toContain("C?");
  });

  it("detects floating wires", () => {
    const data = createErcTestSchematic();
    const { violations } = runErc(data);
    const floating = violations.filter(v => v.type === "floating_wire");
    expect(floating.length).toBeGreaterThanOrEqual(1);
  });

  it("no-connect suppresses unconnected pin warning", () => {
    const data = createSimpleSchematic();
    // nc1 is at R1 pin2 position
    const { violations } = runErc(data);
    const unconnected = violations.filter(v => v.type === "unconnected_pin" && v.message.includes("R1"));
    // R1 pin 2 should NOT be flagged because there's a no-connect
    const pin2issues = unconnected.filter(v => v.message.includes("pin 2"));
    expect(pin2issues.length).toBe(0);
  });

  it("multiple different net names on same net warns", () => {
    const data = createSimpleSchematic();
    // Add a second label with different name on the same wire
    data.labels.push({
      uuid: "l2", text: "VDD", position: { x: 20, y: 17.46 }, rotation: 0,
      label_type: "Net", shape: "", font_size: 1.27, justify: "left",
    });
    const { violations } = runErc(data);
    const multiName = violations.filter(v => v.type === "multiple_net_names");
    expect(multiName.length).toBeGreaterThanOrEqual(1);
  });

  it("same net name labels do NOT trigger multiple name warning", () => {
    const data = createSimpleSchematic();
    // Add a second label with SAME name
    data.labels.push({
      uuid: "l2", text: "VCC", position: { x: 15, y: 17.46 }, rotation: 0,
      label_type: "Net", shape: "", font_size: 1.27, justify: "left",
    });
    const { violations } = runErc(data);
    const multiName = violations.filter(v => v.type === "multiple_net_names");
    expect(multiName.length).toBe(0);
  });
});
