import { useMemo, useCallback, useState } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { zoomToObject } from "@/lib/crossProbe";
import { cn } from "@/lib/utils";

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
  return `${n}\u00b0`;
}

interface ListRow {
  uuid: string;
  kind: string;
  name: string;
  x: number;
  y: number;
  orientation: number;
  color: string;
}

const KIND_COLORS: Record<string, string> = {
  Component:     "#89b4fa",
  "Power Port":  "#f38ba8",
  Wire:          "#a6e3a1",
  Junction:      "#f9e2af",
  "Net Label":   "#cba6f7",
  "Global Label":"#94e2d5",
  "Hier Label":  "#fab387",
  Bus:           "#74c7ec",
  "Bus Entry":   "#74c7ec",
  "No Connect":  "#9399b2",
  "Text Note":   "#bac2de",
  "Sheet":       "#f5c2e7",
  Rectangle:     "#9399b2",
  Drawing:       "#9399b2",
};

const ALL_KINDS = Object.keys(KIND_COLORS);

type SortKey = "kind" | "name" | "x" | "y" | "orientation";

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ListPanel() {
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const units = useEditorStore((s) => s.statusBar.units);

  const [kindFilter, setKindFilter] = useState("All");
  const [sortCol, setSortCol] = useState<SortKey | null>(null);
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");

  // Build flat list of all objects
  const allRows: ListRow[] = useMemo(() => {
    if (!data) return [];
    const rows: ListRow[] = [];

    for (const sym of data.symbols) {
      const isPower = sym.is_power;
      rows.push({
        uuid: sym.uuid,
        kind: isPower ? "Power Port" : "Component",
        name: isPower ? sym.value : sym.reference,
        x: sym.position.x, y: sym.position.y,
        orientation: sym.rotation,
        color: KIND_COLORS[isPower ? "Power Port" : "Component"],
      });
    }

    for (const w of data.wires) rows.push({ uuid: w.uuid, kind: "Wire", name: "--", x: w.start.x, y: w.start.y, orientation: 0, color: KIND_COLORS.Wire });
    for (const j of data.junctions) rows.push({ uuid: j.uuid, kind: "Junction", name: "--", x: j.position.x, y: j.position.y, orientation: 0, color: KIND_COLORS.Junction });

    for (const l of data.labels) {
      const km: Record<string, string> = { Net: "Net Label", Global: "Global Label", Hierarchical: "Hier Label", Power: "Power Port" };
      const kind = km[l.label_type] ?? "Net Label";
      rows.push({ uuid: l.uuid, kind, name: l.text, x: l.position.x, y: l.position.y, orientation: l.rotation, color: KIND_COLORS[kind] });
    }

    for (const b of data.buses) rows.push({ uuid: b.uuid, kind: "Bus", name: "--", x: b.start.x, y: b.start.y, orientation: 0, color: KIND_COLORS.Bus });
    for (const be of data.bus_entries) rows.push({ uuid: be.uuid, kind: "Bus Entry", name: "--", x: be.position.x, y: be.position.y, orientation: 0, color: KIND_COLORS["Bus Entry"] });
    for (const nc of data.no_connects) rows.push({ uuid: nc.uuid, kind: "No Connect", name: "--", x: nc.position.x, y: nc.position.y, orientation: 0, color: KIND_COLORS["No Connect"] });

    for (const t of data.text_notes) rows.push({ uuid: t.uuid, kind: "Text Note", name: t.text?.slice(0, 30) || "--", x: t.position.x, y: t.position.y, orientation: t.rotation, color: KIND_COLORS["Text Note"] });
    for (const cs of data.child_sheets) rows.push({ uuid: cs.uuid, kind: "Sheet", name: cs.name || cs.filename, x: cs.position.x, y: cs.position.y, orientation: 0, color: KIND_COLORS.Sheet });
    for (const r of data.rectangles) rows.push({ uuid: r.uuid, kind: "Rectangle", name: "--", x: r.start.x, y: r.start.y, orientation: 0, color: KIND_COLORS.Rectangle });

    for (const d of data.drawings) {
      const pos = (d as any).position || (d as any).start || (d as any).center || { x: 0, y: 0 };
      rows.push({ uuid: d.uuid, kind: "Drawing", name: "--", x: pos.x, y: pos.y, orientation: 0, color: KIND_COLORS.Drawing });
    }

    return rows;
  }, [data]);

  // Filter by kind
  const filteredRows = useMemo(() => {
    if (kindFilter === "All") return allRows;
    return allRows.filter(r => r.kind === kindFilter);
  }, [allRows, kindFilter]);

  // Show only selected if any, otherwise all filtered
  const displayRows = useMemo(() => {
    if (selectedIds.size === 0) return filteredRows;
    return filteredRows.filter(r => selectedIds.has(r.uuid));
  }, [filteredRows, selectedIds]);

  // Sort
  const sortedRows = useMemo(() => {
    if (!sortCol) return displayRows;
    return [...displayRows].sort((a, b) => {
      const av = a[sortCol], bv = b[sortCol];
      if (typeof av === "number" && typeof bv === "number") return sortDir === "asc" ? av - bv : bv - av;
      return sortDir === "asc"
        ? String(av).localeCompare(String(bv), undefined, { numeric: true })
        : String(bv).localeCompare(String(av), undefined, { numeric: true });
    });
  }, [displayRows, sortCol, sortDir]);

  const handleSort = useCallback((col: SortKey) => {
    setSortCol(prev => {
      if (prev === col) {
        setSortDir(d => d === "asc" ? "desc" : "asc");
        return col;
      }
      setSortDir("asc");
      return col;
    });
  }, []);

  const handleRowClick = useCallback((uuid: string, e: React.MouseEvent) => {
    if (e.ctrlKey || e.metaKey) {
      useSchematicStore.getState().toggleSelect(uuid);
    } else {
      useSchematicStore.getState().select(uuid);
    }
  }, []);

  const handleRowDoubleClick = useCallback((uuid: string) => {
    useSchematicStore.getState().select(uuid);
    zoomToObject(uuid);
  }, []);

  const sortIndicator = (col: SortKey) => {
    if (sortCol !== col) return null;
    return <span className="ml-0.5 text-accent">{sortDir === "asc" ? "\u25b2" : "\u25bc"}</span>;
  };

  if (!data) return <div className="p-4 text-xs text-text-muted/50">No document loaded</div>;

  return (
    <div className="text-xs flex flex-col h-full select-none">
      {/* Header with filter */}
      <div className="px-2 py-1 border-b border-border-subtle bg-bg-surface/80 shrink-0 flex items-center gap-2">
        <select
          value={kindFilter}
          onChange={e => setKindFilter(e.target.value)}
          className="bg-bg-secondary border border-border-subtle rounded px-1.5 py-0.5 text-[10px] text-text-secondary outline-none"
        >
          <option value="All">All Types</option>
          {ALL_KINDS.map(k => <option key={k} value={k}>{k}</option>)}
        </select>
        <span className="text-[10px] text-text-muted/50 ml-auto">
          {sortedRows.length} / {allRows.length}
        </span>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        <table className="w-full border-collapse">
          <thead className="sticky top-0 z-10 bg-bg-surface">
            <tr className="border-b border-border-subtle text-left">
              <th onClick={() => handleSort("kind")} className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium cursor-pointer hover:text-accent">
                Kind{sortIndicator("kind")}
              </th>
              <th onClick={() => handleSort("name")} className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium cursor-pointer hover:text-accent">
                Name{sortIndicator("name")}
              </th>
              <th onClick={() => handleSort("x")} className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium text-right w-[68px] cursor-pointer hover:text-accent">
                X{sortIndicator("x")}
              </th>
              <th onClick={() => handleSort("y")} className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium text-right w-[68px] cursor-pointer hover:text-accent">
                Y{sortIndicator("y")}
              </th>
              <th onClick={() => handleSort("orientation")} className="px-2 py-1 text-[9px] uppercase tracking-wider text-text-muted/50 font-medium w-[48px] cursor-pointer hover:text-accent">
                Rot{sortIndicator("orientation")}
              </th>
              <th className="px-2 py-1 w-[24px]" />
            </tr>
          </thead>
          <tbody>
            {sortedRows.map(row => {
              const isSelected = selectedIds.has(row.uuid);
              return (
                <tr
                  key={row.uuid}
                  onClick={e => handleRowClick(row.uuid, e)}
                  onDoubleClick={() => handleRowDoubleClick(row.uuid)}
                  className={cn(
                    "border-b border-border-subtle/20 cursor-pointer transition-colors",
                    isSelected ? "bg-accent/15 hover:bg-accent/20" : "hover:bg-bg-hover/50",
                  )}
                >
                  <td className="px-2 py-0.5 text-[10px] text-text-secondary">{row.kind}</td>
                  <td className="px-2 py-0.5 text-[10px] font-mono text-text-primary truncate max-w-[120px]">{row.name}</td>
                  <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/70 tabular-nums text-right">{mmToDisplay(row.x, units)}</td>
                  <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/70 tabular-nums text-right">{mmToDisplay(row.y, units)}</td>
                  <td className="px-2 py-0.5 text-[10px] text-text-muted/70">{rotationLabel(row.orientation)}</td>
                  <td className="px-1 py-0.5">
                    <div className="w-2.5 h-2.5 rounded-sm border border-white/10" style={{ backgroundColor: row.color }} />
                  </td>
                </tr>
              );
            })}
            {sortedRows.length === 0 && (
              <tr><td colSpan={6} className="px-2 py-6 text-center text-text-muted/40 text-[10px]">No objects</td></tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Status bar */}
      <div className="px-3 py-1 border-t border-border-subtle text-[10px] text-text-muted/50 shrink-0">
        {allRows.length} Objects ({selectedIds.size} Selected){kindFilter !== "All" ? ` \u2014 Filter: ${kindFilter}` : ""}
      </div>
    </div>
  );
}
