import { useMemo, useCallback } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { cn } from "@/lib/utils";
import type { SchPoint } from "@/types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function mmToDisplay(mm: number, unit: "mm" | "mil" | "inch"): string {
  if (unit === "mil") return (mm / 0.0254).toFixed(0);
  if (unit === "inch") return (mm / 25.4).toFixed(4);
  return mm.toFixed(2);
}

function rotationLabel(deg: number): string {
  const n = ((deg % 360) + 360) % 360;
  return `${n} Degrees`;
}

// Unified row shape for every object kind
interface ListRow {
  uuid: string;
  kind: string;
  x: number;
  y: number;
  orientation: number;
  color: string; // hex color string
}

const KIND_COLORS: Record<string, string> = {
  Component:     "#89b4fa", // blue
  "Power Port":  "#f38ba8", // red
  Wire:          "#a6e3a1", // green
  Junction:      "#f9e2af", // yellow
  "Net Label":   "#cba6f7", // mauve
  "Global Label":"#94e2d5", // teal
  "Hier Label":  "#fab387", // peach
  Bus:           "#74c7ec", // sapphire
  "Bus Entry":   "#74c7ec",
  "No Connect":  "#9399b2", // overlay1
  "Text Note":   "#bac2de", // subtext1
  "Sheet":       "#f5c2e7", // pink
  Rectangle:     "#9399b2",
  Drawing:       "#9399b2",
};

