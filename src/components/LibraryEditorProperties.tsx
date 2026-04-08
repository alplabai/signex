import { useState, useCallback } from "react";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import type { SchPin } from "@/types";
import { cn } from "@/lib/utils";
import { Eye, Lock, ChevronDown } from "lucide-react";
import { FootprintPickerDialog } from "@/components/FootprintPickerDialog";

const PIN_TYPES = [
  "passive", "input", "output", "bidirectional", "power_in", "power_out",
  "open_collector", "open_emitter", "tri_state", "unspecified", "no_connect",
];

const PIN_SHAPES = [
  "line", "inverted", "clock", "inverted_clock", "input_low", "clock_low",
  "output_low", "edge_clock_high", "non_logic",
];

const inp = "bg-bg-secondary border border-border-subtle rounded px-1.5 py-0.5 text-[10px] text-text-primary outline-none w-full";

// ---------------------------------------------------------------------------
// Main export — Altium-style Properties for symbol editor
// ---------------------------------------------------------------------------

export function LibraryEditorProperties() {
  const symbol = useLibraryEditorStore(s => s.symbol);
  const [tab, setTab] = useState<"general" | "pins">("general");
  const [paramTab, setParamTab] = useState("All");

  if (!symbol) return <div className="p-4 text-xs text-text-muted/50">No symbol loaded</div>;

  const pinCount = symbol.pins.length;

  return (
    <div className="flex flex-col h-full overflow-hidden text-[10px]">
      {/* Header — Altium: "Component" + "Pins (and N more)" */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-accent/10 border-b border-border-subtle shrink-0">
        <span className="font-semibold text-text-secondary text-[11px]">Component</span>
        <span className="text-text-muted/60">Pins (and {pinCount} more)</span>
      </div>

      {/* General | Pins tabs */}
      <div className="flex border-b border-border-subtle bg-bg-secondary/60 shrink-0">
        {(["general", "pins"] as const).map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={cn("px-3 py-1.5 text-[10px] font-medium capitalize",
              tab === t ? "text-accent border-b-2 border-accent" : "text-text-muted/50 hover:text-text-primary"
            )}>
            {t}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto">
        {tab === "general" ? (
          <GeneralTab symbol={symbol} paramTab={paramTab} setParamTab={setParamTab} />
        ) : (
          <PinsTab />
        )}
      </div>

      {/* Status bar */}
      <div className="px-3 py-1 border-t border-border-subtle text-[10px] text-accent/70 shrink-0">
        1 object is selected
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// General tab
// ---------------------------------------------------------------------------

function GeneralTab({ symbol, paramTab, setParamTab }: {
  symbol: import("@/types").LibSymbol;
  paramTab: string;
  setParamTab: (t: string) => void;
}) {
  const updateSymbolId = useLibraryEditorStore(s => s.updateSymbolId);
  const updateSymbolMeta = useLibraryEditorStore(s => s.updateSymbolMeta);
  const designatorPrefix = useLibraryEditorStore(s => s.designatorPrefix);
  const setDesignatorPrefix = useLibraryEditorStore(s => s.setDesignatorPrefix);
  const comment = useLibraryEditorStore(s => s.comment);
  const setComment = useLibraryEditorStore(s => s.setComment);
  const description = useLibraryEditorStore(s => s.description);
  const setDescription = useLibraryEditorStore(s => s.setDescription);
  const footprint = useLibraryEditorStore(s => s.footprint);
  const setFootprint = useLibraryEditorStore(s => s.setFootprint);
  const componentType = useLibraryEditorStore(s => s.componentType);
  const setComponentType = useLibraryEditorStore(s => s.setComponentType);
  const mirrored = useLibraryEditorStore(s => s.mirrored);
  const setMirrored = useLibraryEditorStore(s => s.setMirrored);
  const [fpPickerOpen, setFpPickerOpen] = useState(false);

  return (
    <>
    <FootprintPickerDialog
      open={fpPickerOpen}
      initialValue={footprint}
      onClose={() => setFpPickerOpen(false)}
      onSelect={(id) => setFootprint(id)}
    />
    <>
      {/* General section */}
      <SectionHeader title="General" />
      <div className="px-3 py-1.5 space-y-1.5">
        <Row label="Design Item ID">
          <input value={symbol.id} onChange={e => updateSymbolId(e.target.value)} className={inp} />
        </Row>
        <Row label="Designator">
          <div className="flex items-center gap-1">
            <input value={designatorPrefix} onChange={e => setDesignatorPrefix(e.target.value)}
              placeholder="U?" className={cn(inp, "flex-1")} />
            <button className="text-text-muted/40 hover:text-accent" title="Toggle visibility"><Eye size={12} /></button>
            <button className="text-text-muted/40 hover:text-accent" title="Lock"><Lock size={12} /></button>
          </div>
        </Row>
        <Row label="Comment">
          <div className="flex items-center gap-1">
            <input value={comment} onChange={e => setComment(e.target.value)} className={cn(inp, "flex-1")} />
            <button className="text-text-muted/40 hover:text-accent" title="Toggle visibility"><Eye size={12} /></button>
            <button className="text-text-muted/40 hover:text-accent" title="Lock"><Lock size={12} /></button>
          </div>
        </Row>
        <Row label="Part">
          <div className="flex items-center gap-1">
            <select className={cn(inp, "flex-1")} defaultValue="A">
              {Array.from({ length: symbol.unit_count ?? 1 }, (_, i) => (
                <option key={i} value={String.fromCharCode(65 + i)}>Part {String.fromCharCode(65 + i)}</option>
              ))}
            </select>
            <span className="text-text-muted/50 whitespace-nowrap">of Parts</span>
            <input value={symbol.unit_count ?? 1} className={cn(inp, "w-8 text-center")} readOnly />
          </div>
        </Row>
        <Row label="Description">
          <textarea rows={3} value={description} onChange={e => setDescription(e.target.value)}
            className={cn(inp, "resize-none")} />
        </Row>
        <Row label="Type">
          <select value={componentType} onChange={e => setComponentType(e.target.value as any)} className={inp}>
            <option value="standard_no_bom">Standard (No BOM)</option>
            <option value="standard">Standard</option>
            <option value="mechanical">Mechanical</option>
            <option value="graphical">Graphical</option>
          </select>
        </Row>
      </div>

      {/* Parameters section */}
      <SectionHeader title="Parameters" />
      <div className="px-1.5 py-1">
        <div className="flex flex-wrap gap-0.5 mb-1.5">
          {["All", "Footprints", "Models", "Parameters", "Links", "Rules"].map(t => (
            <button key={t} onClick={() => setParamTab(t)}
              className={cn("px-2 py-0.5 rounded text-[9px] font-medium border",
                paramTab === t
                  ? "bg-accent/20 border-accent text-accent"
                  : "border-border-subtle text-text-muted/50 hover:text-text-primary hover:border-text-muted/30"
              )}>
              {t}
            </button>
          ))}
        </div>

        <div className="border border-border-subtle rounded overflow-hidden">
          <div className="flex items-center px-2 py-0.5 bg-bg-secondary/60 border-b border-border-subtle text-[9px] font-semibold text-text-muted/50">
            <span className="flex-1">Name</span>
            <span className="w-[140px] text-right">Value</span>
          </div>
          {(paramTab === "All" || paramTab === "Footprints") && (
            <div className="flex items-center px-2 py-1 hover:bg-bg-hover/30 border-b border-border-subtle/20 gap-1">
              <span className="flex-1 text-text-secondary">Footprint</span>
              <span className="font-mono text-text-muted/60 truncate max-w-[100px]">{footprint || "(none)"}</span>
              <button onClick={() => setFpPickerOpen(true)}
                className="text-[9px] text-accent hover:underline shrink-0">Show</button>
            </div>
          )}
          {(paramTab === "All" || paramTab === "Models") && (
            <div className="px-2 py-1.5 text-center text-text-muted/30">No Models</div>
          )}
          {(paramTab === "All" || paramTab === "Parameters") && (
            <div className="px-2 py-1.5 text-center text-text-muted/30">No Parameters</div>
          )}
          {(paramTab === "All" || paramTab === "Links") && (
            <div className="px-2 py-1.5 text-center text-text-muted/30">No Links</div>
          )}
          {(paramTab === "All" || paramTab === "Rules") && (
            <div className="px-2 py-1.5 text-center text-text-muted/30">No Rules</div>
          )}
        </div>

        <div className="flex items-center justify-end gap-1 mt-1.5">
          <button onClick={() => setFpPickerOpen(true)}
            className="px-2 py-0.5 text-[9px] bg-bg-secondary border border-border-subtle rounded text-text-secondary hover:text-text-primary">
            Add <ChevronDown size={8} className="inline" />
          </button>
        </div>
      </div>

      {/* Graphical section */}
      <SectionHeader title="Graphical" />
      <div className="px-3 py-2 space-y-2">
        <label className="flex items-center gap-2 cursor-pointer">
          <input type="checkbox" checked={mirrored} onChange={e => setMirrored(e.target.checked)} className="rounded" />
          <span className="text-text-secondary">Mirrored</span>
        </label>
        <div className="flex items-center gap-3 flex-wrap">
          <span className="text-text-muted/60">Local Colors</span>
          <span className="text-text-muted/50">Fills</span>
          <div className="w-4 h-4 rounded border border-white/10 bg-[#666644] cursor-pointer" title="Fill color" />
          <span className="text-text-muted/50">Lines</span>
          <div className="w-4 h-4 rounded border border-white/10 bg-[#cc4444] cursor-pointer" title="Line color" />
          <span className="text-text-muted/50">Pins</span>
          <div className="w-4 h-4 rounded border border-white/10 bg-[#888888] cursor-pointer" title="Pin color" />
        </div>
      </div>

      {/* Part Choices section */}
      <SectionHeader title="Part Choices" />
      <div className="px-3 py-2">
        <button className="px-2 py-1 text-[10px] bg-bg-secondary border border-border-subtle rounded text-text-secondary hover:text-text-primary">
          Edit Supplier Links...
        </button>
        <div className="mt-1 text-text-muted/40">No part choices found</div>
      </div>

      {/* Pin visibility toggles */}
      <SectionHeader title="Display" />
      <div className="px-3 py-2 space-y-1">
        <label className="flex items-center gap-2 cursor-pointer">
          <input type="checkbox" checked={symbol.show_pin_numbers}
            onChange={e => updateSymbolMeta({ show_pin_numbers: e.target.checked })} />
          <span className="text-text-secondary">Show Pin Numbers</span>
        </label>
        <label className="flex items-center gap-2 cursor-pointer">
          <input type="checkbox" checked={symbol.show_pin_names}
            onChange={e => updateSymbolMeta({ show_pin_names: e.target.checked })} />
          <span className="text-text-secondary">Show Pin Names</span>
        </label>
        <Row label="Name Offset">
          <input type="number" step={0.1} value={symbol.pin_name_offset}
            onChange={e => updateSymbolMeta({ pin_name_offset: parseFloat(e.target.value) || 0 })}
            className={cn(inp, "w-16")} />
        </Row>
      </div>
    </>
    </>
  );
}

// ---------------------------------------------------------------------------
// Pins tab — table of all pins with inline editing
// ---------------------------------------------------------------------------

function PinsTab() {
  const symbol = useLibraryEditorStore(s => s.symbol);
  const updatePin = useLibraryEditorStore(s => s.updatePin);
  const selectedItem = useLibraryEditorStore(s => s.selectedItem);
  const setSelectedItem = useLibraryEditorStore(s => s.setSelectedItem);

  if (!symbol) return null;

  // If a pin is selected, show its detail properties
  if (selectedItem?.type === "pin" && selectedItem.index < symbol.pins.length) {
    return <PinDetailProperties pin={symbol.pins[selectedItem.index]} index={selectedItem.index} updatePin={updatePin} />;
  }

  // Otherwise show pin table
  return (
    <div className="overflow-auto">
      <table className="w-full border-collapse">
        <thead className="sticky top-0 z-10 bg-bg-surface">
          <tr className="border-b border-border-subtle text-left">
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium w-[28px]">#</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium">Name</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium w-[36px]">Num</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium">Type</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 font-medium w-[36px]">Len</th>
          </tr>
        </thead>
        <tbody>
          {symbol.pins.map((pin, i) => {
            const isSel = selectedItem?.type === "pin" && selectedItem.index === i;
            return (
              <tr key={i} onClick={() => setSelectedItem({ type: "pin", index: i })}
                className={cn("border-b border-border-subtle/20 cursor-pointer",
                  isSel ? "bg-accent/15" : "hover:bg-bg-hover/50"
                )}>
                <td className="px-1.5 py-0.5 text-text-muted/50 tabular-nums">{i + 1}</td>
                <td className="px-1.5 py-0.5">
                  <input value={pin.name} onChange={e => updatePin(i, { name: e.target.value })}
                    className="bg-transparent w-full outline-none text-text-primary font-mono text-[10px]" />
                </td>
                <td className="px-1.5 py-0.5">
                  <input value={pin.number} onChange={e => updatePin(i, { number: e.target.value })}
                    className="bg-transparent w-full outline-none text-text-primary font-mono text-[10px]" />
                </td>
                <td className="px-1.5 py-0.5 text-text-muted/60">{pin.pin_type.replace(/_/g, " ")}</td>
                <td className="px-1.5 py-0.5 font-mono">{pin.length.toFixed(1)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {symbol.pins.length === 0 && (
        <div className="p-4 text-center text-text-muted/40">No pins</div>
      )}
      <div className="px-2 py-1 border-t border-border-subtle text-text-muted/50">
        {symbol.pins.length} pin{symbol.pins.length !== 1 ? "s" : ""}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pin detail properties (when a pin is selected)
// ---------------------------------------------------------------------------

function PinDetailProperties({ pin, index, updatePin }: { pin: SchPin; index: number; updatePin: (i: number, u: Partial<SchPin>) => void }) {
  const u = useCallback((updates: Partial<SchPin>) => updatePin(index, updates), [index, updatePin]);

  return (
    <div className="overflow-y-auto">
      <SectionHeader title={`Pin ${pin.number}: ${pin.name}`} />
      <div className="px-3 py-1.5 space-y-1">
        <Row label="Name">
          <input value={pin.name} onChange={e => u({ name: e.target.value })} className={inp} />
        </Row>
        <Row label="Designator">
          <input value={pin.number} onChange={e => u({ number: e.target.value })} className={cn(inp, "w-16")} />
        </Row>
        <Row label="Electrical Type">
          <select value={pin.pin_type} onChange={e => u({ pin_type: e.target.value })} className={inp}>
            {PIN_TYPES.map(t => <option key={t} value={t}>{t.replace(/_/g, " ")}</option>)}
          </select>
        </Row>
        <Row label="Pin Shape">
          <select value={pin.shape} onChange={e => u({ shape: e.target.value })} className={inp}>
            {PIN_SHAPES.map(s => <option key={s} value={s}>{s.replace(/_/g, " ")}</option>)}
          </select>
        </Row>
        <Row label="Length">
          <input type="number" step={0.01} value={pin.length}
            onChange={e => u({ length: parseFloat(e.target.value) || 2.54 })} className={cn(inp, "w-16")} />
        </Row>
        <Row label="Orientation">
          <select value={pin.rotation} onChange={e => u({ rotation: parseInt(e.target.value) })} className={cn(inp, "w-20")}>
            {[0, 90, 180, 270].map(r => <option key={r} value={r}>{r}&deg;</option>)}
          </select>
        </Row>
        <Row label="X">
          <input type="number" step={1.27} value={pin.position.x.toFixed(2)}
            onChange={e => u({ position: { ...pin.position, x: parseFloat(e.target.value) || 0 } })} className={cn(inp, "w-20")} />
        </Row>
        <Row label="Y">
          <input type="number" step={1.27} value={pin.position.y.toFixed(2)}
            onChange={e => u({ position: { ...pin.position, y: parseFloat(e.target.value) || 0 } })} className={cn(inp, "w-20")} />
        </Row>
        <label className="flex items-center gap-2 cursor-pointer py-0.5">
          <input type="checkbox" checked={pin.name_visible} onChange={e => u({ name_visible: e.target.checked })} />
          <span className="text-text-secondary">Name Visible</span>
        </label>
        <label className="flex items-center gap-2 cursor-pointer py-0.5">
          <input type="checkbox" checked={pin.number_visible} onChange={e => u({ number_visible: e.target.checked })} />
          <span className="text-text-secondary">Number Visible</span>
        </label>
        <label className="flex items-center gap-2 cursor-pointer py-0.5">
          <input type="checkbox" checked={pin.hidden ?? false} onChange={e => u({ hidden: e.target.checked })} />
          <span className="text-text-secondary">Hidden</span>
        </label>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shared UI
// ---------------------------------------------------------------------------

function SectionHeader({ title }: { title: string }) {
  return (
    <div className="px-3 py-1.5 bg-bg-secondary/80 border-y border-border-subtle text-[10px] font-semibold text-text-secondary">
      {title}
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-start gap-2">
      <span className="w-[80px] shrink-0 text-text-muted/60 pt-0.5 text-right">{label}</span>
      <div className="flex-1">{children}</div>
    </div>
  );
}
