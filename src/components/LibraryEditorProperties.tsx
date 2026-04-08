import { useCallback } from "react";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import type { LibPanelView } from "@/stores/libraryEditor";
import type { SchPin } from "@/types";
import { cn } from "@/lib/utils";

const PIN_TYPES = [
  "passive", "input", "output", "bidirectional", "power_in", "power_out",
  "open_collector", "open_emitter", "tri_state", "unspecified", "no_connect",
];

const PIN_SHAPES = [
  "line", "inverted", "clock", "inverted_clock", "input_low", "clock_low",
  "output_low", "edge_clock_high", "non_logic",
];

// ---------------------------------------------------------------------------
// Tab bar
// ---------------------------------------------------------------------------

function TabBar({ view, setView }: { view: LibPanelView; setView: (v: LibPanelView) => void }) {
  return (
    <div className="flex border-b border-border-subtle bg-bg-secondary/80 shrink-0">
      {([["properties", "Properties"], ["pinTable", "Pin Table"]] as const).map(([id, label]) => (
        <button key={id} onClick={() => setView(id)}
          className={cn("px-3 py-1.5 text-[10px] font-medium transition-colors",
            view === id ? "text-accent border-b-2 border-accent" : "text-text-muted/60 hover:text-text-primary"
          )}>
          {label}
        </button>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Properties tab: symbol metadata + selected item properties
// ---------------------------------------------------------------------------

function PropertiesTab() {
  const symbol = useLibraryEditorStore(s => s.symbol);
  const selectedItem = useLibraryEditorStore(s => s.selectedItem);
  const updateSymbolMeta = useLibraryEditorStore(s => s.updateSymbolMeta);
  const updateSymbolId = useLibraryEditorStore(s => s.updateSymbolId);
  const updatePin = useLibraryEditorStore(s => s.updatePin);
  const updateGraphic = useLibraryEditorStore(s => s.updateGraphic);

  if (!symbol) return <div className="p-3 text-[10px] text-text-muted/50">No symbol loaded</div>;

  return (
    <div className="overflow-y-auto text-[10px]">
      {/* Symbol metadata */}
      <Section title="Symbol">
        <Field label="ID">
          <input value={symbol.id} onChange={e => updateSymbolId(e.target.value)}
            className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none" />
        </Field>
        <Field label="Pins">{symbol.pins.length}</Field>
        <Field label="Graphics">{symbol.graphics.length}</Field>
        <Field label="Show Pin Numbers">
          <input type="checkbox" checked={symbol.show_pin_numbers}
            onChange={e => updateSymbolMeta({ show_pin_numbers: e.target.checked })} />
        </Field>
        <Field label="Show Pin Names">
          <input type="checkbox" checked={symbol.show_pin_names}
            onChange={e => updateSymbolMeta({ show_pin_names: e.target.checked })} />
        </Field>
        <Field label="Name Offset">
          <input type="number" step={0.1} value={symbol.pin_name_offset}
            onChange={e => updateSymbolMeta({ pin_name_offset: parseFloat(e.target.value) || 0 })}
            className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
        </Field>
      </Section>

      {/* Selected pin properties */}
      {selectedItem?.type === "pin" && selectedItem.index < symbol.pins.length && (
        <SelectedPinProperties pin={symbol.pins[selectedItem.index]} index={selectedItem.index} updatePin={updatePin} />
      )}

      {/* Selected graphic properties */}
      {selectedItem?.type === "graphic" && selectedItem.index < symbol.graphics.length && (
        <SelectedGraphicProperties graphic={symbol.graphics[selectedItem.index]} index={selectedItem.index} updateGraphic={updateGraphic} />
      )}
    </div>
  );
}

function SelectedPinProperties({ pin, index, updatePin }: { pin: SchPin; index: number; updatePin: (i: number, u: Partial<SchPin>) => void }) {
  const u = useCallback((updates: Partial<SchPin>) => updatePin(index, updates), [index, updatePin]);

  return (
    <Section title={`Pin ${pin.number}: ${pin.name}`}>
      <Field label="Name">
        <input value={pin.name} onChange={e => u({ name: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none" />
      </Field>
      <Field label="Number">
        <input value={pin.number} onChange={e => u({ number: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
      </Field>
      <Field label="Electrical Type">
        <select value={pin.pin_type} onChange={e => u({ pin_type: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none">
          {PIN_TYPES.map(t => <option key={t} value={t}>{t.replace(/_/g, " ")}</option>)}
        </select>
      </Field>
      <Field label="Shape">
        <select value={pin.shape} onChange={e => u({ shape: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none">
          {PIN_SHAPES.map(s => <option key={s} value={s}>{s.replace(/_/g, " ")}</option>)}
        </select>
      </Field>
      <Field label="Length">
        <input type="number" step={0.01} value={pin.length}
          onChange={e => u({ length: parseFloat(e.target.value) || 2.54 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
      </Field>
      <Field label="Rotation">
        <select value={pin.rotation} onChange={e => u({ rotation: parseInt(e.target.value) })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16">
          {[0, 90, 180, 270].map(r => <option key={r} value={r}>{r}&deg;</option>)}
        </select>
      </Field>
      <Field label="Position X">
        <input type="number" step={1.27} value={pin.position.x.toFixed(2)}
          onChange={e => u({ position: { ...pin.position, x: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
      </Field>
      <Field label="Position Y">
        <input type="number" step={1.27} value={pin.position.y.toFixed(2)}
          onChange={e => u({ position: { ...pin.position, y: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
      </Field>
      <Field label="Name Visible">
        <input type="checkbox" checked={pin.name_visible} onChange={e => u({ name_visible: e.target.checked })} />
      </Field>
      <Field label="Number Visible">
        <input type="checkbox" checked={pin.number_visible} onChange={e => u({ number_visible: e.target.checked })} />
      </Field>
      <Field label="Hidden">
        <input type="checkbox" checked={pin.hidden ?? false} onChange={e => u({ hidden: e.target.checked })} />
      </Field>
    </Section>
  );
}

function SelectedGraphicProperties({ graphic, index, updateGraphic }: { graphic: import("@/types").Graphic; index: number; updateGraphic: (i: number, g: import("@/types").Graphic) => void }) {
  const g = graphic;
  const update = useCallback((partial: Record<string, any>) => {
    updateGraphic(index, { ...g, ...partial } as any);
  }, [index, g, updateGraphic]);

  return (
    <Section title={`Graphic: ${g.type}`}>
      {"width" in g && (
        <Field label="Line Width">
          <input type="number" step={0.01} value={(g as any).width}
            onChange={e => update({ width: parseFloat(e.target.value) || 0.254 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
        </Field>
      )}
      {"fill_type" in g && (
        <Field label="Fill">
          <select value={(g as any).fill_type} onChange={e => update({ fill_type: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none">
            <option value="none">None</option>
            <option value="outline">Outline</option>
            <option value="background">Background</option>
          </select>
        </Field>
      )}
      {g.type === "Rectangle" && (
        <>
          <Field label="Start X">
            <input type="number" step={1.27} value={g.start.x.toFixed(2)} onChange={e => update({ start: { ...g.start, x: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
          </Field>
          <Field label="Start Y">
            <input type="number" step={1.27} value={g.start.y.toFixed(2)} onChange={e => update({ start: { ...g.start, y: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
          </Field>
          <Field label="End X">
            <input type="number" step={1.27} value={g.end.x.toFixed(2)} onChange={e => update({ end: { ...g.end, x: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
          </Field>
          <Field label="End Y">
            <input type="number" step={1.27} value={g.end.y.toFixed(2)} onChange={e => update({ end: { ...g.end, y: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
          </Field>
        </>
      )}
      {g.type === "Circle" && (
        <>
          <Field label="Center X">
            <input type="number" step={1.27} value={g.center.x.toFixed(2)} onChange={e => update({ center: { ...g.center, x: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
          </Field>
          <Field label="Center Y">
            <input type="number" step={1.27} value={g.center.y.toFixed(2)} onChange={e => update({ center: { ...g.center, y: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
          </Field>
          <Field label="Radius">
            <input type="number" step={0.1} value={g.radius.toFixed(2)} onChange={e => update({ radius: parseFloat(e.target.value) || 1 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
          </Field>
        </>
      )}
      {g.type === "Text" && (
        <>
          <Field label="Text">
            <input value={g.text} onChange={e => update({ text: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none" />
          </Field>
          <Field label="Font Size">
            <input type="number" step={0.1} value={g.font_size} onChange={e => update({ font_size: parseFloat(e.target.value) || 1.27 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
          </Field>
        </>
      )}
    </Section>
  );
}

// ---------------------------------------------------------------------------
// Pin Table tab: spreadsheet view of all pins
// ---------------------------------------------------------------------------

function PinTableTab() {
  const symbol = useLibraryEditorStore(s => s.symbol);
  const updatePin = useLibraryEditorStore(s => s.updatePin);
  const setSelectedItem = useLibraryEditorStore(s => s.setSelectedItem);
  const selectedItem = useLibraryEditorStore(s => s.selectedItem);

  if (!symbol) return <div className="p-3 text-[10px] text-text-muted/50">No symbol loaded</div>;

  return (
    <div className="overflow-auto text-[10px]">
      <table className="w-full border-collapse">
        <thead className="sticky top-0 z-10 bg-bg-surface">
          <tr className="border-b border-border-subtle text-left">
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium w-[32px]">#</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium">Name</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium w-[36px]">Num</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium">Type</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium">Shape</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium w-[42px]">Len</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium w-[36px]">Rot</th>
          </tr>
        </thead>
        <tbody>
          {symbol.pins.map((pin, i) => {
            const isSelected = selectedItem?.type === "pin" && selectedItem.index === i;
            return (
              <tr key={i}
                onClick={() => setSelectedItem({ type: "pin", index: i })}
                className={cn("border-b border-border-subtle/20 cursor-pointer transition-colors",
                  isSelected ? "bg-accent/15" : "hover:bg-bg-hover/50"
                )}>
                <td className="px-1.5 py-0.5 text-text-muted/50 tabular-nums">{i + 1}</td>
                <td className="px-1.5 py-0.5">
                  <input value={pin.name} onChange={e => updatePin(i, { name: e.target.value })}
                    className="bg-transparent w-full outline-none text-text-primary font-mono" />
                </td>
                <td className="px-1.5 py-0.5">
                  <input value={pin.number} onChange={e => updatePin(i, { number: e.target.value })}
                    className="bg-transparent w-full outline-none text-text-primary font-mono" />
                </td>
                <td className="px-1.5 py-0.5">
                  <select value={pin.pin_type} onChange={e => updatePin(i, { pin_type: e.target.value })}
                    className="bg-transparent outline-none text-text-secondary text-[10px] w-full">
                    {PIN_TYPES.map(t => <option key={t} value={t}>{t.replace(/_/g, " ")}</option>)}
                  </select>
                </td>
                <td className="px-1.5 py-0.5">
                  <select value={pin.shape} onChange={e => updatePin(i, { shape: e.target.value })}
                    className="bg-transparent outline-none text-text-secondary text-[10px] w-full">
                    {PIN_SHAPES.map(s => <option key={s} value={s}>{s.replace(/_/g, " ")}</option>)}
                  </select>
                </td>
                <td className="px-1.5 py-0.5 font-mono">{pin.length.toFixed(1)}</td>
                <td className="px-1.5 py-0.5 font-mono">{pin.rotation}&deg;</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {symbol.pins.length === 0 && (
        <div className="p-4 text-center text-text-muted/40">No pins. Use the Pin tool to add pins.</div>
      )}
      <div className="px-2 py-1 border-t border-border-subtle text-text-muted/50">
        {symbol.pins.length} pin{symbol.pins.length !== 1 ? "s" : ""}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shared UI components
// ---------------------------------------------------------------------------

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="border-b border-border-subtle">
      <div className="px-3 py-1.5 bg-bg-secondary/60 text-[10px] font-semibold text-text-secondary">{title}</div>
      <div className="px-3 py-1 space-y-0.5">{children}</div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 py-0.5">
      <span className="w-24 shrink-0 text-text-muted/60 text-[10px]">{label}</span>
      <div className="flex-1 text-[10px] text-text-primary">{children}</div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main export
// ---------------------------------------------------------------------------

export function LibraryEditorProperties() {
  const panelView = useLibraryEditorStore(s => s.panelView);
  const setPanelView = useLibraryEditorStore(s => s.setPanelView);

  return (
    <div className="flex flex-col h-full">
      <TabBar view={panelView} setView={setPanelView} />
      <div className="flex-1 overflow-hidden">
        {panelView === "properties" ? <PropertiesTab /> : <PinTableTab />}
      </div>
    </div>
  );
}
