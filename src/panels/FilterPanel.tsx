import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { Eye, EyeOff, MousePointer2 } from "lucide-react";
import { cn } from "@/lib/utils";

interface FilterItem {
  label: string;
  key: string;
  count: number;
  visible: boolean;
  selectable: boolean;
}

export function FilterPanel() {
  const data = useSchematicStore((s) => s.data);
  const filter = useEditorStore((s) => s.selectionFilter);
  const setFilterItem = useEditorStore((s) => s.setFilterItem);
  const resetFilter = useEditorStore((s) => s.resetFilter);

  if (!data) {
    return <div className="p-4 text-xs text-text-muted/50">No document loaded</div>;
  }

  const items: FilterItem[] = [
    { label: "Components", key: "components", count: data.symbols.filter(s => !s.is_power).length, ...filter.components },
    { label: "Wires", key: "wires", count: data.wires.length, ...filter.wires },
    { label: "Buses", key: "buses", count: data.buses.length, ...filter.buses },
    { label: "Net Labels", key: "labels", count: data.labels.filter(l => l.label_type === "Net").length, ...filter.labels },
    { label: "Power Ports", key: "powerPorts", count: data.labels.filter(l => l.label_type === "Power").length + data.symbols.filter(s => s.is_power).length, ...filter.powerPorts },
    { label: "Junctions", key: "junctions", count: data.junctions.length, ...filter.junctions },
    { label: "No Connects", key: "noConnects", count: data.no_connects.length, ...filter.noConnects },
    { label: "Texts", key: "textNotes", count: data.text_notes.length, ...filter.textNotes },
    { label: "Drawing Objects", key: "drawings", count: data.drawings.length, ...filter.drawings },
    { label: "Sheet Symbols", key: "sheetSymbols", count: data.child_sheets.length, ...filter.sheetSymbols },
  ];

  return (
    <div className="text-xs">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border-subtle">
        <span className="text-[11px] font-semibold text-text-secondary">Selection Filter</span>
        <button onClick={resetFilter} className="text-[10px] text-accent hover:underline">All On</button>
      </div>

      <div className="flex items-center px-3 py-1 border-b border-border-subtle/50 text-[9px] text-text-muted/50 uppercase tracking-wider">
        <span className="flex-1">Object Type</span>
        <span className="w-10 text-center">Vis</span>
        <span className="w-10 text-center">Sel</span>
        <span className="w-12 text-right">Count</span>
      </div>

      {items.map(item => (
        <div key={item.key} className="flex items-center px-3 py-1 hover:bg-bg-hover/50 transition-colors">
          <span className={cn("flex-1 text-[11px]", item.visible ? "text-text-secondary" : "text-text-muted/30")}>
            {item.label}
          </span>
          <button onClick={() => setFilterItem(item.key, "visible", !item.visible)} className="w-10 flex justify-center">
            {item.visible
              ? <Eye size={12} className="text-accent" />
              : <EyeOff size={12} className="text-text-muted/20" />}
          </button>
          <button onClick={() => setFilterItem(item.key, "selectable", !item.selectable)} className="w-10 flex justify-center">
            <MousePointer2 size={12} className={item.selectable ? "text-accent" : "text-text-muted/20"} />
          </button>
          <span className="w-12 text-right text-[10px] font-mono text-text-muted/60 tabular-nums">{item.count}</span>
        </div>
      ))}
    </div>
  );
}
