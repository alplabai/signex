import { useState, useCallback } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { MousePointer2 } from "lucide-react";
import { cn } from "@/lib/utils";

export function PropertiesPanel() {
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  if (!data || selectedIds.size === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-6">
        <MousePointer2 size={28} className="text-text-muted/20" />
        <span className="text-text-muted/50">No selection</span>
        <span className="text-text-muted/30 text-[11px]">Click an object to inspect</span>
      </div>
    );
  }

  const selectedSymbols = data.symbols.filter((s) => selectedIds.has(s.uuid));
  const selectedWires = data.wires.filter((w) => selectedIds.has(w.uuid));
  const selectedLabels = data.labels.filter((l) => selectedIds.has(l.uuid));
  const selectedJunctions = data.junctions.filter((j) => selectedIds.has(j.uuid));

  // Single component
  if (selectedSymbols.length === 1 && selectedWires.length === 0 && selectedLabels.length === 0) {
    return <SymbolProps uuid={selectedSymbols[0].uuid} />;
  }

  // Single wire
  if (selectedWires.length === 1 && selectedSymbols.length === 0 && selectedLabels.length === 0) {
    const wire = selectedWires[0];
    return (
      <div className="p-3 text-xs space-y-3">
        <SectionHeader>Wire</SectionHeader>
        <PropRow label="From" value={`(${wire.start.x.toFixed(2)}, ${wire.start.y.toFixed(2)})`} />
        <PropRow label="To" value={`(${wire.end.x.toFixed(2)}, ${wire.end.y.toFixed(2)})`} />
        <PropRow label="Length" value={`${Math.hypot(wire.end.x - wire.start.x, wire.end.y - wire.start.y).toFixed(2)} mm`} />
      </div>
    );
  }

  // Single label
  if (selectedLabels.length === 1 && selectedSymbols.length === 0 && selectedWires.length === 0) {
    return <LabelProps uuid={selectedLabels[0].uuid} />;
  }

  // Multiple selection
  const total = selectedSymbols.length + selectedWires.length + selectedLabels.length + selectedJunctions.length;
  return (
    <div className="p-3 text-xs space-y-2">
      <SectionHeader>Selection ({total})</SectionHeader>
      <div className="text-text-muted space-y-1">
        {selectedSymbols.length > 0 && <div>{selectedSymbols.length} component(s)</div>}
        {selectedWires.length > 0 && <div>{selectedWires.length} wire(s)</div>}
        {selectedLabels.length > 0 && <div>{selectedLabels.length} label(s)</div>}
        {selectedJunctions.length > 0 && <div>{selectedJunctions.length} junction(s)</div>}
      </div>
    </div>
  );
}

/** Editable component properties */
function SymbolProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const updateSymbolProp = useSchematicStore((s) => s.updateSymbolProp);
  const sym = data?.symbols.find((s) => s.uuid === uuid);
  if (!sym) return null;

  return (
    <div className="p-3 text-xs space-y-3">
      <SectionHeader>Component</SectionHeader>
      <EditableRow label="Reference" value={sym.reference} onCommit={(v) => updateSymbolProp(uuid, "reference", v)} />
      <EditableRow label="Value" value={sym.value} onCommit={(v) => updateSymbolProp(uuid, "value", v)} />
      <EditableRow label="Footprint" value={sym.footprint} onCommit={(v) => updateSymbolProp(uuid, "footprint", v)} />
      <PropRow label="Library" value={sym.lib_id} />
      <div className="h-px bg-border-subtle my-2" />
      <SectionHeader>Position</SectionHeader>
      <EditableRow label="X" value={sym.position.x.toFixed(2)} onCommit={(v) => updateSymbolProp(uuid, "x", v)} suffix="mm" />
      <EditableRow label="Y" value={sym.position.y.toFixed(2)} onCommit={(v) => updateSymbolProp(uuid, "y", v)} suffix="mm" />
      <EditableRow label="Rotation" value={String(sym.rotation)} onCommit={(v) => updateSymbolProp(uuid, "rotation", v)} suffix="°" />
      {(sym.mirror_x || sym.mirror_y) && (
        <PropRow label="Mirror" value={[sym.mirror_x && "X", sym.mirror_y && "Y"].filter(Boolean).join(", ")} />
      )}
      <div className="h-px bg-border-subtle my-2" />
      <PropRow label="Unit" value={String(sym.unit)} />
      <PropRow label="UUID" value={sym.uuid.slice(0, 8) + "..."} />
    </div>
  );
}

/** Editable label properties */
function LabelProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const updateLabelProp = useSchematicStore((s) => s.updateLabelProp);
  const label = data?.labels.find((l) => l.uuid === uuid);
  if (!label) return null;

  return (
    <div className="p-3 text-xs space-y-3">
      <SectionHeader>{label.label_type} Label</SectionHeader>
      <EditableRow label="Text" value={label.text} onCommit={(v) => updateLabelProp(uuid, "text", v)} />
      <PropRow label="Type" value={label.label_type} />
      {label.shape && <PropRow label="Shape" value={label.shape} />}
      <div className="h-px bg-border-subtle my-2" />
      <SectionHeader>Position</SectionHeader>
      <EditableRow label="X" value={label.position.x.toFixed(2)} onCommit={(v) => updateLabelProp(uuid, "x", v)} suffix="mm" />
      <EditableRow label="Y" value={label.position.y.toFixed(2)} onCommit={(v) => updateLabelProp(uuid, "y", v)} suffix="mm" />
      <PropRow label="Rotation" value={`${label.rotation}°`} />
    </div>
  );
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">
      {children}
    </div>
  );
}

function PropRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-text-muted shrink-0">{label}</span>
      <span className="text-text-primary truncate text-right font-mono text-[10px]">{value}</span>
    </div>
  );
}

/** Editable property row — click to edit, Enter/blur to commit */
function EditableRow({
  label, value, onCommit, suffix,
}: {
  label: string; value: string; onCommit: (v: string) => void; suffix?: string;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  const startEdit = useCallback(() => {
    setDraft(value);
    setEditing(true);
  }, [value]);

  const commit = useCallback(() => {
    setEditing(false);
    if (draft !== value) onCommit(draft);
  }, [draft, value, onCommit]);

  if (editing) {
    return (
      <div className="flex items-center justify-between gap-2">
        <span className="text-text-muted shrink-0">{label}</span>
        <div className="flex items-center gap-1">
          <input
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") setEditing(false);
            }}
            className="w-full max-w-[120px] bg-bg-primary border border-accent/40 rounded px-1.5 py-0.5 text-[10px] font-mono text-text-primary outline-none focus:border-accent"
          />
          {suffix && <span className="text-text-muted/50 text-[10px]">{suffix}</span>}
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between gap-2 group cursor-pointer" onClick={startEdit}>
      <span className="text-text-muted shrink-0">{label}</span>
      <span className={cn(
        "truncate text-right font-mono text-[10px] group-hover:text-accent transition-colors",
        "text-text-primary"
      )}>
        {value}{suffix ? ` ${suffix}` : ""}
      </span>
    </div>
  );
}
