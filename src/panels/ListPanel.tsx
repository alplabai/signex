import { useState, useMemo } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { resolveNets } from "@/lib/netResolver";
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
  const [editingCell, setEditingCell] = useState<{ uuid: string; field: string } | null>(null);
  const [editValue, setEditValue] = useState("");

  // Resolve nets for the nets tab
  const nets = useMemo(() => {
    if (!data) return [];
    return resolveNets(data);
  }, [data]);

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

  const startEdit = (uuid: string, field: string, currentValue: string) => {
    setEditingCell({ uuid, field });
    setEditValue(currentValue);
  };

  const commitEdit = () => {
    if (!editingCell) return;
    const { uuid, field } = editingCell;
    useSchematicStore.getState().updateSymbolProp(uuid, field, editValue);
    setEditingCell(null);
  };

  const cancelEdit = () => {
    setEditingCell(null);
  };

  const EditableCell = ({ uuid, field, value, className }: { uuid: string; field: string; value: string; className?: string }) => {
    const isEditing = editingCell?.uuid === uuid && editingCell?.field === field;
    if (isEditing) {
      return (
        <td className={cn("px-2 py-0", className)}>
          <input autoFocus value={editValue}
            onChange={(e) => setEditValue(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") commitEdit();
              if (e.key === "Escape") cancelEdit();
            }}
            onBlur={commitEdit}
            className="w-full bg-bg-primary border border-accent rounded px-1 py-0 text-[10px] font-mono outline-none" />
        </td>
      );
    }
    return (
      <td className={cn("px-2 py-0.5 font-mono text-[10px] cursor-text", className)}
        onDoubleClick={() => startEdit(uuid, field, value)}>
        {value}
      </td>
    );
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
      <div className="flex border-b border-border-subtle shrink-0">
        {(["components", "wires", "labels", "nets"] as const).map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={cn("px-3 py-1.5 text-[10px] capitalize transition-colors border-b-2",
              tab === t ? "border-accent text-accent font-semibold" : "border-transparent text-text-muted hover:text-text-secondary")}>
            {t}
          </button>
        ))}
      </div>

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
                    <EditableCell uuid={sym.uuid} field="reference" value={sym.reference} className="text-accent" />
                    <EditableCell uuid={sym.uuid} field="value" value={sym.value} />
                    <EditableCell uuid={sym.uuid} field="footprint" value={sym.footprint} className="text-text-muted/60 truncate max-w-[120px]" />
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
          <table className="w-full">
            <thead className="sticky top-0 bg-bg-surface/95">
              <tr className="border-b border-border-subtle">
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Net Name</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Pins</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Wires</th>
                <th className="px-2 text-left text-[9px] text-text-muted/50 uppercase py-1">Labels</th>
              </tr>
            </thead>
            <tbody>
              {nets.map((net, i) => (
                <tr key={net.name || `net-${i}`}
                  className="border-b border-border-subtle/30 hover:bg-bg-hover/50 transition-colors">
                  <td className="px-2 py-0.5 font-mono text-[10px] text-accent">{net.name || "(unnamed)"}</td>
                  <td className="px-2 py-0.5 text-[10px] text-text-muted/60 tabular-nums">{net.pins.length}</td>
                  <td className="px-2 py-0.5 text-[10px] text-text-muted/60 tabular-nums">{net.wireUuids.length}</td>
                  <td className="px-2 py-0.5 text-[10px] text-text-muted/60 tabular-nums">{net.labelUuids.length}</td>
                </tr>
              ))}
              {nets.length === 0 && (
                <tr><td colSpan={4} className="px-2 py-4 text-center text-text-muted/40">No nets resolved</td></tr>
              )}
            </tbody>
          </table>
        )}
      </div>

      <div className="px-3 py-1 border-t border-border-subtle text-[10px] text-text-muted/40 shrink-0">
        {tab === "components" && `${data.symbols.filter(s => !s.is_power).length} components`}
        {tab === "wires" && `${data.wires.length} wires`}
        {tab === "labels" && `${data.labels.length} labels`}
        {tab === "nets" && `${nets.length} nets`}
      </div>
    </div>
  );
}
