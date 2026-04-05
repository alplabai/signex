import { useSchematicStore } from "@/stores/schematic";
import { MousePointer2 } from "lucide-react";

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

  // Find selected elements
  const selectedSymbols = data.symbols.filter((s) => selectedIds.has(s.uuid));
  const selectedWires = data.wires.filter((w) => selectedIds.has(w.uuid));
  const selectedLabels = data.labels.filter((l) => selectedIds.has(l.uuid));

  if (selectedSymbols.length === 1 && selectedWires.length === 0 && selectedLabels.length === 0) {
    const sym = selectedSymbols[0];
    return (
      <div className="p-3 text-xs space-y-3">
        <div className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">
          Component
        </div>
        <PropRow label="Reference" value={sym.reference} />
        <PropRow label="Value" value={sym.value} />
        <PropRow label="Library" value={sym.lib_id} />
        <PropRow label="Footprint" value={sym.footprint} />
        <div className="h-px bg-border-subtle my-2" />
        <PropRow label="X" value={`${sym.position.x.toFixed(2)} mm`} />
        <PropRow label="Y" value={`${sym.position.y.toFixed(2)} mm`} />
        <PropRow label="Rotation" value={`${sym.rotation}°`} />
        {sym.mirror_x && <PropRow label="Mirror" value="X" />}
        {sym.mirror_y && <PropRow label="Mirror" value="Y" />}
        <PropRow label="Unit" value={String(sym.unit)} />
        <PropRow label="UUID" value={sym.uuid.slice(0, 8) + "..."} />
      </div>
    );
  }

  if (selectedWires.length === 1 && selectedSymbols.length === 0) {
    const wire = selectedWires[0];
    return (
      <div className="p-3 text-xs space-y-3">
        <div className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">
          Wire
        </div>
        <PropRow label="From" value={`(${wire.start.x.toFixed(2)}, ${wire.start.y.toFixed(2)})`} />
        <PropRow label="To" value={`(${wire.end.x.toFixed(2)}, ${wire.end.y.toFixed(2)})`} />
        <PropRow label="Length" value={`${Math.hypot(wire.end.x - wire.start.x, wire.end.y - wire.start.y).toFixed(2)} mm`} />
      </div>
    );
  }

  if (selectedLabels.length === 1 && selectedSymbols.length === 0) {
    const label = selectedLabels[0];
    return (
      <div className="p-3 text-xs space-y-3">
        <div className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">
          Label
        </div>
        <PropRow label="Text" value={label.text} />
        <PropRow label="Type" value={label.label_type} />
        <PropRow label="X" value={`${label.position.x.toFixed(2)} mm`} />
        <PropRow label="Y" value={`${label.position.y.toFixed(2)} mm`} />
      </div>
    );
  }

  return (
    <div className="p-3 text-xs space-y-2">
      <div className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">
        Multiple Selection
      </div>
      <div className="text-text-muted">
        {selectedSymbols.length > 0 && <div>{selectedSymbols.length} component(s)</div>}
        {selectedWires.length > 0 && <div>{selectedWires.length} wire(s)</div>}
        {selectedLabels.length > 0 && <div>{selectedLabels.length} label(s)</div>}
      </div>
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
