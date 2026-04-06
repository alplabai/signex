import { useState } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { MousePointer2, Eye, EyeOff, ChevronDown, ChevronRight, Lock } from "lucide-react";
import { cn } from "@/lib/utils";

// ─── Unit conversion (internal = mm) ─────────────────────────────
function mmToDisplay(mm: number, unit: "mm" | "mil" | "inch"): string {
  if (unit === "mil") return (mm / 0.0254).toFixed(0);
  if (unit === "inch") return (mm / 25.4).toFixed(4);
  return mm.toFixed(2);
}
function displayToMm(val: number, unit: "mm" | "mil" | "inch"): number {
  if (unit === "mil") return val * 0.0254;
  if (unit === "inch") return val * 25.4;
  return val;
}

// ═══════════════════════════════════════════════════════════════════
// MAIN PANEL ROUTER
// ═══════════════════════════════════════════════════════════════════

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

  if (selectedIds.size === 0) return <DocumentProps />;

  const sel = {
    symbols: data.symbols.filter((s) => selectedIds.has(s.uuid)),
    wires: data.wires.filter((w) => selectedIds.has(w.uuid)),
    labels: data.labels.filter((l) => selectedIds.has(l.uuid)),
    junctions: data.junctions.filter((j) => selectedIds.has(j.uuid)),
    noConnects: data.no_connects.filter((nc) => selectedIds.has(nc.uuid)),
    textNotes: data.text_notes.filter((t) => selectedIds.has(t.uuid)),
    buses: data.buses.filter((b) => selectedIds.has(b.uuid)),
    busEntries: data.bus_entries.filter((be) => selectedIds.has(be.uuid)),
    childSheets: data.child_sheets.filter((cs) => selectedIds.has(cs.uuid)),
  };
  const total = sel.symbols.length + sel.wires.length + sel.labels.length +
    sel.junctions.length + sel.noConnects.length + sel.textNotes.length +
    sel.buses.length + sel.busEntries.length + sel.childSheets.length;

  if (sel.symbols.length === 1 && total === 1) return <ComponentProps uuid={sel.symbols[0].uuid} />;
  if (sel.wires.length === 1 && total === 1) return <WireProps uuid={sel.wires[0].uuid} />;
  if (sel.labels.length === 1 && total === 1) return <LabelProps uuid={sel.labels[0].uuid} />;
  if (sel.junctions.length === 1 && total === 1) return <JunctionProps uuid={sel.junctions[0].uuid} />;
  if (sel.noConnects.length === 1 && total === 1) return <NoConnectProps uuid={sel.noConnects[0].uuid} />;
  if (sel.textNotes.length === 1 && total === 1) return <TextNoteProps uuid={sel.textNotes[0].uuid} />;
  if (sel.buses.length === 1 && total === 1) return <BusProps uuid={sel.buses[0].uuid} />;
  if (sel.busEntries.length === 1 && total === 1) return <BusEntryProps uuid={sel.busEntries[0].uuid} />;
  if (sel.childSheets.length === 1 && total === 1) return <SheetSymbolProps uuid={sel.childSheets[0].uuid} />;

  // Multiple selection — batch editing when all same type
  const allSymbols = total === sel.symbols.length && sel.symbols.length > 1;
  const allLabels = total === sel.labels.length && sel.labels.length > 1;
  const symUuids = sel.symbols.map(s => s.uuid);
  const lblUuids = sel.labels.map(l => l.uuid);

  return (
    <div className="text-xs">
      <PanelHeader title="Selection" count={total} />
      <div className="p-3 space-y-3">
        <div className="text-text-muted space-y-0.5 text-[11px]">
          {sel.symbols.length > 0 && <div>{sel.symbols.length} Component(s)</div>}
          {sel.wires.length > 0 && <div>{sel.wires.length} Wire(s)</div>}
          {sel.labels.length > 0 && <div>{sel.labels.length} Net Label(s)</div>}
          {sel.junctions.length > 0 && <div>{sel.junctions.length} Junction(s)</div>}
          {sel.noConnects.length > 0 && <div>{sel.noConnects.length} No Connect(s)</div>}
          {sel.textNotes.length > 0 && <div>{sel.textNotes.length} Text(s)</div>}
          {sel.buses.length > 0 && <div>{sel.buses.length} Bus(es)</div>}
          {sel.busEntries.length > 0 && <div>{sel.busEntries.length} Bus Entry(ies)</div>}
          {sel.childSheets.length > 0 && <div>{sel.childSheets.length} Sheet Symbol(s)</div>}
        </div>

        {/* Batch editing for same-type selections */}
        {allSymbols && (
          <Section title="Common Properties">
            <FieldRow label="Value">
              <FieldInput value="(mixed)" onCommit={(v) => useSchematicStore.getState().updateMultipleSymbolProp(symUuids, "value", v)} />
            </FieldRow>
            <FieldRow label="Footprint">
              <FieldInput value="(mixed)" onCommit={(v) => useSchematicStore.getState().updateMultipleSymbolProp(symUuids, "footprint", v)} />
            </FieldRow>
          </Section>
        )}
        {allLabels && (
          <Section title="Common Properties">
            <FieldRow label="Net Name">
              <FieldInput value="(mixed)" onCommit={(v) => useSchematicStore.getState().updateMultipleLabelProp(lblUuids, "text", v)} />
            </FieldRow>
          </Section>
        )}
      </div>
      <div className="px-3 py-1.5 border-t border-border-subtle text-[10px] text-text-muted/40">
        {total} objects selected
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// DOCUMENT OPTIONS (Nothing Selected)
// ═══════════════════════════════════════════════════════════════════

function DocumentProps() {
  const data = useSchematicStore((s) => s.data);
  const gridSize = useEditorStore((s) => s.statusBar.gridSize);
  const snapEnabled = useEditorStore((s) => s.statusBar.snapEnabled);
  const gridVisible = useEditorStore((s) => s.gridVisible);
  const units = useEditorStore((s) => s.statusBar.units);
  const [tab, setTab] = useState<"general" | "parameters">("general");

  return (
    <div className="text-xs">
      <PanelHeader title="Document Options" />

      {/* Tabs */}
      <div className="flex border-b border-border-subtle">
        <TabBtn active={tab === "general"} onClick={() => setTab("general")}>General</TabBtn>
        <TabBtn active={tab === "parameters"} onClick={() => setTab("parameters")}>Parameters</TabBtn>
      </div>

      {tab === "general" ? (
        <div className="p-3 space-y-3">
          {/* Selection Filter */}
          <Section title="Selection Filter" defaultOpen={false}>
            <div className="flex flex-wrap gap-1">
              {["Components", "Wires", "Buses", "Sheet Symbols", "Sheet Entries", "Net Labels",
                "Parameters", "Ports", "Power Ports", "Texts", "Drawing Objects", "Other"].map(t => (
                <FilterBtn key={t} label={t} />
              ))}
            </div>
          </Section>

          {/* General */}
          <Section title="General">
            {/* Units toggle - Altium style */}
            <div className="flex items-center justify-between gap-2 mb-2">
              <span className="text-text-muted/70 text-[11px]">Units</span>
              <div className="flex rounded overflow-hidden border border-border-subtle">
                {(["mm", "mil", "inch"] as const).map(u => (
                  <button key={u} onClick={() => useEditorStore.getState().updateStatusBar({ units: u })}
                    className={cn("px-3 py-0.5 text-[10px] transition-colors",
                      units === u ? "bg-accent/30 text-accent font-semibold" : "bg-bg-primary text-text-muted hover:bg-bg-hover")}>
                    {u === "mil" ? "mils" : u}
                  </button>
                ))}
              </div>
            </div>

            <FieldRow label="Visible Grid">
              <FieldInput value={mmToDisplay(gridSize * 2, units)} suffix={units}
                onCommit={(v) => { const mm = displayToMm(parseFloat(v) || 0, units); if (mm > 0) useEditorStore.getState().setGridSize(mm / 2); }} />
              <IconBtn icon={gridVisible ? <Eye size={12} /> : <EyeOff size={12} />}
                active={gridVisible} onClick={() => useEditorStore.getState().toggleGrid()} />
            </FieldRow>

            <FieldRow label="Snap Grid">
              <CheckBox checked={snapEnabled} onChange={() => useEditorStore.getState().toggleSnap()} />
              <FieldInput value={mmToDisplay(gridSize, units)} suffix={units}
                onCommit={(v) => { const mm = displayToMm(parseFloat(v) || 0, units); if (mm > 0) useEditorStore.getState().setGridSize(mm); }} />
              <span className="text-text-muted/40 text-[10px] shrink-0">G</span>
            </FieldRow>

            <FieldRow label="">
              <CheckBox checked={true} onChange={() => {}} />
              <span className="text-[10px] text-text-secondary">Snap to Electrical Object Hotspots</span>
              <span className="text-text-muted/30 text-[10px] ml-auto shrink-0">Shift+E</span>
            </FieldRow>

            <FieldRow label="Snap Distance">
              <FieldInput value={mmToDisplay(1.0, units)} suffix={units} onCommit={() => {}} />
            </FieldRow>

            <FieldRow label="Document Font">
              <span className="text-accent text-[10px] cursor-pointer hover:underline">Roboto, 13</span>
            </FieldRow>

            <FieldRow label="Sheet Border">
              <ColorSwatch color="#1e2035" />
              <ColorSwatch color="#2a2d4a" />
            </FieldRow>

            <FieldRow label="Sheet Color">
              <ColorSwatch color="#1e2035" />
            </FieldRow>
          </Section>

          {/* Page Options */}
          <Section title="Page Options">
            <div className="flex rounded overflow-hidden border border-border-subtle mb-2">
              {["Template", "Standard", "Custom"].map(t => (
                <button key={t} className={cn("flex-1 px-1.5 py-0.5 text-[10px] transition-colors",
                  t === "Standard" ? "bg-accent/30 text-accent font-semibold" : "bg-bg-primary text-text-muted hover:bg-bg-hover")}>
                  {t}
                </button>
              ))}
            </div>
            <FieldRow label="Paper Size">
              <select value={data?.paper_size || "A4"} onChange={() => {}}
                className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none focus:border-accent">
                {["A4", "A3", "A2", "A1", "A0", "A", "B", "C", "D"].map(o => <option key={o} value={o}>{o}</option>)}
              </select>
            </FieldRow>
            <FieldRow label="Orientation">
              <span className="text-[10px] font-mono text-text-primary">Landscape</span>
            </FieldRow>
          </Section>

          {/* Margin and Zones */}
          <Section title="Margin and Zones" defaultOpen={false}>
            <FieldRow label="">
              <CheckBox checked={false} onChange={() => {}} />
              <span className="text-[10px] text-text-secondary">Show Zones</span>
            </FieldRow>
            <FieldRow label="Vertical">
              <FieldInput value="1" onCommit={() => {}} />
            </FieldRow>
            <FieldRow label="Horizontal">
              <FieldInput value="1" onCommit={() => {}} />
            </FieldRow>
            <FieldRow label="Origin">
              <select className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
                <option>Upper Left</option><option>Lower Left</option>
              </select>
            </FieldRow>
          </Section>

          {/* Statistics */}
          <Section title="Statistics">
            <StatRow label="Components" value={data?.symbols.filter(s => !s.is_power).length ?? 0} />
            <StatRow label="Wires" value={data?.wires.length ?? 0} />
            <StatRow label="Labels" value={data?.labels.length ?? 0} />
            <StatRow label="Junctions" value={data?.junctions.length ?? 0} />
            <StatRow label="Buses" value={data?.buses.length ?? 0} />
            <StatRow label="No-Connects" value={data?.no_connects.length ?? 0} />
            <StatRow label="Sheets" value={(data?.child_sheets.length ?? 0) + 1} />
          </Section>
        </div>
      ) : (
        <div className="p-3 space-y-2">
          <div className="text-[10px] text-text-muted/50 py-8 text-center">
            Document parameters will appear here.<br/>Use Add to create custom parameters.
          </div>
          <button className="w-full py-1 px-2 rounded bg-bg-surface border border-border-subtle text-[10px] text-text-muted hover:bg-bg-hover hover:text-text-primary transition-colors">
            Add Parameter...
          </button>
        </div>
      )}

      {/* Footer */}
      <div className="px-3 py-1.5 border-t border-border-subtle text-[10px] text-text-muted/40">
        Nothing selected
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// COMPONENT (Altium-style)
// ═══════════════════════════════════════════════════════════════════

function ComponentProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const updateSymbolProp = useSchematicStore((s) => s.updateSymbolProp);
  const units = useEditorStore((s) => s.statusBar.units);
  const sym = data?.symbols.find((s) => s.uuid === uuid);
  const [tab, setTab] = useState<"general" | "pins">("general");
  if (!sym) return null;

  const lib = data?.lib_symbols[sym.lib_id];
  const toggleProp = (setter: (s: NonNullable<typeof sym>) => void) => {
    useSchematicStore.getState().pushUndo();
    const d = useSchematicStore.getState().data;
    if (!d) return;
    const nd = structuredClone(d);
    const found = nd.symbols.find(x => x.uuid === uuid);
    if (found) setter(found);
    useSchematicStore.setState({ data: nd, dirty: true });
  };

  return (
    <div className="text-xs">
      <PanelHeader title="Component" count={1} />

      <div className="flex border-b border-border-subtle">
        <TabBtn active={tab === "general"} onClick={() => setTab("general")}>General</TabBtn>
        <TabBtn active={tab === "pins"} onClick={() => setTab("pins")}>Pins</TabBtn>
      </div>

      {tab === "general" ? (
        <div className="p-3 space-y-3">
          {/* General */}
          <Section title="General">
            <FieldRow label="Designator">
              <FieldInput value={sym.reference} onCommit={(v) => updateSymbolProp(uuid, "reference", v)} />
              <IconBtn icon={sym.ref_text.hidden ? <EyeOff size={11} /> : <Eye size={11} />}
                active={!sym.ref_text.hidden}
                onClick={() => toggleProp(s => { s.ref_text.hidden = !s.ref_text.hidden; })} />
              <Lock size={10} className="text-text-muted/20 shrink-0" />
            </FieldRow>
            <FieldRow label="Comment">
              <FieldInput value={sym.value} onCommit={(v) => updateSymbolProp(uuid, "value", v)} />
              <IconBtn icon={sym.val_text.hidden ? <EyeOff size={11} /> : <Eye size={11} />}
                active={!sym.val_text.hidden}
                onClick={() => toggleProp(s => { s.val_text.hidden = !s.val_text.hidden; })} />
              <Lock size={10} className="text-text-muted/20 shrink-0" />
            </FieldRow>
            <FieldRow label="Description">
              <span className="text-[10px] text-text-secondary truncate flex-1" title={sym.lib_id}>
                {sym.lib_id}
              </span>
            </FieldRow>
            <FieldRow label="Type">
              <select value={sym.is_power ? "Power" : "Standard"} onChange={() => {}}
                className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
                <option>Standard</option><option>Power</option>
              </select>
            </FieldRow>
            <FieldRow label="Source">
              <span className="text-[10px] text-text-muted truncate flex-1">
                {sym.lib_id.split(":")[0] || "—"}
              </span>
            </FieldRow>
          </Section>

          {/* Location */}
          <Section title="Location">
            <FieldRow label="(X/Y)">
              <FieldInput value={mmToDisplay(sym.position.x, units)} suffix={units}
                onCommit={(v) => updateSymbolProp(uuid, "x", String(displayToMm(parseFloat(v) || 0, units)))} />
              <FieldInput value={mmToDisplay(sym.position.y, units)} suffix={units}
                onCommit={(v) => updateSymbolProp(uuid, "y", String(displayToMm(parseFloat(v) || 0, units)))} />
            </FieldRow>
            <FieldRow label="Rotation">
              <select value={sym.rotation} onChange={(e) => updateSymbolProp(uuid, "rotation", e.target.value)}
                className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
                {[0, 90, 180, 270].map(r => <option key={r} value={r}>{r} Degrees</option>)}
              </select>
            </FieldRow>
          </Section>

          {/* Parameters */}
          <Section title="Parameters">
            <div className="flex rounded overflow-hidden border border-border-subtle mb-2">
              {["All", "Footprints", "Models", "Parameters"].map(t => (
                <button key={t} className={cn("flex-1 px-1 py-0.5 text-[9px] transition-colors",
                  t === "All" ? "bg-accent/20 text-accent" : "bg-bg-primary text-text-muted hover:bg-bg-hover")}>
                  {t}
                </button>
              ))}
            </div>
            {/* Parameters table */}
            <div className="border border-border-subtle rounded overflow-hidden">
              <div className="flex bg-bg-surface/50 text-[9px] text-text-muted/60 uppercase tracking-wider">
                <div className="flex-1 px-2 py-0.5">Name</div>
                <div className="flex-1 px-2 py-0.5">Value</div>
              </div>
              <div className="border-t border-border-subtle">
                <ParamRow name="Footprint" value={sym.footprint || "(none)"}
                  onEdit={(v) => updateSymbolProp(uuid, "footprint", v)} />
              </div>
            </div>
          </Section>

          {/* Graphical */}
          <Section title="Graphical">
            <FieldRow label="Mode">
              <select className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
                <option>Normal</option><option>De Morgan</option>
              </select>
            </FieldRow>
            <FieldRow label="">
              <CheckBox checked={sym.mirror_x} onChange={() => {
                toggleProp(s => { s.mirror_x = !s.mirror_x; });
              }} />
              <span className="text-[10px] text-text-secondary">Mirrored</span>
            </FieldRow>
            <FieldRow label="">
              <CheckBox checked={sym.locked} onChange={() => {
                useSchematicStore.getState().toggleDesignatorLock(uuid);
              }} />
              <span className="text-[10px] text-text-secondary">Lock Designator</span>
            </FieldRow>
            <FieldRow label="Local Colors">
              <span className="text-[10px] text-text-muted/50 mr-1">Fills</span>
              <ColorSwatch color="#9fa8da" />
              <span className="text-[10px] text-text-muted/50 mx-1">Lines</span>
              <ColorSwatch color="#9fa8da" />
              <span className="text-[10px] text-text-muted/50 mx-1">Pins</span>
              <ColorSwatch color="#81c784" />
            </FieldRow>
          </Section>
        </div>
      ) : (
        <div className="p-3 space-y-2">
          {/* Pins tab */}
          <div className="border border-border-subtle rounded overflow-hidden">
            <div className="flex bg-bg-surface/50 text-[9px] text-text-muted/60 uppercase tracking-wider">
              <div className="w-10 px-2 py-0.5">#</div>
              <div className="flex-1 px-2 py-0.5">Name</div>
              <div className="w-16 px-2 py-0.5">Type</div>
            </div>
            {lib?.pins.map((pin, i) => (
              <div key={i} className="flex border-t border-border-subtle text-[10px] hover:bg-bg-hover/50">
                <div className="w-10 px-2 py-0.5 font-mono text-text-muted">{pin.number}</div>
                <div className="flex-1 px-2 py-0.5 text-text-primary">{pin.name}</div>
                <div className="w-16 px-2 py-0.5 text-text-muted/60 capitalize">{pin.pin_type}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="px-3 py-1.5 border-t border-border-subtle text-[10px] text-text-muted/40">
        1 object is selected
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// WIRE
// ═══════════════════════════════════════════════════════════════════

function WireProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const units = useEditorStore((s) => s.statusBar.units);
  const wire = data?.wires.find((w) => w.uuid === uuid);
  if (!wire) return null;
  const length = Math.hypot(wire.end.x - wire.start.x, wire.end.y - wire.start.y);

  return (
    <div className="text-xs">
      <PanelHeader title="Wire" count={1} />
      <div className="p-3 space-y-3">
        <Section title="Properties">
          <FieldRow label="Net Name">
            <span className="text-[10px] font-mono text-text-muted/50">(unresolved)</span>
          </FieldRow>
          <FieldRow label="Width">
            <select className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
              <option>Smallest</option><option>Small</option><option>Medium</option><option>Large</option>
            </select>
          </FieldRow>
          <FieldRow label="Color">
            <ColorSwatch color="#4fc3f7" />
          </FieldRow>
        </Section>

        <Section title="Vertices">
          <div className="border border-border-subtle rounded overflow-hidden">
            <div className="flex bg-bg-surface/50 text-[9px] text-text-muted/60 uppercase tracking-wider">
              <div className="w-8 px-1.5 py-0.5">#</div>
              <div className="flex-1 px-1.5 py-0.5">X</div>
              <div className="flex-1 px-1.5 py-0.5">Y</div>
            </div>
            <div className="flex border-t border-border-subtle text-[10px]">
              <div className="w-8 px-1.5 py-0.5 font-mono text-text-muted">1</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(wire.start.x, units)}</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(wire.start.y, units)}</div>
            </div>
            <div className="flex border-t border-border-subtle text-[10px]">
              <div className="w-8 px-1.5 py-0.5 font-mono text-text-muted">2</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(wire.end.x, units)}</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(wire.end.y, units)}</div>
            </div>
          </div>
          <PropRow label="Length" value={mmToDisplay(length, units) + " " + units} />
        </Section>
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// NET LABEL / GLOBAL LABEL / HIERARCHICAL LABEL
// ═══════════════════════════════════════════════════════════════════

function LabelProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const updateLabelProp = useSchematicStore((s) => s.updateLabelProp);
  const units = useEditorStore((s) => s.statusBar.units);
  const label = data?.labels.find((l) => l.uuid === uuid);
  if (!label) return null;

  const isPower = label.label_type === "Power";
  const title = isPower ? "Power Port" : label.label_type === "Global" ? "Global Label" :
    label.label_type === "Hierarchical" ? "Hierarchical Label" : "Net Label";

  return (
    <div className="text-xs">
      <PanelHeader title={title} count={1} />
      <div className="p-3 space-y-3">
        <Section title="Location">
          <FieldRow label="(X/Y)">
            <FieldInput value={mmToDisplay(label.position.x, units)} suffix={units}
              onCommit={(v) => updateLabelProp(uuid, "x", String(displayToMm(parseFloat(v) || 0, units)))} />
            <FieldInput value={mmToDisplay(label.position.y, units)} suffix={units}
              onCommit={(v) => updateLabelProp(uuid, "y", String(displayToMm(parseFloat(v) || 0, units)))} />
          </FieldRow>
          <FieldRow label="Rotation">
            <select value={label.rotation} onChange={() => {}}
              className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
              {[0, 90, 180, 270].map(r => <option key={r} value={r}>{r} Degrees</option>)}
            </select>
          </FieldRow>
        </Section>

        <Section title="Properties">
          <FieldRow label="Net Name">
            <FieldInput value={label.text} onCommit={(v) => updateLabelProp(uuid, "text", v)} />
          </FieldRow>
          {isPower && (
            <FieldRow label="Style">
              <select value={label.shape || "input"} onChange={() => {}}
                className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
                {["Bar", "Arrow", "Power Ground", "Signal Ground", "Earth Ground", "Circle"].map(s =>
                  <option key={s}>{s}</option>
                )}
              </select>
            </FieldRow>
          )}
          {(label.label_type === "Global" || label.label_type === "Hierarchical") && (
            <FieldRow label="I/O Type">
              <select value={label.shape || "bidirectional"} onChange={() => {}}
                className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
                {["Unspecified", "Output", "Input", "Bidirectional"].map(s => <option key={s} value={s.toLowerCase()}>{s}</option>)}
              </select>
            </FieldRow>
          )}
          <FieldRow label="Font">
            <span className="text-accent text-[10px] cursor-pointer hover:underline">Roboto</span>
            <FieldInput value={mmToDisplay(label.font_size, units)} suffix={units} onCommit={() => {}} />
            <ColorSwatch color="#81c784" />
          </FieldRow>
          {/* Text style buttons */}
          <div className="flex gap-px rounded overflow-hidden border border-border-subtle ml-[70px]">
            {["B", "I", "U", "S"].map(s => (
              <button key={s} className="px-2.5 py-0.5 text-[10px] bg-bg-primary text-text-muted hover:bg-bg-hover transition-colors"
                style={{ fontWeight: s === "B" ? 700 : 400, fontStyle: s === "I" ? "italic" : "normal",
                         textDecoration: s === "U" ? "underline" : s === "S" ? "line-through" : "none" }}>
                {s}
              </button>
            ))}
          </div>
          <FieldRow label="Justification">
            <div className="grid grid-cols-3 gap-px rounded overflow-hidden border border-border-subtle">
              {["↖","↑","↗","←","·","→","↙","↓","↘"].map((a, i) => (
                <button key={i} className={cn("w-5 h-5 text-[9px] transition-colors",
                  i === 6 ? "bg-accent/20 text-accent" : "bg-bg-primary text-text-muted hover:bg-bg-hover")}>{a}</button>
              ))}
            </div>
          </FieldRow>
        </Section>
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// JUNCTION
// ═══════════════════════════════════════════════════════════════════

function JunctionProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const units = useEditorStore((s) => s.statusBar.units);
  const junction = data?.junctions.find((j) => j.uuid === uuid);
  if (!junction) return null;

  return (
    <div className="text-xs">
      <PanelHeader title="Junction" count={1} />
      <div className="p-3 space-y-3">
        <Section title="Location">
          <FieldRow label="(X/Y)">
            <span className="font-mono text-[10px]">{mmToDisplay(junction.position.x, units)}</span>
            <span className="font-mono text-[10px]">{mmToDisplay(junction.position.y, units)}</span>
            <span className="text-text-muted/40 text-[10px]">{units}</span>
          </FieldRow>
        </Section>
        <Section title="Properties">
          <FieldRow label="Size">
            <select className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
              <option>Smallest</option><option>Small</option><option>Medium</option><option>Large</option>
            </select>
          </FieldRow>
          <FieldRow label="Color">
            <ColorSwatch color="#4fc3f7" />
          </FieldRow>
        </Section>
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// NO CONNECT
// ═══════════════════════════════════════════════════════════════════

function NoConnectProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const units = useEditorStore((s) => s.statusBar.units);
  const nc = data?.no_connects.find((n) => n.uuid === uuid);
  if (!nc) return null;

  return (
    <div className="text-xs">
      <PanelHeader title="No Connect" count={1} />
      <div className="p-3 space-y-3">
        <Section title="Location">
          <FieldRow label="(X/Y)">
            <span className="font-mono text-[10px]">{mmToDisplay(nc.position.x, units)}</span>
            <span className="font-mono text-[10px]">{mmToDisplay(nc.position.y, units)}</span>
            <span className="text-text-muted/40 text-[10px]">{units}</span>
          </FieldRow>
        </Section>
        <Section title="Properties">
          <FieldRow label="Color">
            <ColorSwatch color="#e8667a" />
          </FieldRow>
        </Section>
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// TEXT NOTE (Altium Note style)
// ═══════════════════════════════════════════════════════════════════

function TextNoteProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const updateTextNoteProp = useSchematicStore((s) => s.updateTextNoteProp);
  const units = useEditorStore((s) => s.statusBar.units);
  const note = data?.text_notes.find((t) => t.uuid === uuid);
  const [editingText, setEditingText] = useState(false);
  const [textDraft, setTextDraft] = useState("");
  if (!note) return null;

  return (
    <div className="text-xs">
      <PanelHeader title="Note" count={1} />
      <div className="p-3 space-y-3">
        <Section title="Location">
          <FieldRow label="(X/Y)">
            <FieldInput value={mmToDisplay(note.position.x, units)} suffix={units}
              onCommit={(v) => updateTextNoteProp(uuid, "x", String(displayToMm(parseFloat(v) || 0, units)))} />
            <FieldInput value={mmToDisplay(note.position.y, units)} suffix={units}
              onCommit={(v) => updateTextNoteProp(uuid, "y", String(displayToMm(parseFloat(v) || 0, units)))} />
          </FieldRow>
        </Section>

        <Section title="Properties">
          <div className="space-y-1">
            <span className="text-text-muted/70 text-[11px]">Text</span>
            {editingText ? (
              <textarea autoFocus value={textDraft}
                onChange={(e) => setTextDraft(e.target.value)}
                onBlur={() => { if (textDraft !== note.text) updateTextNoteProp(uuid, "text", textDraft); setEditingText(false); }}
                onKeyDown={(e) => { if (e.key === "Escape") setEditingText(false); e.stopPropagation(); }}
                rows={Math.min(8, Math.max(3, note.text.split("\n").length + 1))}
                className="w-full bg-bg-surface border border-accent/40 rounded px-2 py-1 text-[10px] font-mono text-text-primary outline-none focus:border-accent resize-y"
              />
            ) : (
              <div onClick={() => { setTextDraft(note.text); setEditingText(true); }}
                className="w-full bg-bg-surface border border-border-subtle rounded px-2 py-1.5 text-[10px] font-mono text-text-primary cursor-pointer hover:border-accent/40 transition-colors whitespace-pre-wrap max-h-[120px] overflow-y-auto min-h-[48px]">
                {note.text || "(empty)"}
              </div>
            )}
          </div>
          <FieldRow label="">
            <CheckBox checked={true} onChange={() => {}} />
            <span className="text-[10px] text-text-secondary">Word Wrap</span>
          </FieldRow>
          <FieldRow label="">
            <CheckBox checked={false} onChange={() => {}} />
            <span className="text-[10px] text-text-secondary">Clip to Area</span>
          </FieldRow>
          <FieldRow label="Font">
            <span className="text-accent text-[10px] cursor-pointer hover:underline">Roboto</span>
            <FieldInput value={mmToDisplay(note.font_size, units)} suffix={units} onCommit={() => {}} />
            <ColorSwatch color="#cdd6f4" />
          </FieldRow>
          <div className="flex gap-px rounded overflow-hidden border border-border-subtle ml-[70px]">
            {["B", "I", "U", "S"].map(s => (
              <button key={s} className="px-2.5 py-0.5 text-[10px] bg-bg-primary text-text-muted hover:bg-bg-hover transition-colors"
                style={{ fontWeight: s === "B" ? 700 : 400, fontStyle: s === "I" ? "italic" : "normal",
                         textDecoration: s === "U" ? "underline" : s === "S" ? "line-through" : "none" }}>
                {s}
              </button>
            ))}
          </div>
          <FieldRow label="Alignment">
            <div className="flex gap-px rounded overflow-hidden border border-border-subtle">
              {["Left", "Center", "Right"].map(a => (
                <button key={a} className={cn("px-2 py-0.5 text-[9px] transition-colors",
                  a === "Left" ? "bg-accent/20 text-accent" : "bg-bg-primary text-text-muted hover:bg-bg-hover")}>{a}</button>
              ))}
            </div>
          </FieldRow>
          <FieldRow label="Rotation">
            <select value={note.rotation} onChange={() => {}}
              className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
              {[0, 90, 180, 270].map(r => <option key={r} value={r}>{r}°</option>)}
            </select>
          </FieldRow>
          <FieldRow label="Border">
            <CheckBox checked={true} onChange={() => {}} />
            <select className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
              <option>Smallest</option><option>Small</option><option>Medium</option><option>Large</option>
            </select>
            <ColorSwatch color="#2a2d4a" />
          </FieldRow>
          <FieldRow label="Fill Color">
            <ColorSwatch color="#1e2035" />
          </FieldRow>
        </Section>
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// BUS
// ═══════════════════════════════════════════════════════════════════

function BusProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const units = useEditorStore((s) => s.statusBar.units);
  const bus = data?.buses.find((b) => b.uuid === uuid);
  if (!bus) return null;
  const length = Math.hypot(bus.end.x - bus.start.x, bus.end.y - bus.start.y);

  return (
    <div className="text-xs">
      <PanelHeader title="Bus" count={1} />
      <div className="p-3 space-y-3">
        <Section title="Properties">
          <FieldRow label="Width">
            <select className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
              <option>Smallest</option><option>Small</option><option>Medium</option><option>Large</option>
            </select>
          </FieldRow>
          <FieldRow label="Color">
            <ColorSwatch color="#4a86c8" />
          </FieldRow>
        </Section>
        <Section title="Vertices">
          <div className="border border-border-subtle rounded overflow-hidden">
            <div className="flex bg-bg-surface/50 text-[9px] text-text-muted/60 uppercase tracking-wider">
              <div className="w-8 px-1.5 py-0.5">#</div>
              <div className="flex-1 px-1.5 py-0.5">X</div>
              <div className="flex-1 px-1.5 py-0.5">Y</div>
            </div>
            <div className="flex border-t border-border-subtle text-[10px]">
              <div className="w-8 px-1.5 py-0.5 font-mono text-text-muted">1</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(bus.start.x, units)}</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(bus.start.y, units)}</div>
            </div>
            <div className="flex border-t border-border-subtle text-[10px]">
              <div className="w-8 px-1.5 py-0.5 font-mono text-text-muted">2</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(bus.end.x, units)}</div>
              <div className="flex-1 px-1.5 py-0.5 font-mono">{mmToDisplay(bus.end.y, units)}</div>
            </div>
          </div>
          <PropRow label="Length" value={mmToDisplay(length, units) + " " + units} />
        </Section>
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// BUS ENTRY
// ═══════════════════════════════════════════════════════════════════

function BusEntryProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const units = useEditorStore((s) => s.statusBar.units);
  const be = data?.bus_entries.find((b) => b.uuid === uuid);
  if (!be) return null;

  return (
    <div className="text-xs">
      <PanelHeader title="Bus Entry" count={1} />
      <div className="p-3 space-y-3">
        <Section title="Location">
          <FieldRow label="Start">
            <span className="font-mono text-[10px]">{mmToDisplay(be.position.x, units)}</span>
            <span className="font-mono text-[10px]">{mmToDisplay(be.position.y, units)}</span>
            <span className="text-text-muted/40 text-[10px]">{units}</span>
          </FieldRow>
          <FieldRow label="End">
            <span className="font-mono text-[10px]">{mmToDisplay(be.position.x + be.size[0], units)}</span>
            <span className="font-mono text-[10px]">{mmToDisplay(be.position.y + be.size[1], units)}</span>
            <span className="text-text-muted/40 text-[10px]">{units}</span>
          </FieldRow>
        </Section>
        <Section title="Properties">
          <FieldRow label="Width">
            <select className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none">
              <option>Smallest</option><option>Small</option><option>Medium</option><option>Large</option>
            </select>
          </FieldRow>
          <FieldRow label="Color">
            <ColorSwatch color="#4a86c8" />
          </FieldRow>
        </Section>
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// SHEET SYMBOL
// ═══════════════════════════════════════════════════════════════════

function SheetSymbolProps({ uuid }: { uuid: string }) {
  const data = useSchematicStore((s) => s.data);
  const units = useEditorStore((s) => s.statusBar.units);
  const sheet = data?.child_sheets.find((cs) => cs.uuid === uuid);
  if (!sheet) return null;

  return (
    <div className="text-xs">
      <PanelHeader title="Sheet Symbol" count={1} />
      <div className="p-3 space-y-3">
        <Section title="General">
          <FieldRow label="Designator">
            <span className="text-[10px] font-mono text-text-primary flex-1">{sheet.name}</span>
          </FieldRow>
          <FieldRow label="File Name">
            <span className="text-[10px] font-mono text-text-primary flex-1 truncate" title={sheet.filename}>
              {sheet.filename}
            </span>
          </FieldRow>
        </Section>

        <Section title="Location">
          <FieldRow label="(X/Y)">
            <span className="font-mono text-[10px]">{mmToDisplay(sheet.position.x, units)}</span>
            <span className="font-mono text-[10px]">{mmToDisplay(sheet.position.y, units)}</span>
            <span className="text-text-muted/40 text-[10px]">{units}</span>
          </FieldRow>
          <FieldRow label="Size">
            <span className="font-mono text-[10px]">{mmToDisplay(sheet.size[0], units)} x {mmToDisplay(sheet.size[1], units)} {units}</span>
          </FieldRow>
        </Section>

        {sheet.pins.length > 0 && (
          <Section title="Sheet Entries">
            <div className="border border-border-subtle rounded overflow-hidden">
              <div className="flex bg-bg-surface/50 text-[9px] text-text-muted/60 uppercase tracking-wider">
                <div className="flex-1 px-2 py-0.5">Name</div>
                <div className="w-20 px-2 py-0.5">I/O Type</div>
              </div>
              {sheet.pins.map((pin, i) => (
                <div key={i} className="flex border-t border-border-subtle text-[10px] hover:bg-bg-hover/50">
                  <div className="flex-1 px-2 py-0.5 text-text-primary">{pin.name}</div>
                  <div className="w-20 px-2 py-0.5 text-text-muted/60 capitalize">{pin.direction}</div>
                </div>
              ))}
            </div>
          </Section>
        )}
      </div>
      <PanelFooter />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// SHARED UI PRIMITIVES
// ═══════════════════════════════════════════════════════════════════

function PanelHeader({ title, count }: { title: string; count?: number }) {
  return (
    <div className="px-3 py-2 border-b border-border-subtle flex items-center justify-between">
      <span className="text-[11px] font-semibold text-text-secondary">{title}</span>
      {count !== undefined && <span className="text-[10px] text-text-muted/50">{count} object{count !== 1 ? "s" : ""}</span>}
    </div>
  );
}

function PanelFooter() {
  return (
    <div className="px-3 py-1.5 border-t border-border-subtle text-[10px] text-text-muted/40">
      1 object is selected
    </div>
  );
}

function TabBtn({ active, children, onClick }: { active: boolean; children: React.ReactNode; onClick: () => void }) {
  return (
    <button onClick={onClick}
      className={cn("px-3 py-1.5 text-[10px] transition-colors border-b-2",
        active ? "border-accent text-accent font-semibold" : "border-transparent text-text-muted hover:text-text-secondary")}>
      {children}
    </button>
  );
}

function Section({ title, children, defaultOpen = true }: { title: string; children: React.ReactNode; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div>
      <button onClick={() => setOpen(!open)}
        className="flex items-center gap-1 text-[10px] font-semibold text-text-muted/70 uppercase tracking-wider w-full hover:text-text-secondary transition-colors mb-1.5">
        {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        {title}
      </button>
      {open && <div className="space-y-1.5 pl-0.5">{children}</div>}
    </div>
  );
}

/** Altium-style field row: label on left, controls on right */
function FieldRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-1.5 min-h-[22px]">
      {label && <span className="text-text-muted/70 shrink-0 text-[11px] w-[70px] text-right">{label}</span>}
      {!label && <span className="w-[70px] shrink-0" />}
      {children}
    </div>
  );
}

function FieldInput({ value, suffix, onCommit }: { value: string; suffix?: string; onCommit: (v: string) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  if (editing) {
    return (
      <input autoFocus value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => { setEditing(false); if (draft !== value) onCommit(draft); }}
        onKeyDown={(e) => { if (e.key === "Enter") { setEditing(false); if (draft !== value) onCommit(draft); } if (e.key === "Escape") setEditing(false); e.stopPropagation(); }}
        className="flex-1 min-w-0 bg-bg-surface border border-accent/50 rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none focus:border-accent"
      />
    );
  }

  return (
    <div onClick={() => { setDraft(value); setEditing(true); }}
      className="flex-1 min-w-0 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary cursor-pointer hover:border-accent/40 transition-colors truncate">
      {value}{suffix ? <span className="text-text-muted/40 ml-0.5">{suffix}</span> : null}
    </div>
  );
}

function PropRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-1.5 min-h-[22px]">
      <span className="text-text-muted/70 shrink-0 text-[11px] w-[70px] text-right">{label}</span>
      <span className="text-text-primary truncate text-right font-mono text-[10px] flex-1">{value}</span>
    </div>
  );
}

function StatRow({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-text-muted/70 text-[11px]">{label}</span>
      <span className="text-text-primary font-mono text-[10px] tabular-nums">{value}</span>
    </div>
  );
}

function CheckBox({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <button onClick={onChange}
      className={cn("w-3.5 h-3.5 rounded-sm border shrink-0 flex items-center justify-center transition-colors",
        checked ? "bg-accent/30 border-accent" : "bg-bg-primary border-border-subtle")}>
      {checked && <span className="text-accent text-[8px] leading-none">✓</span>}
    </button>
  );
}

function ColorSwatch({ color }: { color: string }) {
  return (
    <div className="w-4 h-4 rounded-sm border border-border-subtle shrink-0 cursor-pointer hover:ring-1 hover:ring-accent/30 transition-all"
      style={{ backgroundColor: color }} title={color} />
  );
}

function IconBtn({ icon, active, onClick }: { icon: React.ReactNode; active?: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick}
      className={cn("p-0.5 rounded shrink-0 transition-colors",
        active ? "text-accent" : "text-text-muted/30 hover:text-text-muted")}>
      {icon}
    </button>
  );
}

function FilterBtn({ label }: { label: string }) {
  const [on, setOn] = useState(true);
  return (
    <button onClick={() => setOn(!on)}
      className={cn("px-1.5 py-0.5 rounded text-[9px] border transition-colors",
        on ? "bg-accent/20 text-accent border-accent/30" : "bg-bg-primary text-text-muted/40 border-border-subtle")}>
      {label}
    </button>
  );
}

function ParamRow({ name, value, onEdit }: { name: string; value: string; onEdit?: (v: string) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  return (
    <div className="flex text-[10px] hover:bg-bg-hover/50">
      <div className="flex-1 px-2 py-0.5 text-text-muted">{name}</div>
      <div className="flex-1 px-2 py-0.5">
        {editing ? (
          <input autoFocus value={draft} onChange={(e) => setDraft(e.target.value)}
            onBlur={() => { setEditing(false); if (onEdit && draft !== value) onEdit(draft); }}
            onKeyDown={(e) => { if (e.key === "Enter") { setEditing(false); if (onEdit && draft !== value) onEdit(draft); } e.stopPropagation(); }}
            className="w-full bg-bg-surface border border-accent/40 rounded px-1 text-[10px] font-mono text-text-primary outline-none" />
        ) : (
          <span onClick={() => { setDraft(value); setEditing(true); }}
            className="cursor-pointer hover:text-accent transition-colors font-mono text-text-primary">{value}</span>
        )}
      </div>
    </div>
  );
}
