import { useCallback, useState } from "react";
import { useFootprintEditorStore } from "@/stores/footprintEditor";
import type { PcbPad, PcbGraphic } from "@/types/pcb";
import { LAYER_DISPLAY_NAMES } from "@/types/pcb";
import { cn } from "@/lib/utils";

const PAD_TYPES = ["smd", "thru_hole", "np_thru_hole", "connect"] as const;
const PAD_SHAPES = ["rect", "roundrect", "circle", "oval", "trapezoid", "custom"] as const;

export function FootprintEditorProperties() {
  const footprint = useFootprintEditorStore(s => s.footprint);
  const selectedItem = useFootprintEditorStore(s => s.selectedItem);
  const updatePad = useFootprintEditorStore(s => s.updatePad);
  const updateGraphic = useFootprintEditorStore(s => s.updateGraphic);
  const updateFootprintId = useFootprintEditorStore(s => s.updateFootprintId);
  const [tab, setTab] = useState<"properties" | "padTable">("properties");

  if (!footprint) return <div className="w-[280px] border-l border-border bg-bg-primary p-3 text-[10px] text-text-muted/50">No footprint loaded</div>;

  return (
    <div className="flex flex-col h-full w-[280px] border-l border-border bg-bg-primary">
      {/* Tabs */}
      <div className="flex border-b border-border-subtle bg-bg-secondary/80 shrink-0">
        {([["properties", "Properties"], ["padTable", "Pad Table"]] as const).map(([id, label]) => (
          <button key={id} onClick={() => setTab(id)}
            className={cn("px-3 py-1.5 text-[10px] font-medium transition-colors",
              tab === id ? "text-accent border-b-2 border-accent" : "text-text-muted/60 hover:text-text-primary"
            )}>
            {label}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto text-[10px]">
        {tab === "properties" ? (
          <>
            {/* Footprint metadata */}
            <Section title="Footprint">
              <Field label="ID">
                <input value={footprint.id} onChange={e => updateFootprintId(e.target.value)}
                  className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none" />
              </Field>
              <Field label="Pads">{footprint.pads.length}</Field>
              <Field label="Graphics">{footprint.graphics.length}</Field>
              <Field label="3D Model">
                <input value={footprint.model3d} onChange={() => {}} placeholder="(none)" className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none" disabled />
              </Field>
            </Section>

            {/* Selected pad properties */}
            {selectedItem?.type === "pad" && selectedItem.index < footprint.pads.length && (
              <PadProperties pad={footprint.pads[selectedItem.index]} index={selectedItem.index} updatePad={updatePad} />
            )}

            {/* Selected graphic properties */}
            {selectedItem?.type === "graphic" && selectedItem.index < footprint.graphics.length && (
              <GraphicProperties graphic={footprint.graphics[selectedItem.index]} index={selectedItem.index} updateGraphic={updateGraphic} />
            )}

            {!selectedItem && <div className="p-3 text-text-muted/40">Select a pad or graphic to edit</div>}
          </>
        ) : (
          <PadTable />
        )}
      </div>
    </div>
  );
}

function PadProperties({ pad, index, updatePad }: { pad: PcbPad; index: number; updatePad: (i: number, u: Partial<PcbPad>) => void }) {
  const u = useCallback((updates: Partial<PcbPad>) => updatePad(index, updates), [index, updatePad]);

  return (
    <Section title={`Pad ${pad.number}`}>
      <Field label="Number">
        <input value={pad.number} onChange={e => u({ number: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
      </Field>
      <Field label="Type">
        <select value={pad.type} onChange={e => u({ type: e.target.value as any })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none">
          {PAD_TYPES.map(t => <option key={t} value={t}>{t.replace(/_/g, " ")}</option>)}
        </select>
      </Field>
      <Field label="Shape">
        <select value={pad.shape} onChange={e => u({ shape: e.target.value as any })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none">
          {PAD_SHAPES.map(s => <option key={s} value={s}>{s}</option>)}
        </select>
      </Field>
      <Field label="Width">
        <input type="number" step={0.05} value={pad.size[0]}
          onChange={e => u({ size: [parseFloat(e.target.value) || 0.5, pad.size[1]] })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
      </Field>
      <Field label="Height">
        <input type="number" step={0.05} value={pad.size[1]}
          onChange={e => u({ size: [pad.size[0], parseFloat(e.target.value) || 0.5] })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
      </Field>
      <Field label="X">
        <input type="number" step={0.1} value={pad.position.x.toFixed(3)}
          onChange={e => u({ position: { ...pad.position, x: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
      </Field>
      <Field label="Y">
        <input type="number" step={0.1} value={pad.position.y.toFixed(3)}
          onChange={e => u({ position: { ...pad.position, y: parseFloat(e.target.value) || 0 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-20" />
      </Field>
      {pad.type === "thru_hole" && (
        <Field label="Drill">
          <input type="number" step={0.05} value={pad.drill?.diameter ?? 0}
            onChange={e => u({ drill: { ...pad.drill, diameter: parseFloat(e.target.value) || 0.5 } })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
        </Field>
      )}
      {pad.shape === "roundrect" && (
        <Field label="Corner %">
          <input type="number" step={5} min={0} max={100} value={Math.round((pad.roundrectRatio ?? 0.25) * 100)}
            onChange={e => u({ roundrectRatio: (parseInt(e.target.value) || 25) / 100 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
        </Field>
      )}
      <Field label="Layers">
        <span className="text-text-muted/60 font-mono">{pad.layers.map(l => LAYER_DISPLAY_NAMES[l] || l).join(", ")}</span>
      </Field>
    </Section>
  );
}

function GraphicProperties({ graphic, index, updateGraphic }: { graphic: PcbGraphic; index: number; updateGraphic: (i: number, g: PcbGraphic) => void }) {
  const update = useCallback((partial: Record<string, any>) => {
    updateGraphic(index, { ...graphic, ...partial } as any);
  }, [index, graphic, updateGraphic]);

  return (
    <Section title={`Graphic: ${graphic.type}`}>
      <Field label="Layer">
        <span className="font-mono text-text-muted/60">{LAYER_DISPLAY_NAMES[graphic.layer] || graphic.layer}</span>
      </Field>
      {"width" in graphic && (
        <Field label="Line Width">
          <input type="number" step={0.01} value={(graphic as any).width}
            onChange={e => update({ width: parseFloat(e.target.value) || 0.12 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
        </Field>
      )}
      {graphic.type === "circle" && (
        <Field label="Radius">
          <input type="number" step={0.05} value={graphic.radius}
            onChange={e => update({ radius: parseFloat(e.target.value) || 0.5 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
        </Field>
      )}
      {graphic.type === "text" && (
        <>
          <Field label="Text">
            <input value={graphic.text} onChange={e => update({ text: e.target.value })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none" />
          </Field>
          <Field label="Font Size">
            <input type="number" step={0.1} value={graphic.fontSize}
              onChange={e => update({ fontSize: parseFloat(e.target.value) || 1 })} className="bg-bg-secondary border border-border-subtle rounded px-1 py-0.5 text-[10px] text-text-primary outline-none w-16" />
          </Field>
        </>
      )}
    </Section>
  );
}

function PadTable() {
  const footprint = useFootprintEditorStore(s => s.footprint);
  const updatePad = useFootprintEditorStore(s => s.updatePad);
  const selectedItem = useFootprintEditorStore(s => s.selectedItem);
  const setSelectedItem = useFootprintEditorStore(s => s.setSelectedItem);

  if (!footprint) return null;

  return (
    <div className="overflow-auto">
      <table className="w-full border-collapse">
        <thead className="sticky top-0 z-10 bg-bg-surface">
          <tr className="border-b border-border-subtle text-left">
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 w-[32px]">#</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50">Type</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50">Shape</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 w-[44px]">W</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 w-[44px]">H</th>
            <th className="px-1.5 py-1 text-[9px] uppercase text-text-muted/50 w-[44px]">Drill</th>
          </tr>
        </thead>
        <tbody>
          {footprint.pads.map((pad, i) => {
            const isSel = selectedItem?.type === "pad" && selectedItem.index === i;
            return (
              <tr key={pad.uuid} onClick={() => setSelectedItem({ type: "pad", index: i })}
                className={cn("border-b border-border-subtle/20 cursor-pointer", isSel ? "bg-accent/15" : "hover:bg-bg-hover/50")}>
                <td className="px-1.5 py-0.5 font-mono">{pad.number}</td>
                <td className="px-1.5 py-0.5">
                  <select value={pad.type} onChange={e => updatePad(i, { type: e.target.value as any })}
                    className="bg-transparent outline-none text-[10px] w-full">
                    {PAD_TYPES.map(t => <option key={t} value={t}>{t.replace(/_/g, " ")}</option>)}
                  </select>
                </td>
                <td className="px-1.5 py-0.5">
                  <select value={pad.shape} onChange={e => updatePad(i, { shape: e.target.value as any })}
                    className="bg-transparent outline-none text-[10px] w-full">
                    {PAD_SHAPES.map(s => <option key={s} value={s}>{s}</option>)}
                  </select>
                </td>
                <td className="px-1.5 py-0.5 font-mono">{pad.size[0].toFixed(2)}</td>
                <td className="px-1.5 py-0.5 font-mono">{pad.size[1].toFixed(2)}</td>
                <td className="px-1.5 py-0.5 font-mono">{pad.drill?.diameter?.toFixed(2) ?? "--"}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {footprint.pads.length === 0 && <div className="p-4 text-center text-text-muted/40">No pads. Use SMD/TH pad tools.</div>}
      <div className="px-2 py-1 border-t border-border-subtle text-text-muted/50">{footprint.pads.length} pads</div>
    </div>
  );
}

// Shared UI
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
      <span className="w-20 shrink-0 text-text-muted/60">{label}</span>
      <div className="flex-1 text-text-primary">{children}</div>
    </div>
  );
}

