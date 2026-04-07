/**
 * PCB Placement Tools — Altium-style alignment and distribution.
 */

import { usePcbStore } from "@/stores/pcb";

type Direction = "left" | "right" | "top" | "bottom" | "centerH" | "centerV";

/**
 * Align selected footprints along a given direction.
 */
export function alignFootprints(direction: Direction) {
  const store = usePcbStore.getState();
  const { data, selectedIds } = store;
  if (!data || selectedIds.size < 2) return;

  const footprints = data.footprints.filter((f) => selectedIds.has(f.uuid));
  if (footprints.length < 2) return;

  store.pushUndo();
  const nd = structuredClone(data);
  const selected = nd.footprints.filter((f) => selectedIds.has(f.uuid));

  let target: number;
  switch (direction) {
    case "left": target = Math.min(...selected.map((f) => f.position.x)); break;
    case "right": target = Math.max(...selected.map((f) => f.position.x)); break;
    case "top": target = Math.min(...selected.map((f) => f.position.y)); break;
    case "bottom": target = Math.max(...selected.map((f) => f.position.y)); break;
    case "centerH": target = selected.reduce((s, f) => s + f.position.x, 0) / selected.length; break;
    case "centerV": target = selected.reduce((s, f) => s + f.position.y, 0) / selected.length; break;
    default: return;
  }

  for (const fp of selected) {
    if (direction === "left" || direction === "right" || direction === "centerH") {
      fp.position.x = target;
    } else {
      fp.position.y = target;
    }
  }

  usePcbStore.setState({ data: nd, dirty: true });
}

/**
 * Distribute selected footprints evenly.
 */
export function distributeFootprints(axis: "horizontal" | "vertical") {
  const store = usePcbStore.getState();
  const { data, selectedIds } = store;
  if (!data || selectedIds.size < 3) return;

  store.pushUndo();
  const nd = structuredClone(data);
  const selected = nd.footprints.filter((f) => selectedIds.has(f.uuid));

  if (axis === "horizontal") {
    selected.sort((a, b) => a.position.x - b.position.x);
    const min = selected[0].position.x;
    const max = selected[selected.length - 1].position.x;
    const step = (max - min) / (selected.length - 1);
    selected.forEach((f, i) => { f.position.x = min + i * step; });
  } else {
    selected.sort((a, b) => a.position.y - b.position.y);
    const min = selected[0].position.y;
    const max = selected[selected.length - 1].position.y;
    const step = (max - min) / (selected.length - 1);
    selected.forEach((f, i) => { f.position.y = min + i * step; });
  }

  usePcbStore.setState({ data: nd, dirty: true });
}

/**
 * Swap positions of two selected footprints.
 */
export function swapFootprints() {
  const store = usePcbStore.getState();
  const { data, selectedIds } = store;
  if (!data || selectedIds.size !== 2) return;

  const uuids = [...selectedIds];
  store.pushUndo();
  const nd = structuredClone(data);
  const fpA = nd.footprints.find((f) => f.uuid === uuids[0]);
  const fpB = nd.footprints.find((f) => f.uuid === uuids[1]);
  if (!fpA || !fpB) return;

  const tempPos = { ...fpA.position };
  const tempRot = fpA.rotation;
  const tempLayer = fpA.layer;
  fpA.position = { ...fpB.position };
  fpA.rotation = fpB.rotation;
  fpA.layer = fpB.layer;
  fpB.position = tempPos;
  fpB.rotation = tempRot;
  fpB.layer = tempLayer;

  usePcbStore.setState({ data: nd, dirty: true });
}

/**
 * Snap all selected footprints to the grid.
 */
export function snapFootprintsToGrid() {
  const store = usePcbStore.getState();
  const { data, selectedIds } = store;
  if (!data || selectedIds.size === 0) return;

  const gs = data.board.setup.gridSize || 1.27;
  const snap = (v: number) => Math.round(v / gs) * gs;

  store.pushUndo();
  const nd = structuredClone(data);
  for (const fp of nd.footprints) {
    if (!selectedIds.has(fp.uuid)) continue;
    fp.position.x = snap(fp.position.x);
    fp.position.y = snap(fp.position.y);
  }
  usePcbStore.setState({ data: nd, dirty: true });
}
