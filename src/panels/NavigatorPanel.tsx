import { useSchematicStore } from "@/stores/schematic";
import { ChevronDown, ChevronRight, Cpu, Cable, Tag, Zap, Type, Circle } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";

export function NavigatorPanel() {
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  if (!data) {
    return <div className="p-4 text-xs text-text-muted/50">No document loaded</div>;
  }

  const components = data.symbols.filter(s => !s.is_power);
  const powerPorts = data.symbols.filter(s => s.is_power);

  return (
    <div className="text-xs overflow-y-auto">
      {/* Summary */}
      <div className="px-3 py-2 border-b border-border-subtle">
        <div className="text-[11px] font-semibold text-text-secondary mb-1">Schematic Overview</div>
        <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[10px]">
          <span className="text-text-muted/60">Paper</span><span className="text-text-primary">{data.paper_size}</span>
          <span className="text-text-muted/60">Components</span><span className="text-text-primary">{components.length}</span>
          <span className="text-text-muted/60">Wires</span><span className="text-text-primary">{data.wires.length}</span>
          <span className="text-text-muted/60">Nets</span><span className="text-text-primary">{data.labels.length}</span>
          <span className="text-text-muted/60">Sheets</span><span className="text-text-primary">{data.child_sheets.length + 1}</span>
        </div>
      </div>

      {/* Object tree */}
      <TreeSection icon={<Cpu size={11} />} label={`Components (${components.length})`} defaultOpen>
        {components
          .sort((a, b) => a.reference.localeCompare(b.reference, undefined, { numeric: true }))
          .map(sym => (
            <TreeItem key={sym.uuid} uuid={sym.uuid} selected={selectedIds.has(sym.uuid)}
              label={sym.reference} detail={sym.value} />
          ))}
      </TreeSection>

      <TreeSection icon={<Cable size={11} />} label={`Wires (${data.wires.length})`}>
        <div className="px-3 py-1 text-[10px] text-text-muted/40">{data.wires.length} wire segments</div>
      </TreeSection>

      <TreeSection icon={<Tag size={11} />} label={`Net Labels (${data.labels.filter(l => l.label_type === "Net").length})`}>
        {data.labels.filter(l => l.label_type === "Net").map(l => (
          <TreeItem key={l.uuid} uuid={l.uuid} selected={selectedIds.has(l.uuid)}
            label={l.text} detail="Net" />
        ))}
      </TreeSection>

      <TreeSection icon={<Zap size={11} />} label={`Power (${powerPorts.length + data.labels.filter(l => l.label_type === "Power").length})`}>
        {data.labels.filter(l => l.label_type === "Power").map(l => (
          <TreeItem key={l.uuid} uuid={l.uuid} selected={selectedIds.has(l.uuid)}
            label={l.text} detail="Power" />
        ))}
      </TreeSection>

      {data.text_notes.length > 0 && (
        <TreeSection icon={<Type size={11} />} label={`Text Notes (${data.text_notes.length})`}>
          {data.text_notes.map(t => (
            <TreeItem key={t.uuid} uuid={t.uuid} selected={selectedIds.has(t.uuid)}
              label={t.text.slice(0, 30)} detail="" />
          ))}
        </TreeSection>
      )}

      {data.child_sheets.length > 0 && (
        <TreeSection icon={<Circle size={11} />} label={`Sheets (${data.child_sheets.length})`}>
          {data.child_sheets.map(s => (
            <TreeItem key={s.uuid} uuid={s.uuid} selected={selectedIds.has(s.uuid)}
              label={s.name} detail={s.filename} />
          ))}
        </TreeSection>
      )}
    </div>
  );
}

function TreeSection({ icon, label, children, defaultOpen = false }: {
  icon: React.ReactNode; label: string; children: React.ReactNode; defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div>
      <button onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 w-full px-3 py-1.5 text-[11px] hover:bg-bg-hover/50 transition-colors text-text-secondary">
        {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        <span className="text-text-muted/60">{icon}</span>
        <span className="font-semibold">{label}</span>
      </button>
      {open && <div className="ml-2">{children}</div>}
    </div>
  );
}

function TreeItem({ uuid, selected, label, detail }: {
  uuid: string; selected: boolean; label: string; detail: string;
}) {
  return (
    <button
      onClick={() => useSchematicStore.getState().select(uuid)}
      className={cn(
        "flex items-center justify-between w-full px-4 py-0.5 text-[10px] transition-colors",
        selected ? "bg-accent/15 text-accent" : "text-text-muted hover:bg-bg-hover/50 hover:text-text-primary"
      )}>
      <span className="truncate">{label}</span>
      {detail && <span className="text-text-muted/40 ml-2 shrink-0">{detail}</span>}
    </button>
  );
}
