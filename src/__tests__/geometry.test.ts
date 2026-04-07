import { describe, it, expect } from "vitest";
import { symToSchPoint, dist, pointsMatch } from "@/lib/geometry";
import type { SchSymbol } from "@/types";

const defaultTextProp = { position: { x: 0, y: 0 }, rotation: 0, font_size: 1.27, justify_h: "center", justify_v: "center", hidden: false };

function makeSym(x: number, y: number, rotation: number, mx = false, my = false): SchSymbol {
  return {
    uuid: "test", lib_id: "test", reference: "R1", value: "10k", footprint: "",
    position: { x, y }, rotation, mirror_x: mx, mirror_y: my, unit: 1, is_power: false,
    ref_text: defaultTextProp, val_text: defaultTextProp, fields_autoplaced: true,
    dnp: false, in_bom: true, on_board: true, exclude_from_sim: false, locked: false,
    fields: {},
  };
}

describe("geometry", () => {
  describe("symToSchPoint", () => {
    it("no rotation: local (1,0) → schematic (sx+1, sy)", () => {
      const sym = makeSym(10, 20, 0);
      const p = symToSchPoint(1, 0, sym);
      expect(p.x).toBeCloseTo(11);
      expect(p.y).toBeCloseTo(20);
    });

    it("90 degree rotation", () => {
      const sym = makeSym(10, 20, 90);
      const p = symToSchPoint(1, 0, sym);
      // Local (1,0) → Y-flip → (1,0) → rotate -90 → (0,1) → pos offset
      expect(p.x).toBeCloseTo(10);
      expect(p.y).toBeCloseTo(19); // 20 + (-1)
    });

    it("180 degree rotation", () => {
      const sym = makeSym(10, 20, 180);
      const p = symToSchPoint(1, 0, sym);
      expect(p.x).toBeCloseTo(9);
      expect(p.y).toBeCloseTo(20);
    });

    it("270 degree rotation", () => {
      const sym = makeSym(10, 20, 270);
      const p = symToSchPoint(1, 0, sym);
      expect(p.x).toBeCloseTo(10);
      expect(p.y).toBeCloseTo(21); // 20 + 1
    });

    it("mirror_x flips Y component", () => {
      const sym = makeSym(10, 20, 0, true, false);
      const p = symToSchPoint(0, 1, sym);
      // Local (0,1) → flip Y → (0,-1) → mirror_x flips ry → (0,1) in screen
      expect(p.x).toBeCloseTo(10);
      // mirror_x negates ry after rotation
      expect(Math.abs(p.y - 20)).toBeLessThan(2);
    });

    it("mirror_y flips X component", () => {
      const sym = makeSym(10, 20, 0, false, true);
      const p = symToSchPoint(1, 0, sym);
      expect(p.x).toBeCloseTo(9); // Mirrored
      expect(p.y).toBeCloseTo(20);
    });
  });

  describe("dist", () => {
    it("same point → 0", () => {
      expect(dist({ x: 5, y: 5 }, { x: 5, y: 5 })).toBe(0);
    });

    it("horizontal distance", () => {
      expect(dist({ x: 0, y: 0 }, { x: 3, y: 0 })).toBeCloseTo(3);
    });

    it("diagonal distance", () => {
      expect(dist({ x: 0, y: 0 }, { x: 3, y: 4 })).toBeCloseTo(5);
    });
  });

  describe("pointsMatch", () => {
    it("exact match", () => {
      expect(pointsMatch({ x: 1, y: 2 }, { x: 1, y: 2 })).toBe(true);
    });

    it("within tolerance", () => {
      expect(pointsMatch({ x: 1, y: 2 }, { x: 1.01, y: 2.01 })).toBe(true);
    });

    it("outside tolerance", () => {
      expect(pointsMatch({ x: 1, y: 2 }, { x: 1.1, y: 2 })).toBe(false);
    });
  });
});
