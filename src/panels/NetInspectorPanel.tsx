import { useMemo, useState } from "react";
import { usePcbStore } from "@/stores/pcb";
import { generateNetReport } from "@/lib/pcbOutputFormats";
import { cn } from "@/lib/utils";

export function NetInspectorPanel() {
  const pcbData = usePcbStore((s) => s.data);
  const [sortCol, setSortCol] = useState<"name" | "segments" | "vias" | "totalLength">("name");
  const [sortAsc, setSortAsc] = useState(true);
  const [filter, setFilter] = useState("");

  const report = useMemo(() => {
    if (!pcbData) return [];
    return generateNetReport(pcbData);
  }, [pcbData]);

  const filtered = useMemo(() => {
    let rows = report;
    if (filter) {
      const q = filter.toLowerCase();
      rows = rows.filter((r) => r.name.toLowerCase().includes(q));
    }
    rows = [...rows].sort((a, b) => {
      const av = a[sortCol], bv = b[sortCol];
      if (typeof av === "string" && typeof bv === "string") return sortAsc ? av.localeCompare(bv) : bv.localeCompare(av);
      return sortAsc ? (av as number) - (bv as number) : (bv as number) - (av as number);
    });
    return rows;
  }, [report, sortCol, sortAsc, filter]);

  const toggleSort = (col: typeof sortCol) => {
    if (sortCol === col) setSortAsc(!sortAsc);
    else { setSortCol(col); setSortAsc(true); }
  };

  if (!pcbData) {
    return (
      <div className="flex flex-col h-full">
        <div className="px-3 py-2 border-b border-border-subtle bg-bg-secondary/80 text-[10px] font-semibold text-text-secondary">
          Net Inspector
        </div>
        <div className="flex-1 flex items-center justify-center p-4">
          <span className="text-[10px] text-text-muted/40">No PCB data loaded</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full text-xs select-none">
      <div className="flex items-center gap-2 px-2 py-1.5 border-b border-border-subtle bg-bg-secondary/80 shrink-0">
        <span className="text-[10px] font-semibold text-accent uppercase tracking-wider">Net Inspector</span>
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => e.stopPropagation()}
          placeholder="Filter..."
          className="flex-1 bg-bg-surface border border-border-subtle rounded px-1.5 py-0.5 text-[9px] outline-none focus:border-accent"
        />
        <span className="text-[9px] text-text-muted">{filtered.length} nets</span>
      </div>

      <div className="flex items-center px-2 py-0.5 border-b border-border-subtle bg-bg-secondary/40 text-[9px] font-semibold text-text-muted/50 shrink-0">
        <span className="flex-1 cursor-pointer hover:text-text-primary" onClick={() => toggleSort("name")}>
          Net {sortCol === "name" ? (sortAsc ? "\u25b2" : "\u25bc") : ""}
        </span>
        <span className="w-[50px] text-right cursor-pointer hover:text-text-primary" onClick={() => toggleSort("segments")}>
          Segs {sortCol === "segments" ? (sortAsc ? "\u25b2" : "\u25bc") : ""}
        </span>
        <span className="w-[40px] text-right cursor-pointer hover:text-text-primary" onClick={() => toggleSort("vias")}>
          Vias {sortCol === "vias" ? (sortAsc ? "\u25b2" : "\u25bc") : ""}
        </span>
        <span className="w-[60px] text-right cursor-pointer hover:text-text-primary" onClick={() => toggleSort("totalLength")}>
          Length {sortCol === "totalLength" ? (sortAsc ? "\u25b2" : "\u25bc") : ""}
        </span>
        <span className="w-[40px] text-right">Status</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {filtered.map((r) => (
          <div key={r.number}
            className="flex items-center px-2 py-[2px] border-b border-border-subtle/20 hover:bg-bg-hover/30 cursor-pointer text-[10px]"
            onClick={() => {
              usePcbStore.getState().setNetColor(r.number, "#ff0");
            }}
          >
            <span className="flex-1 font-mono truncate text-text-primary">{r.name || `Net ${r.number}`}</span>
            <span className="w-[50px] text-right text-text-muted">{r.segments}</span>
            <span className="w-[40px] text-right text-text-muted">{r.vias}</span>
            <span className="w-[60px] text-right text-text-muted font-mono">{r.totalLength.toFixed(2)}</span>
            <span className={cn("w-[40px] text-right text-[9px]", r.routed ? "text-success" : "text-warning")}>
              {r.routed ? "OK" : "OPEN"}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
