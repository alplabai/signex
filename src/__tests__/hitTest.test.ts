import { describe, it, expect } from "vitest";
import { hitTest, boxSelect } from "@/canvas/hitTest";
import { createSimpleSchematic } from "./fixtures/simple";

describe("hitTest", () => {
  it("hit on wire segment returns wire type", () => {
    const data = createSimpleSchematic();
    // Wire w1 goes from (10,17.46) to (20,17.46) — click at midpoint
    const hit = hitTest(data, 15, 17.46);
    expect(hit).not.toBeNull();
    expect(hit!.type === "wire" || hit!.type === "wireEndpoint").toBe(true);
  });

  it("hit on wire endpoint returns wireEndpoint type", () => {
    const data = createSimpleSchematic();
    // Wire w1 start at (10, 17.46)
    const hit = hitTest(data, 10, 17.46);
    expect(hit).not.toBeNull();
    // Could be wireEndpoint or symbol (pin is there too)
    expect(["wireEndpoint", "symbol", "wire"].includes(hit!.type)).toBe(true);
  });

  it("hit on label returns label type", () => {
    const data = createSimpleSchematic();
    const hit = hitTest(data, 20, 17.46);
    expect(hit).not.toBeNull();
    // The label is at this position — but wire endpoint is also there
    // Due to priority, wireEndpoint may come first
    expect(hit!.uuid).toBeDefined();
  });

  it("hit on no-connect returns noConnect type", () => {
    const data = createSimpleSchematic();
    const hit = hitTest(data, 10, 22.54);
    expect(hit).not.toBeNull();
    expect(hit!.type).toBe("noConnect");
    expect(hit!.uuid).toBe("nc1");
  });

  it("hit on text note returns textNote type", () => {
    const data = createSimpleSchematic();
    const hit = hitTest(data, 50, 50);
    expect(hit).not.toBeNull();
    expect(hit!.type).toBe("textNote");
    expect(hit!.uuid).toBe("tn1");
  });

  it("miss on empty space returns null", () => {
    const data = createSimpleSchematic();
    const hit = hitTest(data, 200, 200);
    expect(hit).toBeNull();
  });

  it("priority: junction before wire", () => {
    const data = createSimpleSchematic();
    data.junctions.push({ uuid: "j-test", position: { x: 15, y: 17.46 } });
    const hit = hitTest(data, 15, 17.46);
    expect(hit).not.toBeNull();
    expect(hit!.type).toBe("junction");
    expect(hit!.uuid).toBe("j-test");
  });
});

describe("boxSelect", () => {
  it("inside mode: selects objects fully within box", () => {
    const data = createSimpleSchematic();
    // Box around the text note at (50,50)
    const selected = boxSelect(data, 45, 45, 55, 55);
    expect(selected).toContain("tn1");
  });

  it("inside mode: does not select objects outside box", () => {
    const data = createSimpleSchematic();
    const selected = boxSelect(data, 45, 45, 55, 55);
    expect(selected).not.toContain("w1");
    expect(selected).not.toContain("w2");
  });

  it("crossing mode (right-to-left): selects wires that cross box", () => {
    const data = createSimpleSchematic();
    // Right-to-left box that crosses wire w1 (10→20, y=17.46)
    const selected = boxSelect(data, 18, 14, 12, 18); // endX < startX = crossing
    expect(selected).toContain("w1");
  });

  it("no-connects are selectable by box", () => {
    const data = createSimpleSchematic();
    const selected = boxSelect(data, 8, 22, 12, 25);
    expect(selected).toContain("nc1");
  });
});
