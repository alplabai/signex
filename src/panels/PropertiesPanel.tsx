import { useState, useCallback } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { MousePointer2, Eye, EyeOff, ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "@/lib/utils";

export function PropertiesPanel() {
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  if (!data) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-6">
        <MousePointer2 size={28} className="text-text-muted/20" />
        <span className="text-text-muted/50">No document</span>
      </div>
    );
  }

  // Nothing selected → document properties
  if (selectedIds.size === 0) {
    return <DocumentProps />;
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
    return <WireProps uuid={selectedWires[0].uuid} />;
  }

  // Single label (Net / Global / Power / Hierarchical)
  if (selectedLabels.length === 1 && selectedSymbols.length === 0 && selectedWires.length === 0) {
    return <LabelProps uuid={selectedLabels[0].uuid} />;
  }

  // Single junction
  if (selectedJunctions.length === 1 && selectedSymbols.length === 0 && selectedWires.length === 0 && selectedLabels.length === 0) {
    return <JunctionProps uuid={selectedJunctions[0].uuid} />;
  }

  // Multiple selection
  const total = selectedSymbols.length + selectedWires.length + selectedLabels.length + selectedJunctions.length;
  return (
    <div className="p-3 text-xs space-y-2">
      <SectionHeader>Selection ({total})</SectionHeader>
      <div className="text-text-muted space-y-1 text-[11px]">
        {selectedSymbols.length > 0 && <div>{selectedSymbols.length} component(s)</div>}
        {selectedWires.length > 0 && <div>{selectedWires.length} wire(s)</div>}
        {selectedLabels.length > 0 && <div>{selectedLabels.length} label(s)</div>}
        {selectedJunctions.length > 0 && <div>{selectedJunctions.length} junction(s)</div>}
      </div>
      <div className="h-px bg-border-subtle my-2" />
      <div className="text-[10px] text-text-muted/40">
        Common properties for batch editing will appear here when supported types are selected.
      </div>
    </div>
  );
}

// ─── Document Properties (nothing selected) ────────────────────────

function DocumentProps() {
  const data = useSchematicStore((s) => s.data);
  const gridSize = useEditorStore((s) => s.statusBar.gridSize);
  const snapEnabled = useEditorStore((s) => s.statusBar.snapEnabled);
  const gridVisible = useEditorStore((s) => s.gridVisible);
  const units = useEditorStore((s) => s.statusBar.units);

  return (
    <div className="p-3 text-xs space-y-3">
      <SectionHeader>Document Options</SectionHeader>

      <Section title="General">
        <PropRow label="Paper" value={data?.paper_size || "A4"} />
        <PropRow label="UUID" value={data?.uuid.slice(0, 12) + "..." || ""} />
        <PropRow label="Generator" value={data?.generator || ""} />
      </Section>

      <Section title="Grid">
        <EditableRow label="Snap Grid" value={String(gridSize)} suffix={units}
          onCommit={(v) => useEditorStore.getState().setGridSize(parseFloat(v) || gridSize)} />
        <ToggleRow label="Snap Enabled" value={snapEnabled}
          onToggle={() => useEditorStore.getState().toggleSnap()} />
        <ToggleRow label="Grid Visible" value={gridVisible}
          onToggle={() => useEditorStore.getState().toggleGrid()} />
        <DropdownRow label="Units" value={units} options={["mm", "mil", "inch"]}
          onChange={(v) => useEditorStore.getState().updateStatusBar({ units: v as "mm" | "mil" | "inch" })} />
      </Section>

      <Section title="Statistics">
        <PropRow label="Components" value={String(data?.symbols.filter(s => !s.is_power).length ?? 0)} />
        <PropRow label="Wires" value={String(data?.wires.length ?? 0)} />
        <PropRow label="Labels" value={String(data?.labels.length ?? 0)} />
        <PropRow label="Junctions" value={String(data?.junctions.length ?? 0)} />
        <PropRow label="Sheets" value={String((data?.child_sheets.length ?? 0) + 1)} />
      </Section>
    </div>
  );
}

// ─── Component Properties ──────────────────────────────────────────

function SymbolProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const updateSymbolProp = useSchematicStore((s) => s.updateSymbolProp);
  const sym = data?.symbols.find((s) => s.uuid === uuid);
  if (!sym) return null;

  return (
    <div className="p-3 text-xs space-y-3">
      <SectionHeader>Component</SectionHeader>

      <Section title="General">
        <EditableRow label="Designator" value={sym.reference} onCommit={(v) => updateSymbolProp(uuid, "reference", v)} />
        <EditableRow label="Value" value={sym.value} onCommit={(v) => updateSymbolProp(uuid, "value", v)} />
        <EditableRow label="Footprint" value={sym.footprint} onCommit={(v) => updateSymbolProp(uuid, "footprint", v)} />
        <PropRow label="Library" value={sym.lib_id} />
        <PropRow label="Type" value={sym.is_power ? "Power" : "Standard"} />
      </Section>

      <Section title="Location">
        <EditableRow label="X" value={sym.position.x.toFixed(2)} suffix="mm" onCommit={(v) => updateSymbolProp(uuid, "x", v)} />
        <EditableRow label="Y" value={sym.position.y.toFixed(2)} suffix="mm" onCommit={(v) => updateSymbolProp(uuid, "y", v)} />
        <DropdownRow label="Rotation" value={String(sym.rotation)}
          options={["0", "90", "180", "270"]}
          onChange={(v) => updateSymbolProp(uuid, "rotation", v)} />
      </Section>

      <Section title="Graphical">
        <ToggleRow label="Mirror X" value={sym.mirror_x} onToggle={() => {
          useSchematicStore.getState().pushUndo();
          const d = useSchematicStore.getState().data;
          if (!d) return;
          const nd = structuredClone(d);
          const s = nd.symbols.find(s => s.uuid === uuid);
          if (s) s.mirror_x = !s.mirror_x;
          useSchematicStore.setState({ data: nd, dirty: true });
        }} />
        <ToggleRow label="Mirror Y" value={sym.mirror_y} onToggle={() => {
          useSchematicStore.getState().pushUndo();
          const d = useSchematicStore.getState().data;
          if (!d) return;
          const nd = structuredClone(d);
          const s = nd.symbols.find(s => s.uuid === uuid);
          if (s) s.mirror_y = !s.mirror_y;
          useSchematicStore.setState({ data: nd, dirty: true });
        }} />
        <PropRow label="Unit" value={String(sym.unit)} />
      </Section>

      <Section title="Text Properties">
        <ToggleRow label="Reference Visible" value={!sym.ref_text.hidden} onToggle={() => {
          useSchematicStore.getState().pushUndo();
          const d = useSchematicStore.getState().data;
          if (!d) return;
          const nd = structuredClone(d);
          const s = nd.symbols.find(s => s.uuid === uuid);
          if (s) s.ref_text.hidden = !s.ref_text.hidden;
          useSchematicStore.setState({ data: nd, dirty: true });
        }} />
        <ToggleRow label="Value Visible" value={!sym.val_text.hidden} onToggle={() => {
          useSchematicStore.getState().pushUndo();
          const d = useSchematicStore.getState().data;
          if (!d) return;
          const nd = structuredClone(d);
          const s = nd.symbols.find(s => s.uuid === uuid);
          if (s) s.val_text.hidden = !s.val_text.hidden;
          useSchematicStore.setState({ data: nd, dirty: true });
        }} />
        <PropRow label="Font Size" value={`${sym.ref_text.font_size} mm`} />
      </Section>

      <PropRow label="UUID" value={sym.uuid.slice(0, 12) + "..."} />
    </div>
  );
}

// ─── Wire Properties ───────────────────────────────────────────────

function WireProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const wire = data?.wires.find((w) => w.uuid === uuid);
  if (!wire) return null;

  const length = Math.hypot(wire.end.x - wire.start.x, wire.end.y - wire.start.y);

  return (
    <div className="p-3 text-xs space-y-3">
      <SectionHeader>Wire</SectionHeader>

      <Section title="Net">
        <PropRow label="Net Name" value="(unresolved)" />
      </Section>

      <Section title="Vertices">
        <PropRow label="Start X" value={wire.start.x.toFixed(2) + " mm"} />
        <PropRow label="Start Y" value={wire.start.y.toFixed(2) + " mm"} />
        <PropRow label="End X" value={wire.end.x.toFixed(2) + " mm"} />
        <PropRow label="End Y" value={wire.end.y.toFixed(2) + " mm"} />
        <PropRow label="Length" value={length.toFixed(2) + " mm"} />
      </Section>

      <PropRow label="UUID" value={wire.uuid.slice(0, 12) + "..."} />
    </div>
  );
}

// ─── Label Properties (Net / Global / Hierarchical / Power) ────────

function LabelProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const updateLabelProp = useSchematicStore((s) => s.updateLabelProp);
  const label = data?.labels.find((l) => l.uuid === uuid);
  if (!label) return null;

  const isPower = label.label_type === "Power";
  const title = isPower ? "Power Port" : label.label_type === "Global" ? "Global Label" :
    label.label_type === "Hierarchical" ? "Hierarchical Label" : "Net Label";

  return (
    <div className="p-3 text-xs space-y-3">
      <SectionHeader>{title}</SectionHeader>

      <Section title="Location">
        <EditableRow label="X" value={label.position.x.toFixed(2)} suffix="mm"
          onCommit={(v) => updateLabelProp(uuid, "x", v)} />
        <EditableRow label="Y" value={label.position.y.toFixed(2)} suffix="mm"
          onCommit={(v) => updateLabelProp(uuid, "y", v)} />
        <PropRow label="Rotation" value={`${label.rotation}°`} />
      </Section>

      <Section title="Properties">
        <EditableRow label={isPower ? "Net Name" : "Name"}
          value={label.text}
          onCommit={(v) => updateLabelProp(uuid, "text", v)} />
        <PropRow label="Type" value={label.label_type} />
        {label.shape && <PropRow label="Shape" value={label.shape} />}
        <PropRow label="Font Size" value={`${label.font_size} mm`} />
        {label.justify && <PropRow label="Justify" value={label.justify} />}
      </Section>

      {isPower && (
        <Section title="Style">
          <DropdownRow label="Style" value={label.shape || "input"}
            options={["input", "output", "bidirectional", "passive"]}
            onChange={() => {/* TODO: update label shape */}} />
        </Section>
      )}

      <PropRow label="UUID" value={label.uuid.slice(0, 12) + "..."} />
    </div>
  );
}

// ─── Junction Properties ───────────────────────────────────────────

function JunctionProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const junction = data?.junctions.find((j) => j.uuid === uuid);
  if (!junction) return null;

  return (
    <div className="p-3 text-xs space-y-3">
      <SectionHeader>Junction</SectionHeader>

      <Section title="Location">
        <PropRow label="X" value={junction.position.x.toFixed(2) + " mm"} />
        <PropRow label="Y" value={junction.position.y.toFixed(2) + " mm"} />
      </Section>

      <PropRow label="UUID" value={junction.uuid.slice(0, 12) + "..."} />
    </div>
  );
}

// ─── Reusable UI Components ────────────────────────────────────────

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">
      {children}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(true);
  return (
    <div>
      <button onClick={() => setOpen(!open)}
        className="flex items-center gap-1 text-[10px] font-semibold text-text-muted/70 uppercase tracking-wider w-full hover:text-text-secondary transition-colors mb-1">
        {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        {title}
      </button>
      {open && <div className="space-y-1.5 pl-1">{children}</div>}
    </div>
  );
}

function PropRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-text-muted/70 shrink-0 text-[11px]">{label}</span>
      <span className="text-text-primary truncate text-right font-mono text-[10px]">{value}</span>
    </div>
  );
}

function ToggleRow({ label, value, onToggle }: { label: string; value: boolean; onToggle: () => void }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-text-muted/70 shrink-0 text-[11px]">{label}</span>
      <button onClick={onToggle} className="p-0.5 rounded hover:bg-bg-hover transition-colors">
        {value ? <Eye size={13} className="text-accent" /> : <EyeOff size={13} className="text-text-muted/30" />}
      </button>
    </div>
  );
}

function DropdownRow({ label, value, options, onChange }: {
  label: string; value: string; options: string[]; onChange: (v: string) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-text-muted/70 shrink-0 text-[11px]">{label}</span>
      <select value={value} onChange={(e) => onChange(e.target.value)}
        className="bg-bg-primary border border-border-subtle rounded px-1.5 py-0.5 text-[10px] font-mono text-text-primary outline-none focus:border-accent max-w-[100px]">
        {options.map(o => <option key={o} value={o}>{o}</option>)}
      </select>
    </div>
  );
}

function EditableRow({ label, value, onCommit, suffix }: {
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
        <span className="text-text-muted/70 shrink-0 text-[11px]">{label}</span>
        <div className="flex items-center gap-1">
          <input
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") setEditing(false);
              e.stopPropagation();
            }}
            className="w-full max-w-[100px] bg-bg-primary border border-accent/40 rounded px-1.5 py-0.5 text-[10px] font-mono text-text-primary outline-none focus:border-accent"
          />
          {suffix && <span className="text-text-muted/40 text-[10px]">{suffix}</span>}
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between gap-2 group cursor-pointer" onClick={startEdit}>
      <span className="text-text-muted/70 shrink-0 text-[11px]">{label}</span>
      <span className={cn(
        "truncate text-right font-mono text-[10px] group-hover:text-accent transition-colors text-text-primary"
      )}>
        {value}{suffix ? ` ${suffix}` : ""}
      </span>
    </div>
  );
}
