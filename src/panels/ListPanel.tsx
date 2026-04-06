import { useState } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { cn } from "@/lib/utils";

type ListTab = "components" | "wires" | "labels" | "nets";

function mmToDisplay(mm: number, unit: "mm" | "mil" | "inch"): string {
  if (unit === "mil") return (mm / 0.0254).toFixed(0);
  if (unit === "inch") return (mm / 25.4).toFixed(4);
  return mm.toFixed(2);
}

export function ListPanel() {
  const data = useSchematicStore((s) => s.data);
  const units = useEditorStore((s) => s.statusBar.units);
  const [tab, setTab] = useState<ListTab>("components");
  const [sortCol, setSortCol] = useState<string>("ref");
  const [sortAsc, setSortAsc] = useState(true);

  if (!data) {
    return <div className="p-4 text-xs text-text-muted/50">No document loaded</div>;
  }

  const handleSort = (col: string) => {
    if (sortCol === col) setSortAsc(!sortAsc);
    else { setSortCol(col); setSortAsc(true); }
  };

  const selectItem = (uuid: string) => {
    useSchematicStore.getState().select(uuid);
  };

  const SortHeader = ({ col, label, className }: { col: string; label: string; className?: string }) => (
    <button onClick={() => handleSort(col)}
      className={cn("text-left text-[9px] uppercase tracking-wider py-1 hover:text-accent transition-colors",
        sortCol === col ? "text-accent" : "text-text-muted/50", className)}>
      {label} {sortCol === col ? (sortAsc ? "\u25b2" : "\u25bc") : ""}
    </button>
  );

  return (
    <div className="text-xs flex flex-col h-full">
      {/* Tabs */}
      <div className="flex border-b border-border-subtle shrink-0">
        {(["components", "wires", "labels", "nets"] as const).map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={cn("px-3 py-1.5 text-[10px] capitalize transition-colors border-b-2",
              tab === t ? "border-accent text-accent font-semibold" : "border-transparent text-text-muted hover:text-text-secondary")}>
            {t}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {tab === "components" && (
          <table className="w-full">
            <thead className="sticky top-0 bg-bg-surface/95">
              <tr className="border-b border-border-subtle">
                <th className="px-2"><SortHeader col="ref" label="Ref" /></th>
                <th className="px-2"><SortHeader col="value" label="Value" /></th>
                <th className="px-2"><SortHeader col="footprint" label="Footprint" /></th>
                <th className="px-2"><SortHeader col="x" label="X" /></th>
                <th className="px-2"><SortHeader col="y" label="Y" /></th>
              </tr>
            </thead>
            <tbody>
              {data.symbols
                .filter(s => !s.is_power)
                .sort((a, b) => {
                  const va = sortCol === "ref" ? a.reference : sortCol === "value" ? a.value : sortCol === "footprint" ? a.footprint : sortCol === "x" ? String(a.position.x) : String(a.position.y);
                  const vb = sortCol === "ref" ? b.reference : sortCol === "value" ? b.value : sortCol === "footprint" ? b.footprint : sortCol === "x" ? String(b.position.x) : String(b.position.y);
                  return sortAsc ? va.localeCompare(vb, undefined, { numeric: true }) : vb.localeCompare(va, undefined, { numeric: true });
                })
                .map(sym => (
                  <tr key={sym.uuid} onClick={() => selectItem(sym.uuid)}
                    className="border-b border-border-subtle/30 hover:bg-bg-hover/50 cursor-pointer transition-colors">
                    <td className="px-2 py-0.5 font-mono text-[10px] text-accent">{sym.reference}</td>
                    <td className="px-2 py-0.5 font-mono text-[10px]">{sym.value}</td>
                    <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/60 truncate max-w-[120px]">{sym.footprint}</td>
                    <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/60 tabular-nums">{mmToDisplay(sym.position.x, units)}</td>
                    <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/60 tabular-nums">{mmToDisplay(sym.position.y, units)}</td>
                  </tr>
                ))}
            </tbody>
          </table>
        )}

        {tab === "wires" && (
          <table className="w-full">
            <thead className="sticky top-0 bg-bg-surface/95">
              <tr className="border-b border-border-subtle">
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">#</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Start</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">End</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Length</th>
              </tr>
            </thead>
            <tbody>
              {data.wires.map((w, i) => {
                const len = Math.hypot(w.end.x - w.start.x, w.end.y - w.start.y);
                return (
                  <tr key={w.uuid} onClick={() => selectItem(w.uuid)}
                    className="border-b border-border-subtle/30 hover:bg-bg-hover/50 cursor-pointer transition-colors">
                    <td className="px-2 py-0.5 text-[10px] text-text-muted/40">{i + 1}</td>
                    <td className="px-2 py-0.5 font-mono text-[10px]">{mmToDisplay(w.start.x, units)}, {mmToDisplay(w.start.y, units)}</td>
                    <td className="px-2 py-0.5 font-mono text-[10px]">{mmToDisplay(w.end.x, units)}, {mmToDisplay(w.end.y, units)}</td>
                    <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/60">{mmToDisplay(len, units)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}

        {tab === "labels" && (
          <table className="w-full">
            <thead className="sticky top-0 bg-bg-surface/95">
              <tr className="border-b border-border-subtle">
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Name</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Type</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Position</th>
              </tr>
            </thead>
            <tbody>
              {data.labels.map(l => (
                <tr key={l.uuid} onClick={() => selectItem(l.uuid)}
                  className="border-b border-border-subtle/30 hover:bg-bg-hover/50 cursor-pointer transition-colors">
                  <td className="px-2 py-0.5 font-mono text-[10px] text-accent">{l.text}</td>
                  <td className="px-2 py-0.5 text-[10px] text-text-muted/60">{l.label_type}</td>
                  <td className="px-2 py-0.5 font-mono text-[10px] text-text-muted/60">{mmToDisplay(l.position.x, units)}, {mmToDisplay(l.position.y, units)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {tab === "nets" && (
          <div className="p-3 text-text-muted/50 text-[10px]">
            Run ERC to see resolved nets. Use F5 for net color overlay.
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="px-3 py-1 border-t border-border-subtle text-[10px] text-text-muted/40 shrink-0">
        {tab === "components" && `${data.symbols.filter(s => !s.is_power).length} components`}
        {tab === "wires" && `${data.wires.length} wires`}
        {tab === "labels" && `${data.labels.length} labels`}
        {tab === "nets" && "Net browser"}
      </div>
    </div>
  );
}