function positionOf(obj: { position?: SchPoint; start?: SchPoint; center?: SchPoint }): SchPoint {
  if (obj.position) return obj.position;
  if ((obj as any).start) return (obj as any).start;
  if ((obj as any).center) return (obj as any).center;
  return { x: 0, y: 0 };
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ListPanel() {
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const units = useEditorStore((s) => s.statusBar.units);

  // Build a flat list of all objects
  const allRows: ListRow[] = useMemo(() => {
    if (!data) return [];
    const rows: ListRow[] = [];

    // Components (non-power symbols)
    for (const sym of data.symbols) {
      if (sym.is_power) {
        rows.push({
          uuid: sym.uuid,
          kind: "Power Port",
          x: sym.position.x,
          y: sym.position.y,
          orientation: sym.rotation,
          color: KIND_COLORS["Power Port"],
        });
      } else {
        rows.push({
          uuid: sym.uuid,
          kind: "Component",
          x: sym.position.x,
          y: sym.position.y,
          orientation: sym.rotation,
          color: KIND_COLORS["Component"],
        });
      }
    }

    // Wires
    for (const w of data.wires) {
      rows.push({
        uuid: w.uuid,
        kind: "Wire",
        x: w.start.x,
        y: w.start.y,
        orientation: 0,
        color: KIND_COLORS["Wire"],
      });
    }

    // Junctions
    for (const j of data.junctions) {
      rows.push({
        uuid: j.uuid,
        kind: "Junction",
        x: j.position.x,
        y: j.position.y,
        orientation: 0,
        color: KIND_COLORS["Junction"],
      });
    }

    // Labels
    for (const l of data.labels) {
      const kindMap: Record<string, string> = {
        Net: "Net Label",
        Global: "Global Label",
        Hierarchical: "Hier Label",
        Power: "Power Port",
      };
      rows.push({
        uuid: l.uuid,
        kind: kindMap[l.label_type] ?? "Net Label",
        x: l.position.x,
        y: l.position.y,
        orientation: l.rotation,
        color: KIND_COLORS[kindMap[l.label_type] ?? "Net Label"],
      });
    }

    // Buses
    for (const b of data.buses) {
      rows.push({
        uuid: b.uuid,
        kind: "Bus",
        x: b.start.x,
        y: b.start.y,
        orientation: 0,
        color: KIND_COLORS["Bus"],
      });
    }

    // Bus entries
    for (const be of data.bus_entries) {
      rows.push({
        uuid: be.uuid,
        kind: "Bus Entry",
        x: be.position.x,
        y: be.position.y,
        orientation: 0,
        color: KIND_COLORS["Bus Entry"],
      });
    }

    // No connects
    for (const nc of data.no_connects) {
      rows.push({
        uuid: nc.uuid,
        kind: "No Connect",
        x: nc.position.x,
        y: nc.position.y,
        orientation: 0,
        color: KIND_COLORS["No Connect"],
      });
    }

    // Text notes
    for (const t of data.text_notes) {
      rows.push({
        uuid: t.uuid,
        kind: "Text Note",
        x: t.position.x,
        y: t.position.y,
        orientation: t.rotation,
        color: KIND_COLORS["Text Note"],
      });
    }

    // Child sheets
    for (const cs of data.child_sheets) {
      rows.push({
        uuid: cs.uuid,
        kind: "Sheet",
        x: cs.position.x,
        y: cs.position.y,
        orientation: 0,
        color: KIND_COLORS["Sheet"],
      });
    }

    // Rectangles
    for (const r of data.rectangles) {
      rows.push({
        uuid: r.uuid,
        kind: "Rectangle",
        x: r.start.x,
        y: r.start.y,
        orientation: 0,
        color: KIND_COLORS["Rectangle"],
      });
    }

    // Drawings
    for (const d of data.drawings) {
      const pos = positionOf(d as any);
      rows.push({
        uuid: d.uuid,
        kind: "Drawing",
        x: pos.x,
        y: pos.y,
        orientation: 0,
        color: KIND_COLORS["Drawing"],
      });
    }

    return rows;
  }, [data]);

  // If objects are selected, show only those; otherwise show all
  const displayRows = useMemo(() => {
    if (selectedIds.size === 0) return allRows;
    return allRows.filter((r) => selectedIds.has(r.uuid));
  }, [allRows, selectedIds]);

  const totalCount = allRows.length;
  const selectedCount = selectedIds.size;

  const handleRowClick = useCallback(
    (uuid: string, e: React.MouseEvent) => {
      if (e.ctrlKey || e.metaKey) {
        useSchematicStore.getState().toggleSelect(uuid);
      } else {
        useSchematicStore.getState().select(uuid);
      }
    },
    [],
  );

  if (!data) {
    return (
      <div className="p-4 text-xs text-text-muted/50">No document loaded</div>
    );
  }

  return (
    <div className="text-xs flex flex-col h-full select-none">
      {/* Header bar */}
      <div className="px-3 py-1.5 border-b border-border-subtle bg-bg-surface/80 text-[10px] text-text-muted/70 shrink-0 leading-tight">
        Edit selected objects from all project documents include all types of
        objects
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        <table className="w-full border-collapse">
          <thead className="sticky top-0 z-10 bg-bg-surface">
            <tr className="border-b border-border-subtle text-left">
              <th className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium">
                Object Kind
              </th>
              <th className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium text-right w-[72px]">
                X1
              </th>
              <th className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium text-right w-[72px]">
                Y1
              </th>
              <th className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium w-[90px]">
                Orientation
              </th>
              <th className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium w-[48px]">
                Color
              </th>
            </tr>
          </thead>
          <tbody>
            {displayRows.map((row) => {
              const isSelected = selectedIds.has(row.uuid);
              return (
                <tr
                  key={row.uuid}
                  onClick={(e) => handleRowClick(row.uuid, e)}
                  className={cn(
                    "border-b border-border-subtle/20 cursor-pointer transition-colors",
                    isSelected
                      ? "bg-accent/15 hover:bg-accent/20"
                      : "hover:bg-bg-hover/50",
                  )}
                >
                  <td className="px-2 py-0.5 text-[10px] text-text-secondary">
                    {row.kind}
                  </td>
                  <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/70 tabular-nums text-right">
                    {mmToDisplay(row.x, units)}
                  </td>
                  <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/70 tabular-nums text-right">
                    {mmToDisplay(row.y, units)}
                  </td>
                  <td className="px-2 py-0.5 text-[10px] text-text-muted/70">
                    {rotationLabel(row.orientation)}
                  </td>
                  <td className="px-2 py-0.5">
                    <div
                      className="w-3 h-3 rounded-sm border border-white/10"
                      style={{ backgroundColor: row.color }}
                    />
                  </td>
                </tr>
              );
            })}
            {displayRows.length === 0 && (
              <tr>
                <td
                  colSpan={5}
                  className="px-2 py-6 text-center text-text-muted/40 text-[10px]"
                >
                  No objects in document
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Status bar */}
      <div className="px-3 py-1 border-t border-border-subtle text-[10px] text-text-muted/50 shrink-0">
        {totalCount} Object{totalCount !== 1 ? "s" : ""} ({selectedCount}{" "}
        Selected)
      </div>
    </div>
  );
}
